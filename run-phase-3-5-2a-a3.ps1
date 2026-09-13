param(
    [int]$DesktopExitTimeoutSeconds = 60,
    [int]$DesktopExitStabilityMilliseconds = 2000,
    [int]$DesktopExitPollMilliseconds = 250,
    [int]$GoSignalPollMilliseconds = 250,
    [string]$ThreadId = "01a08c05-7880-75a2-976c-2a5895b58723",
    [string]$WorkspaceId = "phase-3-5-1d-workspace",
    [string]$TargetId = "phase-3-5-1d-target",
    [string]$ExpectedRemoteHostIdentity = "014383f2-41f8-4b13-b9d7-30c511e47cec",
    [string]$SettingsPath = "F:\AI\CodexMonitor\.worktrees\phase-3-5-2a-remote-read-admission\src-tauri\target\phase-3-5-2a-a3-host-settings\settings.json",
    [string]$StateDbPath = "C:\Users\DeanX\.codex\state_5.sqlite",
    [string]$SqlitePath = "F:\Software\SQLite\sqlite3.exe",
    [string]$ResultDirectory = "F:\AI\CodexMonitor\.worktrees\phase-3-5-2a-remote-read-admission\src-tauri\target\phase-3-5-2a-a3",
    [ValidateSet(
        "AUTH_FAILURE",
        "IDENTITY_MISMATCH",
        "WORKSPACE_DISCONNECTED",
        "READY_NO_GO",
        "GO_TRANSITION",
        "FINAL_WORKSPACE_DISCONNECTED",
        "FINAL_READY",
        "UTF8_NO_BOM_CANONICAL_SNAPSHOT",
        "SHARED_WRITER_CANONICAL_SNAPSHOT",
        "CANONICAL_GATE_UNRELATED_GROWTH",
        "FULL_LIFECYCLE_AUTH_FAILURE",
        "FULL_LIFECYCLE_FINAL_IDENTITY_MISMATCH",
        "FULL_LIFECYCLE_FINAL_WORKSPACE_DISCONNECTED",
        "FULL_LIFECYCLE_FINAL_READY"
    )]
    [string]$FileScopeRegressionScenario,
    [string]$FileScopeRegressionOutputPath,
    [ValidateSet("ARMED_CONTROL", "UNHANDLED_FAILURE")]
    [string]$LifecycleRegressionScenario,
    [int]$SupervisorPid = 0,
    [switch]$LibraryOnly
)

$ErrorActionPreference = "Stop"
$ResultPath = Join-Path $ResultDirectory "result.json"
$ArmedPath = Join-Path $ResultDirectory "armed.json"
$GoSignalPath = Join-Path $ResultDirectory "go.signal"
$CancelSignalPath = Join-Path $ResultDirectory "cancel.signal"
$RunnerStartedAt = [DateTimeOffset]::UtcNow
$InvokedMethods = [System.Collections.Generic.List[string]]::new()

function Write-JsonAtomically {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Value
    )

    $json = $Value | ConvertTo-Json -Depth 20
    $tempPath = "$Path.tmp-$PID"
    $encoding = [System.Text.UTF8Encoding]::new($false)
    [System.IO.File]::WriteAllText($tempPath, $json, $encoding)
    Move-Item -LiteralPath $tempPath -Destination $Path -Force
}

function Get-RunnerAncestorChain {
    $chain = [System.Collections.Generic.List[object]]::new()
    $currentPid = $PID

    for ($index = 0; $index -lt 16 -and $currentPid -gt 0; $index += 1) {
        $process = Get-CimInstance Win32_Process -Filter "ProcessId=$currentPid"
        if ($null -eq $process) {
            break
        }

        $chain.Add([pscustomobject]@{
            pid = [int]$process.ProcessId
            parentPid = [int]$process.ParentProcessId
            name = [string]$process.Name
            executablePath = [string]$process.ExecutablePath
        })

        if ($process.ParentProcessId -le 0 -or $process.ParentProcessId -eq $currentPid) {
            break
        }
        $currentPid = [int]$process.ParentProcessId
    }

    return @($chain)
}

function Test-DesktopDependentAncestor {
    param([object[]]$Chain)

    return [bool]($Chain | Where-Object {
        $name = ([string]$_.name).ToLowerInvariant()
        $path = ([string]$_.executablePath).ToLowerInvariant()
        $name -eq "chatgpt.exe" -or
        $name -like "codex-command-runner*" -or
        $name -eq "codex-code-mode-host.exe" -or
        $path -like "*\openai\codex\bin\*"
    } | Select-Object -First 1)
}

function Get-CodexDesktopProcessTree {
    $processes = @(Get-CimInstance Win32_Process)
    $roots = @($processes | Where-Object {
        $name = ([string]$_.Name).ToLowerInvariant()
        $path = ([string]$_.ExecutablePath).ToLowerInvariant()
        $commandLine = ([string]$_.CommandLine).ToLowerInvariant()
        $name -eq "chatgpt.exe" -and (
            $path -like "*\windowsapps\openai.codex_*" -or
            $commandLine -like "*\windowsapps\openai.codex_*" -or
            (-not $path -and -not $commandLine)
        )
    })

    $treePids = [System.Collections.Generic.HashSet[int]]::new()
    foreach ($root in $roots) {
        [void]$treePids.Add([int]$root.ProcessId)
    }

    $added = $true
    while ($added) {
        $added = $false
        foreach ($process in $processes) {
            if ($treePids.Contains([int]$process.ParentProcessId) -and
                $treePids.Add([int]$process.ProcessId)) {
                $added = $true
            }
        }
    }

    return @($processes | Where-Object { $treePids.Contains([int]$_.ProcessId) } | ForEach-Object {
        [pscustomobject]@{
            pid = [int]$_.ProcessId
            parentPid = [int]$_.ParentProcessId
            name = [string]$_.Name
            executablePath = [string]$_.ExecutablePath
        }
    })
}

function Wait-CodexDesktopExit {
    param(
        [Parameter(Mandatory = $true)][object[]]$InitialProcesses,
        [Parameter(Mandatory = $true)][double]$TimeoutSeconds,
        [Parameter(Mandatory = $true)][int]$StabilityMilliseconds,
        [Parameter(Mandatory = $true)][int]$PollMilliseconds,
        [scriptblock]$DesktopTreeProvider = { @(Get-CodexDesktopProcessTree) },
        [scriptblock]$ProcessInventoryProvider = { @(Get-CimInstance Win32_Process) }
    )

    $trackedPids = [System.Collections.Generic.HashSet[int]]::new()
    foreach ($process in $InitialProcesses) {
        [void]$trackedPids.Add([int]$process.pid)
    }

    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
    $exitObservedAt = $null
    $stableStartedAt = $null
    $lastActiveProcesses = @($InitialProcesses)

    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        $currentTree = @(& $DesktopTreeProvider)
        foreach ($process in $currentTree) {
            [void]$trackedPids.Add([int]$process.pid)
        }

        $processInventory = @(& $ProcessInventoryProvider)
        $trackedAlive = @($processInventory | Where-Object {
            $trackedPids.Contains([int]$_.ProcessId)
        } | ForEach-Object {
            [pscustomobject]@{
                pid = [int]$_.ProcessId
                parentPid = [int]$_.ParentProcessId
                name = [string]$_.Name
                executablePath = [string]$_.ExecutablePath
            }
        })

        $lastActiveProcesses = @($trackedAlive + $currentTree | Sort-Object pid -Unique)
        if ($lastActiveProcesses.Count -eq 0) {
            if ($null -eq $stableStartedAt) {
                $exitObservedAt = [DateTimeOffset]::UtcNow
                $stableStartedAt = $exitObservedAt
            }

            $stableFor = ([DateTimeOffset]::UtcNow - $stableStartedAt).TotalMilliseconds
            if ($stableFor -ge $StabilityMilliseconds) {
                return [pscustomobject]@{
                    exited = $true
                    exitObservedAt = $exitObservedAt.ToString("o")
                    stableAt = [DateTimeOffset]::UtcNow.ToString("o")
                    activeProcesses = @()
                }
            }
        } else {
            $exitObservedAt = $null
            $stableStartedAt = $null
        }

        Start-Sleep -Milliseconds $PollMilliseconds
    }

    return [pscustomobject]@{
        exited = $false
        exitObservedAt = $null
        stableAt = $null
        activeProcesses = $lastActiveProcesses
    }
}

function Test-TcpEndpointReachable {
    param(
        [Parameter(Mandatory = $true)][string]$HostName,
        [Parameter(Mandatory = $true)][int]$Port,
        [int]$TimeoutMilliseconds = 2000
    )

    $client = [System.Net.Sockets.TcpClient]::new()
    try {
        $pending = $client.BeginConnect($HostName, $Port, $null, $null)
        if (-not $pending.AsyncWaitHandle.WaitOne($TimeoutMilliseconds)) {
            return $false
        }
        $client.EndConnect($pending)
        return $client.Connected
    } catch {
        return $false
    } finally {
        $client.Dispose()
    }
}

function Read-A3Utf8JsonLines {
    param([Parameter(Mandatory = $true)][string]$Path)

    $share = [System.IO.FileShare]::ReadWrite -bor [System.IO.FileShare]::Delete
    $stream = [System.IO.FileStream]::new(
        $Path,
        [System.IO.FileMode]::Open,
        [System.IO.FileAccess]::Read,
        $share
    )
    $reader = $null
    try {
        $reader = [System.IO.StreamReader]::new(
            $stream,
            [System.Text.Encoding]::UTF8,
            $true,
            4096,
            $true
        )
        while (($line = $reader.ReadLine()) -ne $null) {
            $line | ConvertFrom-Json
        }
    } finally {
        if ($null -ne $reader) {
            $reader.Dispose()
        }
        $stream.Dispose()
    }
}

function Get-CanonicalSnapshot {
    param([string]$ExactThreadId)

    $escapedThreadId = $ExactThreadId.Replace("'", "''")
    $sql = @"
select
  count(*) as total,
  count(distinct id) as uniqueIds,
  sum(case when id='$escapedThreadId' then 1 else 0 end) as exactRows
from threads;
select rollout_path as rolloutPath
from threads
where id='$escapedThreadId';
"@
    $rows = & $SqlitePath -readonly -json $StateDbPath $sql
    if ($LASTEXITCODE -ne 0) {
        throw "sqlite canonical snapshot failed with exit code $LASTEXITCODE"
    }

    $documents = @($rows | ConvertFrom-Json)
    $counts = $documents[0]
    $rolloutPath = if ($documents.Count -gt 1) { [string]$documents[1].rolloutPath } else { $null }
    $turnCount = $null
    $rolloutLines = $null
    if ($rolloutPath -and (Test-Path -LiteralPath $rolloutPath)) {
        $records = @(Read-A3Utf8JsonLines -Path $rolloutPath)
        $turnCount = @($records | Where-Object type -eq "turn_context").Count
        $rolloutLines = $records.Count
    }

    $codexHomePath = [System.IO.Path]::GetDirectoryName([System.IO.Path]::GetFullPath($StateDbPath)).Replace("/", "\").ToLowerInvariant()
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
        $identityDigest = $sha256.ComputeHash([System.Text.Encoding]::UTF8.GetBytes($codexHomePath))
    } finally {
        $sha256.Dispose()
    }
    $codexHomeIdentity = "codex-home:" + (($identityDigest | ForEach-Object { $_.ToString("x2") }) -join "")

    return [pscustomobject]@{
        total = [int]$counts.total
        unique = [int]$counts.uniqueIds
        duplicate = [int]$counts.total - [int]$counts.uniqueIds
        exactRows = [int]$counts.exactRows
        codexHomeIdentity = $codexHomeIdentity
        fullThreadId = $ExactThreadId
        rolloutPath = $rolloutPath
        turnCount = $turnCount
        rolloutLines = $rolloutLines
    }
}

function Test-A3CanonicalBeforeAdmission {
    param(
        [Parameter(Mandatory = $true)]$Snapshot,
        [Parameter(Mandatory = $true)][string]$ExpectedThreadId
    )

    return [ordered]@{
        canonicalDuplicateZeroBefore = ($Snapshot.duplicate -eq 0)
        exactTargetRowBefore = ($Snapshot.exactRows -eq 1)
        targetFullThreadIdBeforeExact = [string]::Equals(
            [string]$Snapshot.fullThreadId,
            $ExpectedThreadId,
            [System.StringComparison]::Ordinal
        )
        targetCodexHomeIdentityBeforePresent = (-not [string]::IsNullOrWhiteSpace([string]$Snapshot.codexHomeIdentity))
        targetRolloutPathBeforePresent = (-not [string]::IsNullOrWhiteSpace([string]$Snapshot.rolloutPath))
        targetTurnCountBeforeOne = ($Snapshot.turnCount -eq 1)
    }
}

function Get-A3CanonicalGateEvaluation {
    param(
        [Parameter(Mandatory = $true)]$Before,
        [Parameter(Mandatory = $true)]$After,
        [Parameter(Mandatory = $true)][string]$ExpectedThreadId
    )

    $beforeAssertions = Test-A3CanonicalBeforeAdmission -Snapshot $Before -ExpectedThreadId $ExpectedThreadId
    $assertions = [ordered]@{}
    foreach ($entry in $beforeAssertions.GetEnumerator()) {
        $assertions[$entry.Key] = $entry.Value
    }
    $assertions.canonicalDuplicateZeroAfter = ($After.duplicate -eq 0)
    $assertions.exactTargetRowAfter = ($After.exactRows -eq 1)
    $assertions.targetFullThreadIdAfterExact = [string]::Equals(
        [string]$After.fullThreadId,
        $ExpectedThreadId,
        [System.StringComparison]::Ordinal
    )
    $assertions.targetCodexThreadKeyUnchanged = (
        -not [string]::IsNullOrWhiteSpace([string]$Before.codexHomeIdentity) -and
        [string]::Equals(
            [string]$Before.codexHomeIdentity,
            [string]$After.codexHomeIdentity,
            [System.StringComparison]::Ordinal
        ) -and
        [string]::Equals(
            [string]$Before.fullThreadId,
            [string]$After.fullThreadId,
            [System.StringComparison]::Ordinal
        )
    )
    $assertions.targetRolloutPathUnchanged = (
        -not [string]::IsNullOrWhiteSpace([string]$Before.rolloutPath) -and
        [string]::Equals(
            [string]$Before.rolloutPath,
            [string]$After.rolloutPath,
            [System.StringComparison]::Ordinal
        )
    )
    $assertions.targetTurnCountAfterOne = ($After.turnCount -eq 1)

    return [pscustomobject]@{
        assertions = $assertions
        telemetry = [ordered]@{
            beforeTotal = [int]$Before.total
            afterTotal = [int]$After.total
            totalDelta = [int]$After.total - [int]$Before.total
            beforeUnique = [int]$Before.unique
            afterUnique = [int]$After.unique
        }
    }
}

function Invoke-DaemonRpc {
    param(
        [Parameter(Mandatory = $true)][int]$Id,
        [Parameter(Mandatory = $true)][string]$Method,
        [Parameter(Mandatory = $true)]$Params,
        [Parameter(Mandatory = $true)][System.IO.StreamWriter]$Writer,
        [Parameter(Mandatory = $true)][System.IO.StreamReader]$Reader
    )

    $InvokedMethods.Add($Method)
    $request = @{ id = $Id; method = $Method; params = $Params } | ConvertTo-Json -Compress -Depth 12
    $Writer.WriteLine($request)

    while ($true) {
        $line = $Reader.ReadLine()
        if ($null -eq $line) {
            throw "transport ended while waiting for RPC $Method"
        }
        $message = $line | ConvertFrom-Json
        if ($message.id -eq $Id) {
            return $message
        }
    }
}

function Invoke-A3ReadinessSequence {
    param(
        [Parameter(Mandatory = $true)][scriptblock]$Rpc,
        [Parameter(Mandatory = $true)][string]$Token,
        [Parameter(Mandatory = $true)][string]$ExpectedRemoteHostIdentity,
        [Parameter(Mandatory = $true)][string]$WorkspaceId
    )

    $auth = & $Rpc 1 "auth" @{ token = $Token }
    if ($auth.error) {
        throw "daemon authentication failed"
    }

    $daemonInfo = & $Rpc 2 "daemon_info" @{}
    if ($daemonInfo.error) {
        throw "daemon_info failed: $($daemonInfo.error.message)"
    }
    if ([string]$daemonInfo.result.name -ne "codex-monitor-daemon") {
        throw "remote daemon identity response has an unexpected service name"
    }
    if ([string]$daemonInfo.result.mode -ne "tcp") {
        throw "remote daemon identity response has an unexpected mode"
    }
    if ([int]$daemonInfo.result.protocolVersion -ne 1) {
        throw "unsupported remote daemon protocol version: $($daemonInfo.result.protocolVersion)"
    }
    $observedRemoteHostIdentity = [string]$daemonInfo.result.remoteHostIdentity
    if ($observedRemoteHostIdentity -ne $ExpectedRemoteHostIdentity) {
        throw "remote host identity mismatch"
    }

    $workspaces = & $Rpc 3 "list_workspaces" @{}
    if ($workspaces.error) {
        throw "list_workspaces failed: $($workspaces.error.message)"
    }
    $workspace = @($workspaces.result | Where-Object id -eq $WorkspaceId)
    if ($workspace.Count -ne 1) {
        throw "acceptance workspace was not found"
    }
    if (-not [bool]$workspace[0].connected) {
        throw "acceptance WorkspaceSession is not connected"
    }

    return [pscustomobject]@{
        daemonName = [string]$daemonInfo.result.name
        daemonMode = [string]$daemonInfo.result.mode
        daemonProtocolVersion = [int]$daemonInfo.result.protocolVersion
        observedRemoteHostIdentity = $observedRemoteHostIdentity
        workspaceId = [string]$workspace[0].id
        workspaceConnected = $true
    }
}

function Invoke-A3FinalRpcSequence {
    param(
        [Parameter(Mandatory = $true)][scriptblock]$Rpc,
        [Parameter(Mandatory = $true)][string]$Token,
        [Parameter(Mandatory = $true)][string]$ExpectedRemoteHostIdentity,
        [Parameter(Mandatory = $true)][string]$WorkspaceId,
        [Parameter(Mandatory = $true)][string]$ThreadId,
        [System.Collections.IDictionary]$State,
        $Stage,
        [scriptblock]$CanonicalSnapshotProvider
    )

    $readiness = Invoke-A3ReadinessSequence `
        -Rpc $Rpc `
        -Token $Token `
        -ExpectedRemoteHostIdentity $ExpectedRemoteHostIdentity `
        -WorkspaceId $WorkspaceId
    $canonicalBefore = $null
    $hasAdmissionEvidence = (
        $null -ne $State -or
        $null -ne $Stage -or
        $null -ne $CanonicalSnapshotProvider
    )
    if ($hasAdmissionEvidence) {
        if ($null -eq $State -or $null -eq $Stage -or $null -eq $CanonicalSnapshotProvider) {
            throw "final admission evidence requires State, Stage, and CanonicalSnapshotProvider"
        }
        $State.observedRemoteHostIdentity = $readiness.observedRemoteHostIdentity
        $State.workspaceConnectedBeforeResume = $readiness.workspaceConnected
        $State.finalReadinessPassed = $true
        $Stage.value = "CANONICAL_BEFORE"
        $canonicalBefore = & $CanonicalSnapshotProvider -ExactThreadId $ThreadId
        $State.canonicalBefore = $canonicalBefore
        $State.turnCountBefore = $canonicalBefore.turnCount
        $canonicalBeforeAssertions = Test-A3CanonicalBeforeAdmission `
            -Snapshot $canonicalBefore `
            -ExpectedThreadId $ThreadId
        $State.canonicalBeforeAssertions = $canonicalBeforeAssertions
        $preResumeMutationMethods = @($InvokedMethods | Where-Object {
            $_ -in @(
                "resume_thread",
                "thread/start",
                "start_thread",
                "turn/start",
                "start_turn",
                "connect_workspace"
            )
        })
        $State.noTargetMutationBeforeResume = ($preResumeMutationMethods.Count -eq 0)
        $failedCanonicalBeforeAssertions = @(
            $canonicalBeforeAssertions.GetEnumerator() |
                Where-Object { -not $_.Value } |
                ForEach-Object Key
        )
        if ($failedCanonicalBeforeAssertions.Count -gt 0) {
            throw "canonical pre-resume invariant failed: $($failedCanonicalBeforeAssertions -join ', ')"
        }
        if (-not $State.noTargetMutationBeforeResume) {
            throw "target mutation occurred before exact resume admission"
        }
        $Stage.value = "RESUME"
        $State.resumeAttemptCount = 1
    }
    $resumeResponse = & $Rpc 4 "resume_thread" @{
        workspaceId = $WorkspaceId
        threadId = $ThreadId
    }

    return [pscustomobject]@{
        readiness = $readiness
        canonicalBefore = $canonicalBefore
        resumeResponse = $resumeResponse
    }
}

function New-A3ArmState {
    param(
        [Parameter(Mandatory = $true)][int]$RunnerPid,
        [int]$SupervisorPid = 0,
        [Parameter(Mandatory = $true)][DateTimeOffset]$RunnerStartedAt,
        [Parameter(Mandatory = $true)][DateTimeOffset]$ArmedAt,
        [Parameter(Mandatory = $true)][string]$RunnerPath,
        [Parameter(Mandatory = $true)][string]$RunnerSha256,
        [Parameter(Mandatory = $true)][string]$ArmNonce,
        [Parameter(Mandatory = $true)][string]$GoSignalPath,
        [Parameter(Mandatory = $true)][string]$CancelSignalPath,
        [Parameter(Mandatory = $true)]$Readiness,
        [Parameter(Mandatory = $true)][string]$ThreadId,
        [Parameter(Mandatory = $true)][string]$WorkspaceId,
        [Parameter(Mandatory = $true)][string]$TargetId,
        [Parameter(Mandatory = $true)][string]$Endpoint,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$AncestorChain,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$DesktopProcesses
    )

    return [pscustomobject][ordered]@{
        schemaVersion = 1
        gate = "Phase 3.5.2a Gate A3"
        state = "ARMED_WAITING_FOR_GO"
        runnerPid = $RunnerPid
        supervisorPid = if ($SupervisorPid -gt 0) { $SupervisorPid } else { $null }
        runnerStartedAt = $RunnerStartedAt.ToString("o")
        armedAt = $ArmedAt.ToString("o")
        runnerPath = $RunnerPath
        runnerSha256 = $RunnerSha256
        armNonce = $ArmNonce
        goWaitMode = "EXPLICIT_SIGNAL_ONLY"
        goSignalPath = $GoSignalPath
        cancelSignalPath = $CancelSignalPath
        daemonName = [string]$Readiness.daemonName
        daemonMode = [string]$Readiness.daemonMode
        daemonProtocolVersion = [int]$Readiness.daemonProtocolVersion
        daemonHostIdentity = [string]$Readiness.observedRemoteHostIdentity
        workspaceId = $WorkspaceId
        workspaceConnected = [bool]$Readiness.workspaceConnected
        targetId = $TargetId
        targetThreadId = $ThreadId
        endpoint = $Endpoint
        ancestorChain = @($AncestorChain)
        desktopProcessesAtArm = @($DesktopProcesses)
    }
}

function Read-A3BoundControlSignal {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][ValidateSet("GO", "CANCEL")][string]$ExpectedState,
        [Parameter(Mandatory = $true)][int]$ExpectedRunnerPid,
        [Parameter(Mandatory = $true)][string]$ExpectedArmNonce,
        [Parameter(Mandatory = $true)][string]$ExpectedRunnerSha256
    )

    $signal = [System.IO.File]::ReadAllText($Path) | ConvertFrom-Json
    if ([string]$signal.state -ne $ExpectedState -or
        [int]$signal.runnerPid -ne $ExpectedRunnerPid -or
        [string]$signal.armNonce -ne $ExpectedArmNonce -or
        [string]$signal.runnerSha256 -ne $ExpectedRunnerSha256) {
        throw "$ExpectedState signal does not match the current armed runner"
    }
    return $signal
}

function Wait-A3ControlSignal {
    param(
        [Parameter(Mandatory = $true)][string]$GoPath,
        [Parameter(Mandatory = $true)][string]$CancelPath,
        [Parameter(Mandatory = $true)][int]$ExpectedRunnerPid,
        [Parameter(Mandatory = $true)][string]$ExpectedArmNonce,
        [Parameter(Mandatory = $true)][string]$ExpectedRunnerSha256,
        [Parameter(Mandatory = $true)][int]$PollMilliseconds
    )

    while ($true) {
        $goExists = [System.IO.File]::Exists($GoPath)
        $cancelExists = [System.IO.File]::Exists($CancelPath)
        if ($goExists -and $cancelExists) {
            throw "conflicting GO and CANCEL signals"
        }
        if ($cancelExists) {
            $signal = Read-A3BoundControlSignal `
                -Path $CancelPath `
                -ExpectedState "CANCEL" `
                -ExpectedRunnerPid $ExpectedRunnerPid `
                -ExpectedArmNonce $ExpectedArmNonce `
                -ExpectedRunnerSha256 $ExpectedRunnerSha256
            return [pscustomobject]@{
                kind = "CANCEL"
                receivedAt = [DateTimeOffset]::UtcNow
                signalCreatedAt = [string]$signal.createdAt
            }
        }
        if ($goExists) {
            $signal = Read-A3BoundControlSignal `
                -Path $GoPath `
                -ExpectedState "GO" `
                -ExpectedRunnerPid $ExpectedRunnerPid `
                -ExpectedArmNonce $ExpectedArmNonce `
                -ExpectedRunnerSha256 $ExpectedRunnerSha256
            return [pscustomobject]@{
                kind = "GO"
                receivedAt = [DateTimeOffset]::UtcNow
                signalCreatedAt = [string]$signal.createdAt
            }
        }
        Start-Sleep -Milliseconds $PollMilliseconds
    }
}

function New-A3DesktopWaitState {
    param(
        [Parameter(Mandatory = $true)]$ArmedState,
        [Parameter(Mandatory = $true)][DateTimeOffset]$GoReceivedAt,
        [Parameter(Mandatory = $true)][int]$TimeoutSeconds
    )

    $state = [ordered]@{}
    foreach ($property in $ArmedState.PSObject.Properties) {
        $state[$property.Name] = $property.Value
    }
    $state.state = "WAITING_FOR_DESKTOP_EXIT"
    $state.goReceivedAt = $GoReceivedAt.ToString("o")
    $state.desktopExitDeadline = $GoReceivedAt.AddSeconds($TimeoutSeconds).ToString("o")
    $state.desktopExitTimeoutSeconds = $TimeoutSeconds
    return [pscustomobject]$state
}

function Get-A3RemoteConfiguration {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$TargetId,
        [Parameter(Mandatory = $true)][string]$ExpectedRemoteHostIdentity
    )

    $settings = [System.IO.File]::ReadAllText($Path) | ConvertFrom-Json
    if ([string]$settings.backendMode -ne "remote") {
        throw "acceptance settings backendMode is not remote"
    }
    if ([string]$settings.activeRemoteBackendId -ne $TargetId) {
        throw "acceptance settings active Remote target does not match"
    }
    $targets = @($settings.remoteBackends | Where-Object id -eq $TargetId)
    if ($targets.Count -ne 1) {
        throw "expected exactly one configured acceptance target"
    }
    $target = $targets[0]
    if ([string]$target.remoteHostIdentity -ne $ExpectedRemoteHostIdentity) {
        throw "persisted target pin does not match expected Host A identity"
    }
    $token = [string]$target.token
    if (-not $token) {
        $token = [string]$settings.remoteBackendToken
    }
    if (-not $token) {
        throw "acceptance token is unavailable"
    }
    $endpoint = [string]$target.host
    if (-not $endpoint) {
        $endpoint = [string]$settings.remoteBackendHost
    }
    if ($endpoint -notmatch "^([^:]+):(\d+)$") {
        throw "invalid remote endpoint"
    }

    return [pscustomobject]@{
        endpoint = $endpoint
        hostName = $Matches[1]
        port = [int]$Matches[2]
        token = $token
    }
}

function Open-A3DaemonConnection {
    param(
        [Parameter(Mandatory = $true)][string]$HostName,
        [Parameter(Mandatory = $true)][int]$Port
    )

    $tcp = [System.Net.Sockets.TcpClient]::new()
    try {
        $tcp.Connect($HostName, $Port)
        $stream = $tcp.GetStream()
        $writer = [System.IO.StreamWriter]::new($stream, [System.Text.UTF8Encoding]::new($false))
        $writer.AutoFlush = $true
        $reader = [System.IO.StreamReader]::new($stream, [System.Text.UTF8Encoding]::new($false))
        return [pscustomobject]@{
            tcp = $tcp
            writer = $writer
            reader = $reader
        }
    } catch {
        $tcp.Dispose()
        throw
    }
}

function Close-A3DaemonConnection {
    param($Connection)

    if ($null -ne $Connection -and $null -ne $Connection.tcp) {
        $Connection.tcp.Dispose()
    }
}

function New-A3RpcAdapter {
    param([Parameter(Mandatory = $true)]$Connection)

    $invokeDaemonRpc = ${function:Invoke-DaemonRpc}
    return {
        param($Id, $Method, $Params)
        & $invokeDaemonRpc `
            -Id $Id `
            -Method $Method `
            -Params $Params `
            -Writer $Connection.writer `
            -Reader $Connection.reader
    }.GetNewClosure()
}

function New-A3RegressionConnection {
    param([Parameter(Mandatory = $true)][object[]]$Responses)

    $responseLines = @($Responses | ForEach-Object { $_ | ConvertTo-Json -Compress -Depth 12 })
    $responseText = ($responseLines -join "`r`n") + "`r`n"
    $responseBytes = [System.Text.Encoding]::UTF8.GetBytes($responseText)
    $inputStream = [System.IO.MemoryStream]::new($responseBytes)
    $outputStream = [System.IO.MemoryStream]::new()
    $reader = [System.IO.StreamReader]::new($inputStream, [System.Text.UTF8Encoding]::new($false))
    $writer = [System.IO.StreamWriter]::new($outputStream, [System.Text.UTF8Encoding]::new($false))
    $writer.AutoFlush = $true
    return [pscustomobject]@{
        tcp = $null
        reader = $reader
        writer = $writer
        inputStream = $inputStream
        outputStream = $outputStream
    }
}

function Close-A3RegressionConnection {
    param($Connection)

    if ($null -eq $Connection) {
        return
    }
    $Connection.writer.Dispose()
    $Connection.reader.Dispose()
    $Connection.inputStream.Dispose()
    $Connection.outputStream.Dispose()
}

function New-A3RegressionResponses {
    param(
        [Parameter(Mandatory = $true)][string]$Scenario,
        [Parameter(Mandatory = $true)][string]$ExpectedRemoteHostIdentity,
        [Parameter(Mandatory = $true)][string]$WorkspaceId,
        [Parameter(Mandatory = $true)][string]$ThreadId
    )

    $authSuccess = [ordered]@{ id = 1; result = [ordered]@{ ok = $true } }
    $daemonInfo = [ordered]@{
        id = 2
        result = [ordered]@{
            name = "codex-monitor-daemon"
            mode = "tcp"
            protocolVersion = 1
            remoteHostIdentity = $ExpectedRemoteHostIdentity
        }
    }
    $workspaceConnected = [ordered]@{
        id = 3
        result = @([ordered]@{ id = $WorkspaceId; connected = $true })
    }

    switch ($Scenario) {
        "AUTH_FAILURE" {
            return @([ordered]@{ id = 1; error = [ordered]@{ message = "invalid token" } })
        }
        "IDENTITY_MISMATCH" {
            $daemonInfo.result.remoteHostIdentity = "0ae35772-5057-459e-b2b2-f259e492303f"
            return @($authSuccess, $daemonInfo)
        }
        { $_ -in @("WORKSPACE_DISCONNECTED", "FINAL_WORKSPACE_DISCONNECTED") } {
            $workspaceConnected.result[0].connected = $false
            return @($authSuccess, $daemonInfo, $workspaceConnected)
        }
        "FINAL_READY" {
            return @(
                $authSuccess,
                $daemonInfo,
                $workspaceConnected,
                [ordered]@{
                    id = 4
                    result = [ordered]@{ thread = [ordered]@{ id = $ThreadId } }
                }
            )
        }
        default {
            return @($authSuccess, $daemonInfo, $workspaceConnected)
        }
    }
}

function New-A3FullLifecycleRegressionResponses {
    param(
        [Parameter(Mandatory = $true)][string]$Scenario,
        [Parameter(Mandatory = $true)][string]$ExpectedRemoteHostIdentity,
        [Parameter(Mandatory = $true)][string]$WorkspaceId,
        [Parameter(Mandatory = $true)][string]$ThreadId
    )

    if ($Scenario -eq "FULL_LIFECYCLE_AUTH_FAILURE") {
        return @(New-A3RegressionResponses `
            -Scenario "AUTH_FAILURE" `
            -ExpectedRemoteHostIdentity $ExpectedRemoteHostIdentity `
            -WorkspaceId $WorkspaceId `
            -ThreadId $ThreadId)
    }

    $prearm = @(New-A3RegressionResponses `
        -Scenario "READY_NO_GO" `
        -ExpectedRemoteHostIdentity $ExpectedRemoteHostIdentity `
        -WorkspaceId $WorkspaceId `
        -ThreadId $ThreadId)
    $finalScenario = switch ($Scenario) {
        "FULL_LIFECYCLE_FINAL_IDENTITY_MISMATCH" { "IDENTITY_MISMATCH" }
        "FULL_LIFECYCLE_FINAL_WORKSPACE_DISCONNECTED" { "FINAL_WORKSPACE_DISCONNECTED" }
        "FULL_LIFECYCLE_FINAL_READY" { "FINAL_READY" }
        default { throw "unsupported full lifecycle regression scenario: $Scenario" }
    }
    $final = @(New-A3RegressionResponses `
        -Scenario $finalScenario `
        -ExpectedRemoteHostIdentity $ExpectedRemoteHostIdentity `
        -WorkspaceId $WorkspaceId `
        -ThreadId $ThreadId)
    return @($prearm + $final)
}

function Invoke-A3FileScopeRegressionScenario {
    param(
        [Parameter(Mandatory = $true)][string]$Scenario,
        [Parameter(Mandatory = $true)][string]$OutputPath
    )

    $fixtureToken = "file-scope-fixture-token"
    $fixtureThreadId = "01a08c05-7880-75a2-976c-2a5895b58723"
    $fixtureWorkspaceId = "phase-3-5-1d-workspace"
    $fixtureHostIdentity = "014383f2-41f8-4b13-b9d7-30c511e47cec"
    $isFullLifecycle = $Scenario -like "FULL_LIFECYCLE_*"
    $responses = if ($isFullLifecycle) {
        @(New-A3FullLifecycleRegressionResponses `
            -Scenario $Scenario `
            -ExpectedRemoteHostIdentity $fixtureHostIdentity `
            -WorkspaceId $fixtureWorkspaceId `
            -ThreadId $fixtureThreadId)
    } else {
        @(New-A3RegressionResponses `
            -Scenario $Scenario `
            -ExpectedRemoteHostIdentity $fixtureHostIdentity `
            -WorkspaceId $fixtureWorkspaceId `
            -ThreadId $fixtureThreadId)
    }
    $connection = New-A3RegressionConnection -Responses $responses
    $InvokedMethods.Clear()
    $result = [ordered]@{
        scenario = $Scenario
        status = "UNEXPECTED_ERROR"
        diagnostic = $null
        prearmPassed = $false
        armed = $false
        goReceived = $false
        desktopStable = $false
        finalReadinessPassed = $false
        invokedMethods = @()
        resumeRpcCount = 0
        returnedFullThreadId = $null
        threadStartCount = 0
        turnStartCount = 0
        connectWorkspaceCount = 0
        retryCount = 0
        forceTakeover = $false
    }

    try {
        $rpc = New-A3RpcAdapter -Connection $connection
        switch ($Scenario) {
            "CANONICAL_GATE_UNRELATED_GROWTH" {
                $before = [pscustomobject]@{
                    total = 633
                    unique = 633
                    duplicate = 0
                    exactRows = 1
                    codexHomeIdentity = "codex-home:file-scope-regression"
                    fullThreadId = $fixtureThreadId
                    rolloutPath = "C:\fixture\target-rollout.jsonl"
                    turnCount = 1
                }
                $after = [pscustomobject]@{
                    total = 634
                    unique = 634
                    duplicate = 0
                    exactRows = 1
                    codexHomeIdentity = "codex-home:file-scope-regression"
                    fullThreadId = $fixtureThreadId
                    rolloutPath = "C:\fixture\target-rollout.jsonl"
                    turnCount = 1
                }
                $evaluation = Get-A3CanonicalGateEvaluation `
                    -Before $before `
                    -After $after `
                    -ExpectedThreadId $fixtureThreadId
                $failed = @($evaluation.assertions.Values | Where-Object { -not $_ })
                $result.status = if ($failed.Count -eq 0) { "PASS" } else { "ASSERTION_FAILED" }
                $result["canonicalAssertions"] = $evaluation.assertions
                $result["canonicalTelemetry"] = $evaluation.telemetry
            }
            "UTF8_NO_BOM_CANONICAL_SNAPSHOT" {
                $fixtureRoot = [System.IO.Path]::GetDirectoryName($OutputPath)
                $fixtureDatabasePath = Join-Path $fixtureRoot "utf8-no-bom-canonical.sqlite"
                $fixtureRolloutPath = Join-Path $fixtureRoot "utf8-no-bom-rollout.jsonl"
                $utf8NoBom = [System.Text.UTF8Encoding]::new($false)
                $nonAsciiText = ([char]0x4E2D).ToString() + '"quoted"'
                $rolloutRecords = @(
                    ([ordered]@{
                        timestamp = "2026-09-13T00:00:00Z"
                        type = "session_meta"
                        payload = [ordered]@{ id = "utf8-fixture" }
                    } | ConvertTo-Json -Compress -Depth 8),
                    ([ordered]@{
                        timestamp = "2026-09-13T00:00:01Z"
                        type = "turn_context"
                        payload = [ordered]@{ text = $nonAsciiText }
                    } | ConvertTo-Json -Compress -Depth 8)
                )
                [System.IO.File]::WriteAllLines($fixtureRolloutPath, $rolloutRecords, $utf8NoBom)

                if ([System.IO.File]::Exists($fixtureDatabasePath)) {
                    [System.IO.File]::Delete($fixtureDatabasePath)
                }
                $escapedFixtureThreadId = $fixtureThreadId.Replace("'", "''")
                $escapedFixtureRolloutPath = $fixtureRolloutPath.Replace("'", "''")
                $fixtureSql = @"
create table threads (id text not null, rollout_path text);
insert into threads (id, rollout_path) values ('$escapedFixtureThreadId', '$escapedFixtureRolloutPath');
"@
                & $SqlitePath $fixtureDatabasePath $fixtureSql
                if ($LASTEXITCODE -ne 0) {
                    throw "sqlite UTF-8 canonical fixture setup failed with exit code $LASTEXITCODE"
                }

                $previousStateDbPath = $script:StateDbPath
                try {
                    $script:StateDbPath = $fixtureDatabasePath
                    $snapshot = Get-CanonicalSnapshot -ExactThreadId $fixtureThreadId
                } finally {
                    $script:StateDbPath = $previousStateDbPath
                }
                $result.status = "PASS"
                $result["canonicalTotal"] = $snapshot.total
                $result["canonicalUnique"] = $snapshot.unique
                $result["canonicalDuplicate"] = $snapshot.duplicate
                $result["exactRows"] = $snapshot.exactRows
                $result["turnCount"] = $snapshot.turnCount
                $result["rolloutLines"] = $snapshot.rolloutLines
                $result["fixtureRolloutPath"] = $fixtureRolloutPath
            }
            "SHARED_WRITER_CANONICAL_SNAPSHOT" {
                $fixtureRoot = [System.IO.Path]::GetDirectoryName($OutputPath)
                $fixtureDatabasePath = Join-Path $fixtureRoot "shared-writer-canonical.sqlite"
                $fixtureRolloutPath = Join-Path $fixtureRoot "shared-writer-rollout.jsonl"
                $utf8NoBom = [System.Text.UTF8Encoding]::new($false)
                $rolloutRecords = @(
                    ([ordered]@{
                        timestamp = "2026-09-13T00:00:00Z"
                        type = "session_meta"
                        payload = [ordered]@{ id = "shared-writer-fixture" }
                    } | ConvertTo-Json -Compress -Depth 8),
                    ([ordered]@{
                        timestamp = "2026-09-13T00:00:01Z"
                        type = "turn_context"
                        payload = [ordered]@{ text = ([char]0x4E2D).ToString() }
                    } | ConvertTo-Json -Compress -Depth 8)
                )
                [System.IO.File]::WriteAllLines($fixtureRolloutPath, $rolloutRecords, $utf8NoBom)

                if ([System.IO.File]::Exists($fixtureDatabasePath)) {
                    [System.IO.File]::Delete($fixtureDatabasePath)
                }
                $escapedFixtureThreadId = $fixtureThreadId.Replace("'", "''")
                $escapedFixtureRolloutPath = $fixtureRolloutPath.Replace("'", "''")
                $fixtureSql = @"
create table threads (id text not null, rollout_path text);
insert into threads (id, rollout_path) values ('$escapedFixtureThreadId', '$escapedFixtureRolloutPath');
"@
                & $SqlitePath $fixtureDatabasePath $fixtureSql
                if ($LASTEXITCODE -ne 0) {
                    throw "sqlite shared-writer canonical fixture setup failed with exit code $LASTEXITCODE"
                }

                $share = [System.IO.FileShare]::ReadWrite -bor [System.IO.FileShare]::Delete
                $writer = [System.IO.FileStream]::new(
                    $fixtureRolloutPath,
                    [System.IO.FileMode]::Open,
                    [System.IO.FileAccess]::Write,
                    $share
                )
                $previousStateDbPath = $script:StateDbPath
                try {
                    $script:StateDbPath = $fixtureDatabasePath
                    $snapshot = Get-CanonicalSnapshot -ExactThreadId $fixtureThreadId
                } finally {
                    $script:StateDbPath = $previousStateDbPath
                    $writer.Dispose()
                }
                $result.status = "PASS"
                $result["canonicalTotal"] = $snapshot.total
                $result["canonicalUnique"] = $snapshot.unique
                $result["canonicalDuplicate"] = $snapshot.duplicate
                $result["exactRows"] = $snapshot.exactRows
                $result["turnCount"] = $snapshot.turnCount
                $result["rolloutLines"] = $snapshot.rolloutLines
                $result["fixtureRolloutPath"] = $fixtureRolloutPath
            }
            "AUTH_FAILURE" {
                try {
                    Invoke-A3ReadinessSequence -Rpc $rpc -Token $fixtureToken -ExpectedRemoteHostIdentity $fixtureHostIdentity -WorkspaceId $fixtureWorkspaceId | Out-Null
                } catch {
                    if ($_.Exception.Message -ne "daemon authentication failed") { throw }
                    $result.status = "AUTH_FAILURE"
                }
            }
            "IDENTITY_MISMATCH" {
                try {
                    Invoke-A3ReadinessSequence -Rpc $rpc -Token $fixtureToken -ExpectedRemoteHostIdentity $fixtureHostIdentity -WorkspaceId $fixtureWorkspaceId | Out-Null
                } catch {
                    if ($_.Exception.Message -ne "remote host identity mismatch") { throw }
                    $result.status = "IDENTITY_MISMATCH"
                }
            }
            "WORKSPACE_DISCONNECTED" {
                try {
                    Invoke-A3ReadinessSequence -Rpc $rpc -Token $fixtureToken -ExpectedRemoteHostIdentity $fixtureHostIdentity -WorkspaceId $fixtureWorkspaceId | Out-Null
                } catch {
                    if ($_.Exception.Message -ne "acceptance WorkspaceSession is not connected") { throw }
                    $result.status = "WORKSPACE_DISCONNECTED"
                }
            }
            "READY_NO_GO" {
                $readiness = Invoke-A3ReadinessSequence -Rpc $rpc -Token $fixtureToken -ExpectedRemoteHostIdentity $fixtureHostIdentity -WorkspaceId $fixtureWorkspaceId
                $goPath = Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) "missing-go.signal"
                $cancelPath = Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) "missing-cancel.signal"
                $arm = New-A3ArmState -RunnerPid $PID -RunnerStartedAt ([DateTimeOffset]::UtcNow) -ArmedAt ([DateTimeOffset]::UtcNow) -RunnerPath $PSCommandPath -RunnerSha256 "fixture-sha" -ArmNonce "fixture-nonce" -GoSignalPath $goPath -CancelSignalPath $cancelPath -Readiness $readiness -ThreadId $fixtureThreadId -WorkspaceId $fixtureWorkspaceId -TargetId "phase-3-5-1d-target" -Endpoint "127.0.0.1:54060" -AncestorChain @() -DesktopProcesses @()
                $result.status = $arm.state
                $result.armed = $true
                $result.goReceived = $false
            }
            "GO_TRANSITION" {
                $readiness = Invoke-A3ReadinessSequence -Rpc $rpc -Token $fixtureToken -ExpectedRemoteHostIdentity $fixtureHostIdentity -WorkspaceId $fixtureWorkspaceId
                $goPath = Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) "go.signal"
                $cancelPath = Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) "cancel.signal"
                $arm = New-A3ArmState -RunnerPid $PID -RunnerStartedAt ([DateTimeOffset]::UtcNow) -ArmedAt ([DateTimeOffset]::UtcNow) -RunnerPath $PSCommandPath -RunnerSha256 "fixture-sha" -ArmNonce "fixture-nonce" -GoSignalPath $goPath -CancelSignalPath $cancelPath -Readiness $readiness -ThreadId $fixtureThreadId -WorkspaceId $fixtureWorkspaceId -TargetId "phase-3-5-1d-target" -Endpoint "127.0.0.1:54060" -AncestorChain @() -DesktopProcesses @()
                $signal = [ordered]@{ state = "GO"; createdAt = [DateTimeOffset]::UtcNow.ToString("o"); runnerPid = $PID; runnerSha256 = "fixture-sha"; armNonce = "fixture-nonce" }
                [System.IO.File]::WriteAllText($goPath, ($signal | ConvertTo-Json), [System.Text.UTF8Encoding]::new($false))
                $go = Wait-A3ControlSignal -GoPath $goPath -CancelPath $cancelPath -ExpectedRunnerPid $PID -ExpectedArmNonce "fixture-nonce" -ExpectedRunnerSha256 "fixture-sha" -PollMilliseconds 5
                $waiting = New-A3DesktopWaitState -ArmedState $arm -GoReceivedAt $go.receivedAt -TimeoutSeconds 60
                $result.status = $waiting.state
                $result.armed = $true
                $result.goReceived = ($go.kind -eq "GO")
            }
            "FINAL_WORKSPACE_DISCONNECTED" {
                try {
                    Invoke-A3FinalRpcSequence -Rpc $rpc -Token $fixtureToken -ExpectedRemoteHostIdentity $fixtureHostIdentity -WorkspaceId $fixtureWorkspaceId -ThreadId $fixtureThreadId | Out-Null
                } catch {
                    if ($_.Exception.Message -ne "acceptance WorkspaceSession is not connected") { throw }
                    $result.status = "FINAL_WORKSPACE_DISCONNECTED"
                }
            }
            "FINAL_READY" {
                $final = Invoke-A3FinalRpcSequence -Rpc $rpc -Token $fixtureToken -ExpectedRemoteHostIdentity $fixtureHostIdentity -WorkspaceId $fixtureWorkspaceId -ThreadId $fixtureThreadId
                $result.status = "PASS"
                $result.returnedFullThreadId = [string]$final.resumeResponse.result.thread.id
            }
            "FULL_LIFECYCLE_AUTH_FAILURE" {
                try {
                    Invoke-A3ReadinessSequence -Rpc $rpc -Token $fixtureToken -ExpectedRemoteHostIdentity $fixtureHostIdentity -WorkspaceId $fixtureWorkspaceId | Out-Null
                } catch {
                    if ($_.Exception.Message -ne "daemon authentication failed") { throw }
                    $result.status = "PREARM_AUTH_FAILURE"
                }
            }
            { $_ -in @(
                "FULL_LIFECYCLE_FINAL_IDENTITY_MISMATCH",
                "FULL_LIFECYCLE_FINAL_WORKSPACE_DISCONNECTED",
                "FULL_LIFECYCLE_FINAL_READY"
            ) } {
                $prearmReadiness = Invoke-A3ReadinessSequence -Rpc $rpc -Token $fixtureToken -ExpectedRemoteHostIdentity $fixtureHostIdentity -WorkspaceId $fixtureWorkspaceId
                $result.prearmPassed = $true
                $fixtureRoot = [System.IO.Path]::GetDirectoryName($OutputPath)
                $goPath = Join-Path $fixtureRoot "full-lifecycle-go.signal"
                $cancelPath = Join-Path $fixtureRoot "full-lifecycle-cancel.signal"
                $armNonce = "full-lifecycle-nonce"
                $runnerSha = "full-lifecycle-runner-sha"
                $arm = New-A3ArmState `
                    -RunnerPid $PID `
                    -RunnerStartedAt ([DateTimeOffset]::UtcNow) `
                    -ArmedAt ([DateTimeOffset]::UtcNow) `
                    -RunnerPath $PSCommandPath `
                    -RunnerSha256 $runnerSha `
                    -ArmNonce $armNonce `
                    -GoSignalPath $goPath `
                    -CancelSignalPath $cancelPath `
                    -Readiness $prearmReadiness `
                    -ThreadId $fixtureThreadId `
                    -WorkspaceId $fixtureWorkspaceId `
                    -TargetId "phase-3-5-1d-target" `
                    -Endpoint "127.0.0.1:54060" `
                    -AncestorChain @() `
                    -DesktopProcesses @([pscustomobject]@{ pid = 4242; parentPid = 1; name = "ChatGPT.exe"; executablePath = "fixture" })
                $result.armed = ($arm.state -eq "ARMED_WAITING_FOR_GO")
                $signal = [ordered]@{
                    state = "GO"
                    createdAt = [DateTimeOffset]::UtcNow.ToString("o")
                    runnerPid = $PID
                    runnerSha256 = $runnerSha
                    armNonce = $armNonce
                }
                [System.IO.File]::WriteAllText($goPath, ($signal | ConvertTo-Json), [System.Text.UTF8Encoding]::new($false))
                $control = Wait-A3ControlSignal `
                    -GoPath $goPath `
                    -CancelPath $cancelPath `
                    -ExpectedRunnerPid $PID `
                    -ExpectedArmNonce $armNonce `
                    -ExpectedRunnerSha256 $runnerSha `
                    -PollMilliseconds 1
                $result.goReceived = ($control.kind -eq "GO")
                New-A3DesktopWaitState `
                    -ArmedState $arm `
                    -GoReceivedAt ([DateTimeOffset]$control.receivedAt) `
                    -TimeoutSeconds 1 | Out-Null
                $desktopExit = Wait-CodexDesktopExit `
                    -InitialProcesses $arm.desktopProcessesAtArm `
                    -TimeoutSeconds 1 `
                    -StabilityMilliseconds 10 `
                    -PollMilliseconds 1 `
                    -DesktopTreeProvider { @() } `
                    -ProcessInventoryProvider { @() }
                $result.desktopStable = [bool]$desktopExit.exited
                if (-not $result.desktopStable) {
                    throw "simulated Desktop exit did not reach stable absence"
                }
                $admissionState = [ordered]@{
                    observedRemoteHostIdentity = $null
                    workspaceConnectedBeforeResume = $null
                    finalReadinessPassed = $false
                    canonicalBefore = $null
                    canonicalBeforeAssertions = $null
                    noTargetMutationBeforeResume = $null
                    turnCountBefore = $null
                    resumeAttemptCount = 0
                }
                $admissionStage = [pscustomobject]@{ value = "FINAL_READINESS" }
                $snapshotProvider = {
                    param([string]$ExactThreadId)
                    return [pscustomobject]@{
                        total = 1
                        unique = 1
                        duplicate = 0
                        exactRows = if ($ExactThreadId -eq "01a08c05-7880-75a2-976c-2a5895b58723") { 1 } else { 0 }
                        codexHomeIdentity = "codex-home:file-scope-regression"
                        fullThreadId = $ExactThreadId
                        rolloutPath = "C:\fixture\target-rollout.jsonl"
                        turnCount = 1
                    }
                }
                try {
                    $final = Invoke-A3FinalRpcSequence `
                        -Rpc $rpc `
                        -Token $fixtureToken `
                        -ExpectedRemoteHostIdentity $fixtureHostIdentity `
                        -WorkspaceId $fixtureWorkspaceId `
                        -ThreadId $fixtureThreadId `
                        -State $admissionState `
                        -Stage $admissionStage `
                        -CanonicalSnapshotProvider $snapshotProvider
                    $result.status = "PASS"
                    $result.returnedFullThreadId = [string]$final.resumeResponse.result.thread.id
                } catch {
                    if ($Scenario -eq "FULL_LIFECYCLE_FINAL_IDENTITY_MISMATCH" -and $_.Exception.Message -eq "remote host identity mismatch") {
                        $result.status = "FINAL_IDENTITY_MISMATCH"
                    } elseif ($Scenario -eq "FULL_LIFECYCLE_FINAL_WORKSPACE_DISCONNECTED" -and $_.Exception.Message -eq "acceptance WorkspaceSession is not connected") {
                        $result.status = "FINAL_WORKSPACE_DISCONNECTED"
                    } else {
                        throw
                    }
                }
                $result.finalReadinessPassed = [bool]$admissionState.finalReadinessPassed
            }
        }
    } catch {
        $result.diagnostic = $_.Exception.Message
    } finally {
        $result.invokedMethods = @($InvokedMethods)
        $result.resumeRpcCount = @($InvokedMethods | Where-Object { $_ -eq "resume_thread" }).Count
        $result.threadStartCount = @($InvokedMethods | Where-Object { $_ -in @("thread/start", "start_thread") }).Count
        $result.turnStartCount = @($InvokedMethods | Where-Object { $_ -in @("turn/start", "start_turn") }).Count
        $result.connectWorkspaceCount = @($InvokedMethods | Where-Object { $_ -eq "connect_workspace" }).Count
        Close-A3RegressionConnection -Connection $connection
        Write-JsonAtomically -Path $OutputPath -Value $result
    }

    if ($result.status -eq "UNEXPECTED_ERROR") {
        return 1
    }
    return 0
}

function Invoke-A3LifecycleRegressionScenario {
    param(
        [Parameter(Mandatory = $true)][string]$Scenario,
        [Parameter(Mandatory = $true)][string]$Directory,
        [int]$SupervisorPid = 0
    )

    New-Item -ItemType Directory -Path $Directory -Force | Out-Null
    $fixtureResultPath = Join-Path $Directory "result.json"
    $fixtureArmedPath = Join-Path $Directory "armed.json"
    $fixtureGoPath = Join-Path $Directory "go.signal"
    $fixtureCancelPath = Join-Path $Directory "cancel.signal"
    foreach ($path in @($fixtureResultPath, $fixtureArmedPath, $fixtureGoPath, $fixtureCancelPath)) {
        Remove-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
    }

    if ($Scenario -eq "UNHANDLED_FAILURE") {
        Write-Host "A3 lifecycle fixture is simulating an unhandled runner failure."
        throw "simulated unhandled runner failure"
    }

    $runnerPath = [System.IO.Path]::GetFullPath($PSCommandPath)
    $runnerSha256 = (Get-FileHash -LiteralPath $runnerPath -Algorithm SHA256).Hash
    $armedAt = [DateTimeOffset]::UtcNow
    $armNonce = [guid]::NewGuid().ToString("N")
    $readiness = [pscustomobject]@{
        daemonName = "codex-monitor-daemon"
        daemonMode = "tcp"
        daemonProtocolVersion = 1
        observedRemoteHostIdentity = "014383f2-41f8-4b13-b9d7-30c511e47cec"
        workspaceConnected = $true
    }
    $armed = New-A3ArmState `
        -RunnerPid $PID `
        -SupervisorPid $SupervisorPid `
        -RunnerStartedAt $RunnerStartedAt `
        -ArmedAt $armedAt `
        -RunnerPath $runnerPath `
        -RunnerSha256 $runnerSha256 `
        -ArmNonce $armNonce `
        -GoSignalPath $fixtureGoPath `
        -CancelSignalPath $fixtureCancelPath `
        -Readiness $readiness `
        -ThreadId $ThreadId `
        -WorkspaceId $WorkspaceId `
        -TargetId $TargetId `
        -Endpoint "127.0.0.1:54060" `
        -AncestorChain @() `
        -DesktopProcesses @()
    Write-JsonAtomically -Path $fixtureArmedPath -Value $armed
    Write-Host "A3 RUNNER ARMED_WAITING_FOR_GO"

    $control = Wait-A3ControlSignal `
        -GoPath $fixtureGoPath `
        -CancelPath $fixtureCancelPath `
        -ExpectedRunnerPid $PID `
        -ExpectedArmNonce $armNonce `
        -ExpectedRunnerSha256 $runnerSha256 `
        -PollMilliseconds $GoSignalPollMilliseconds

    $result = [ordered]@{
        schemaVersion = 1
        gate = "Phase 3.5.2a Gate A3 lifecycle regression"
        status = if ($control.kind -eq "CANCEL") { "CANCELLED_BEFORE_GO" } else { "WAITING_FOR_DESKTOP_EXIT" }
        runnerPid = $PID
        supervisorPid = if ($SupervisorPid -gt 0) { $SupervisorPid } else { $null }
        runnerStartedAt = $RunnerStartedAt.ToString("o")
        armedAt = $armedAt.ToString("o")
        completedAt = [DateTimeOffset]::UtcNow.ToString("o")
        controlSignal = $control.kind
        controlSignalCreatedAt = $control.signalCreatedAt
        resumeAttemptCount = 0
        resumeRpcCount = 0
        threadStartCount = 0
        turnStartCount = 0
        connectWorkspaceCount = 0
        rpcRetryCount = 0
        forceTakeover = $false
    }
    if ($control.kind -eq "GO") {
        $waiting = New-A3DesktopWaitState `
            -ArmedState $armed `
            -GoReceivedAt ([DateTimeOffset]$control.receivedAt) `
            -TimeoutSeconds $DesktopExitTimeoutSeconds
        Write-JsonAtomically -Path $fixtureArmedPath -Value $waiting
    }
    Write-JsonAtomically -Path $fixtureResultPath -Value $result
    return 0
}

if ($FileScopeRegressionScenario) {
    if ([string]::IsNullOrWhiteSpace($FileScopeRegressionOutputPath)) {
        throw "FileScopeRegressionOutputPath is required"
    }
    $regressionExitCode = Invoke-A3FileScopeRegressionScenario `
        -Scenario $FileScopeRegressionScenario `
        -OutputPath $FileScopeRegressionOutputPath
    exit $regressionExitCode
}

if ($LifecycleRegressionScenario) {
    $lifecycleExitCode = Invoke-A3LifecycleRegressionScenario `
        -Scenario $LifecycleRegressionScenario `
        -Directory $ResultDirectory `
        -SupervisorPid $SupervisorPid
    exit $lifecycleExitCode
}

if ($LibraryOnly) {
    return
}

New-Item -ItemType Directory -Path $ResultDirectory -Force | Out-Null
Remove-Item -LiteralPath $ResultPath -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $ArmedPath -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $GoSignalPath -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $CancelSignalPath -Force -ErrorAction SilentlyContinue

$runnerPath = [System.IO.Path]::GetFullPath($PSCommandPath)
$runnerSha256 = (Get-FileHash -LiteralPath $runnerPath -Algorithm SHA256).Hash
$ancestorChain = Get-RunnerAncestorChain
$desktopDependent = Test-DesktopDependentAncestor -Chain $ancestorChain
if ($desktopDependent) {
    $blockedResult = [ordered]@{
        schemaVersion = 1
        gate = "Phase 3.5.2a Gate A3"
        status = "NOT_ARMED_DESKTOP_DEPENDENT"
        runnerPid = $PID
        runnerStartedAt = $RunnerStartedAt.ToString("o")
        runnerSha256 = $runnerSha256
        ancestorChain = $ancestorChain
        diagnostic = "Runner ancestor chain includes Codex Desktop or its command runner; no RPC was sent."
        invokedMethods = @()
        resumeAttemptCount = 0
        rpcRetryCount = 0
    }
    Write-JsonAtomically -Path $ResultPath -Value $blockedResult
    Write-Host "A3 runner refused to arm: launch it from a standalone Windows PowerShell/Terminal."
    exit 3
}

$desktopProcessesAtArm = @(Get-CodexDesktopProcessTree)
if ($desktopProcessesAtArm.Count -eq 0) {
    $blockedResult = [ordered]@{
        schemaVersion = 1
        gate = "Phase 3.5.2a Gate A3"
        status = "DESKTOP_NOT_RUNNING_AT_ARM"
        runnerPid = $PID
        runnerStartedAt = $RunnerStartedAt.ToString("o")
        runnerSha256 = $runnerSha256
        ancestorChain = $ancestorChain
        diagnostic = "Codex Desktop was not running when the runner attempted to arm; no RPC was sent."
        invokedMethods = @()
        resumeAttemptCount = 0
        rpcRetryCount = 0
        threadStartFallbackCalled = $false
        turnStartCalled = $false
    }
    Write-JsonAtomically -Path $ResultPath -Value $blockedResult
    Write-Host "A3 runner refused to arm: Codex Desktop is not running."
    exit 4
}

$remoteConfiguration = $null
$prearmConnection = $null
$prearmReadiness = $null
try {
    $remoteConfiguration = Get-A3RemoteConfiguration `
        -Path $SettingsPath `
        -TargetId $TargetId `
        -ExpectedRemoteHostIdentity $ExpectedRemoteHostIdentity
    $prearmConnection = Open-A3DaemonConnection `
        -HostName $remoteConfiguration.hostName `
        -Port $remoteConfiguration.port
    $prearmRpc = New-A3RpcAdapter -Connection $prearmConnection
    $prearmReadiness = Invoke-A3ReadinessSequence `
        -Rpc $prearmRpc `
        -Token $remoteConfiguration.token `
        -ExpectedRemoteHostIdentity $ExpectedRemoteHostIdentity `
        -WorkspaceId $WorkspaceId
} catch {
    $blockedResult = [ordered]@{
        schemaVersion = 1
        gate = "Phase 3.5.2a Gate A3"
        status = "PREARM_READINESS_FAILED"
        runnerPid = $PID
        runnerStartedAt = $RunnerStartedAt.ToString("o")
        runnerSha256 = $runnerSha256
        ancestorChain = $ancestorChain
        diagnostic = $_.Exception.Message
        invokedMethods = @($InvokedMethods)
        resumeAttemptCount = 0
        rpcRetryCount = 0
        threadStartFallbackCalled = $false
        turnStartCalled = $false
    }
    Write-JsonAtomically -Path $ResultPath -Value $blockedResult
    Write-Host "A3 runner refused to arm: authenticated Remote readiness failed."
    exit 6
} finally {
    Close-A3DaemonConnection -Connection $prearmConnection
}

$armedAt = [DateTimeOffset]::UtcNow
$armNonce = [guid]::NewGuid().ToString("N")
$armed = New-A3ArmState `
    -RunnerPid $PID `
    -SupervisorPid $SupervisorPid `
    -RunnerStartedAt $RunnerStartedAt `
    -ArmedAt $armedAt `
    -RunnerPath $runnerPath `
    -RunnerSha256 $runnerSha256 `
    -ArmNonce $armNonce `
    -GoSignalPath $GoSignalPath `
    -CancelSignalPath $CancelSignalPath `
    -Readiness $prearmReadiness `
    -ThreadId $ThreadId `
    -WorkspaceId $WorkspaceId `
    -TargetId $TargetId `
    -Endpoint $remoteConfiguration.endpoint `
    -AncestorChain $ancestorChain `
    -DesktopProcesses $desktopProcessesAtArm
Write-JsonAtomically -Path $ArmedPath -Value $armed

Write-Host "A3 RUNNER ARMED_WAITING_FOR_GO"
Write-Host "Runner PID: $PID"
Write-Host "Armed evidence: $ArmedPath"
Write-Host "No Desktop-exit timeout or resume RPC starts until a matching one-shot GO signal is received."

try {
    $control = Wait-A3ControlSignal `
        -GoPath $GoSignalPath `
        -CancelPath $CancelSignalPath `
        -ExpectedRunnerPid $PID `
        -ExpectedArmNonce $armNonce `
        -ExpectedRunnerSha256 $runnerSha256 `
        -PollMilliseconds $GoSignalPollMilliseconds
} catch {
    $controlFailure = [ordered]@{
        schemaVersion = 1
        gate = "Phase 3.5.2a Gate A3"
        status = "CONTROL_SIGNAL_INVALID"
        runnerPid = $PID
        runnerStartedAt = $RunnerStartedAt.ToString("o")
        armedAt = $armedAt.ToString("o")
        completedAt = [DateTimeOffset]::UtcNow.ToString("o")
        runnerSha256 = $runnerSha256
        diagnostic = $_.Exception.Message
        invokedMethods = @($InvokedMethods)
        resumeAttemptCount = 0
        rpcRetryCount = 0
        threadStartFallbackCalled = $false
        turnStartCalled = $false
    }
    Write-JsonAtomically -Path $ResultPath -Value $controlFailure
    Write-Host "A3 runner rejected the control signal; no resume RPC was sent."
    exit 8
}

if ($control.kind -eq "CANCEL") {
    $cancelled = [ordered]@{
        schemaVersion = 1
        gate = "Phase 3.5.2a Gate A3"
        status = "CANCELLED_BEFORE_GO"
        runnerPid = $PID
        supervisorPid = if ($SupervisorPid -gt 0) { $SupervisorPid } else { $null }
        runnerStartedAt = $RunnerStartedAt.ToString("o")
        armedAt = $armedAt.ToString("o")
        completedAt = [DateTimeOffset]::UtcNow.ToString("o")
        runnerSha256 = $runnerSha256
        cancelSignalCreatedAt = $control.signalCreatedAt
        diagnostic = "Explicit CANCEL was received; no Desktop-exit timeout or resume RPC was started."
        invokedMethods = @($InvokedMethods)
        resumeAttemptCount = 0
        resumeRpcCount = 0
        rpcRetryCount = 0
        threadStartFallbackCalled = $false
        turnStartCalled = $false
        connectWorkspaceCalled = $false
        forceTakeover = $false
    }
    Write-JsonAtomically -Path $ResultPath -Value $cancelled
    Write-Host "A3 runner cancelled before GO; no resume RPC was sent."
    exit 0
}

$go = $control
$goReceivedAt = [DateTimeOffset]$go.receivedAt
$waitingState = New-A3DesktopWaitState `
    -ArmedState $armed `
    -GoReceivedAt $goReceivedAt `
    -TimeoutSeconds $DesktopExitTimeoutSeconds
$waitingState | Add-Member -NotePropertyName goSignalCreatedAt -NotePropertyValue $go.signalCreatedAt
$waitingState | Add-Member -NotePropertyName desktopExitStabilityMilliseconds -NotePropertyValue $DesktopExitStabilityMilliseconds
$waitingState | Add-Member -NotePropertyName desktopExitPollMilliseconds -NotePropertyValue $DesktopExitPollMilliseconds
Write-JsonAtomically -Path $ArmedPath -Value $waitingState

Write-Host "A3 RUNNER WAITING_FOR_DESKTOP_EXIT"
Write-Host "GO received. Exit Codex Desktop within $DesktopExitTimeoutSeconds seconds; the runner then requires a $DesktopExitStabilityMilliseconds ms stable absence."

$desktopExit = Wait-CodexDesktopExit `
    -InitialProcesses $desktopProcessesAtArm `
    -TimeoutSeconds $DesktopExitTimeoutSeconds `
    -StabilityMilliseconds $DesktopExitStabilityMilliseconds `
    -PollMilliseconds $DesktopExitPollMilliseconds

if (-not $desktopExit.exited) {
    $timeoutResult = [ordered]@{
        schemaVersion = 1
        gate = "Phase 3.5.2a Gate A3"
        status = "DESKTOP_EXIT_TIMEOUT"
        runnerPid = $PID
        runnerStartedAt = $RunnerStartedAt.ToString("o")
        armedAt = $armedAt.ToString("o")
        goReceivedAt = $goReceivedAt.ToString("o")
        completedAt = [DateTimeOffset]::UtcNow.ToString("o")
        desktopExitTimeoutSeconds = $DesktopExitTimeoutSeconds
        desktopExitStabilityMilliseconds = $DesktopExitStabilityMilliseconds
        desktopProcessesAtArm = $desktopProcessesAtArm
        activeDesktopProcessesAtTimeout = $desktopExit.activeProcesses
        diagnostic = "Codex Desktop process tree remained active through the bounded post-GO exit timeout; no resume RPC was sent."
        invokedMethods = @($InvokedMethods)
        resumeAttemptCount = 0
        rpcRetryCount = 0
        threadStartFallbackCalled = $false
        turnStartCalled = $false
    }
    Write-JsonAtomically -Path $ResultPath -Value $timeoutResult
    Write-Host "A3 runner timed out waiting for Codex Desktop to exit; no resume RPC was sent."
    exit 5
}

$finalConnection = $null
$before = $null
$after = $null
$stage = [pscustomobject]@{ value = "FINAL_READINESS" }
$result = [ordered]@{
    schemaVersion = 1
    gate = "Phase 3.5.2a Gate A3"
    status = "FAILED"
    runnerPid = $PID
    runnerStartedAt = $RunnerStartedAt.ToString("o")
    runnerSha256 = $runnerSha256
    armedAt = $armedAt.ToString("o")
    goSignalCreatedAt = $go.signalCreatedAt
    goReceivedAt = $goReceivedAt.ToString("o")
    executedAt = [DateTimeOffset]::UtcNow.ToString("o")
    desktopExitTimeoutSeconds = $DesktopExitTimeoutSeconds
    desktopExitStabilityMilliseconds = $DesktopExitStabilityMilliseconds
    desktopProcessesAtArm = $desktopProcessesAtArm
    desktopExitObservedAt = $desktopExit.exitObservedAt
    desktopStableAt = $desktopExit.stableAt
    ancestorChain = $ancestorChain
    targetId = $TargetId
    workspaceId = $WorkspaceId
    requestedThreadId = $ThreadId
    expectedRemoteHostIdentity = $ExpectedRemoteHostIdentity
    observedRemoteHostIdentity = $null
    endpoint = $remoteConfiguration.endpoint
    workspaceConnectedBeforeResume = $null
    finalReadinessPassed = $false
    resumeResponseFullThreadId = $null
    resumeErrorCode = $null
    resumeErrorMessage = $null
    resumeErrorKind = $null
    invokedMethods = @()
    threadStartFallbackCalled = $false
    turnStartCalled = $false
    connectWorkspaceCalled = $false
    resumeAttemptCount = 0
    rpcRetryCount = 0
    forceTakeover = $false
    canonicalBefore = $null
    canonicalAfter = $null
    canonicalBeforeAssertions = $null
    noTargetMutationBeforeResume = $null
    canonicalTelemetry = $null
    turnCountBefore = $null
    turnCountAfter = $null
    assertions = $null
    diagnostic = $null
}

try {
    if (-not (Test-Path -LiteralPath $StateDbPath)) {
        throw "canonical state database not found"
    }
    if (-not (Test-Path -LiteralPath $SqlitePath)) {
        throw "sqlite executable not found"
    }

    $finalConnection = Open-A3DaemonConnection `
        -HostName $remoteConfiguration.hostName `
        -Port $remoteConfiguration.port
    $finalRpc = New-A3RpcAdapter -Connection $finalConnection
    $getCanonicalSnapshot = ${function:Get-CanonicalSnapshot}
    $finalSequence = Invoke-A3FinalRpcSequence `
        -Rpc $finalRpc `
        -Token $remoteConfiguration.token `
        -ExpectedRemoteHostIdentity $ExpectedRemoteHostIdentity `
        -WorkspaceId $WorkspaceId `
        -ThreadId $ThreadId `
        -State $result `
        -Stage $stage `
        -CanonicalSnapshotProvider $getCanonicalSnapshot
    $finalReadiness = $finalSequence.readiness
    $resume = $finalSequence.resumeResponse
    $before = $finalSequence.canonicalBefore

    $resumeError = if ($resume.error) { $resume.error } else { $resume.result.error }
    if ($resumeError) {
        $result.resumeErrorCode = $resumeError.code
        $result.resumeErrorMessage = [string]$resumeError.message
        $result.resumeErrorKind = [string]$resumeError.data.codexMonitorKind
    } else {
        $responseThread = if ($resume.result.result.thread) {
            $resume.result.result.thread
        } elseif ($resume.result.thread) {
            $resume.result.thread
        } else {
            $null
        }
        $result.resumeResponseFullThreadId = [string]$responseThread.id
    }

    $stage.value = "CANONICAL_AFTER"
    Start-Sleep -Milliseconds 500
    $after = Get-CanonicalSnapshot -ExactThreadId $ThreadId
    $result.canonicalAfter = $after
    $result.turnCountAfter = $after.turnCount

    $result.invokedMethods = @($InvokedMethods)
    $result.threadStartFallbackCalled = [bool](@($InvokedMethods | Where-Object {
        $_ -in @("start_thread", "thread/start")
    }).Count)
    $result.turnStartCalled = [bool](@($InvokedMethods | Where-Object {
        $_ -in @("start_turn", "turn/start")
    }).Count)
    $result.connectWorkspaceCalled = [bool](@($InvokedMethods | Where-Object {
        $_ -eq "connect_workspace"
    }).Count)
    $resumeRpcCount = @($InvokedMethods | Where-Object { $_ -eq "resume_thread" }).Count

    $canonicalGate = Get-A3CanonicalGateEvaluation `
        -Before $before `
        -After $after `
        -ExpectedThreadId $ThreadId
    $assertions = [ordered]@{
        exactThreadIdMatched = ($result.resumeResponseFullThreadId -eq $ThreadId)
        noThreadStartFallback = (-not $result.threadStartFallbackCalled)
        noTurnStart = (-not $result.turnStartCalled)
        noConnectWorkspace = (-not $result.connectWorkspaceCalled)
        exactlyOneResumeAttempt = ($result.resumeAttemptCount -eq 1 -and $resumeRpcCount -eq 1)
        noRpcRetry = ($result.rpcRetryCount -eq 0)
        noForceTakeover = (-not $result.forceTakeover)
        noTargetMutationBeforeResume = [bool]$result.noTargetMutationBeforeResume
    }
    foreach ($entry in $canonicalGate.assertions.GetEnumerator()) {
        $assertions[$entry.Key] = $entry.Value
    }
    $result.assertions = $assertions
    $result.canonicalTelemetry = $canonicalGate.telemetry

    if ($resumeError) {
        if ($result.resumeErrorKind -eq "BLOCKED_BY_ACTIVE_WRITER" -or
            ($result.resumeErrorCode -eq -32600 -and $result.resumeErrorMessage -match "active writer")) {
            $result.status = "BLOCKED_BY_ACTIVE_WRITER"
        } else {
            $result.status = "RESUME_FAILED"
        }
    } elseif (@($assertions.Values | Where-Object { -not $_ }).Count -eq 0) {
        $result.status = "PASS"
    } else {
        $result.status = "ASSERTION_FAILED"
    }
} catch {
    if ($stage.value -eq "FINAL_READINESS") {
        $result.status = "FINAL_READINESS_FAILED"
    } elseif ($stage.value -eq "CANONICAL_BEFORE") {
        $result.status = "CANONICAL_BEFORE_FAILED"
    }
    $result.diagnostic = $_.Exception.Message
} finally {
    Close-A3DaemonConnection -Connection $finalConnection
    $result.invokedMethods = @($InvokedMethods)
    $result.threadStartFallbackCalled = [bool](@($InvokedMethods | Where-Object {
        $_ -in @("start_thread", "thread/start")
    }).Count)
    $result.turnStartCalled = [bool](@($InvokedMethods | Where-Object {
        $_ -in @("start_turn", "turn/start")
    }).Count)
    $result.connectWorkspaceCalled = [bool](@($InvokedMethods | Where-Object {
        $_ -eq "connect_workspace"
    }).Count)
    $result.completedAt = [DateTimeOffset]::UtcNow.ToString("o")
    Write-JsonAtomically -Path $ResultPath -Value $result
}

Write-Host "A3 runner completed with status: $($result.status)"
Write-Host "Result: $ResultPath"
if ($result.status -eq "PASS") {
    exit 0
}
exit 2
