param(
    [string]$RunnerPath = "F:\AI\CodexMonitor\.worktrees\phase-3-5-2a-remote-read-admission\run-phase-3-5-2a-a3.ps1",
    [string]$ResultDirectory = "F:\AI\CodexMonitor\.worktrees\phase-3-5-2a-remote-read-admission\src-tauri\target\phase-3-5-2a-a3",
    [ValidateSet("ARMED_CONTROL", "UNHANDLED_FAILURE")]
    [string]$RunnerLifecycleRegressionScenario
)

$ErrorActionPreference = "Stop"
$supervisorStartedAt = [DateTimeOffset]::UtcNow

function Write-A3JsonAtomically {
    param([string]$Path, $Value)

    $json = $Value | ConvertTo-Json -Depth 20
    $tempPath = "$Path.tmp-$PID-$([guid]::NewGuid().ToString('N'))"
    $backupPath = "$Path.replace-backup-$PID-$([guid]::NewGuid().ToString('N'))"
    try {
        [System.IO.File]::WriteAllText($tempPath, $json, [System.Text.UTF8Encoding]::new($false))
        if ([System.IO.File]::Exists($Path)) {
            [System.IO.File]::Replace($tempPath, $Path, $backupPath)
            [System.IO.File]::Delete($backupPath)
        } else {
            [System.IO.File]::Move($tempPath, $Path)
        }
    } finally {
        if ([System.IO.File]::Exists($tempPath)) { [System.IO.File]::Delete($tempPath) }
        if ([System.IO.File]::Exists($backupPath)) { [System.IO.File]::Delete($backupPath) }
    }
}

function Get-A3AncestorChain {
    $chain = [System.Collections.Generic.List[object]]::new()
    $currentPid = $PID
    for ($index = 0; $index -lt 16 -and $currentPid -gt 0; $index += 1) {
        $process = Get-CimInstance Win32_Process -Filter "ProcessId=$currentPid"
        if ($null -eq $process) { break }
        $chain.Add([pscustomobject]@{
            pid = [int]$process.ProcessId
            parentPid = [int]$process.ParentProcessId
            name = [string]$process.Name
            executablePath = [string]$process.ExecutablePath
        })
        if ($process.ParentProcessId -le 0 -or $process.ParentProcessId -eq $currentPid) { break }
        $currentPid = [int]$process.ParentProcessId
    }
    return @($chain)
}

function Test-A3DesktopDependentAncestor {
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

function ConvertTo-A3ProcessArgument {
    param([AllowEmptyString()][string]$Argument)

    if ($Argument.Length -gt 0 -and $Argument -notmatch '[\s"]') {
        return $Argument
    }

    $builder = [System.Text.StringBuilder]::new()
    [void]$builder.Append('"')
    $backslashCount = 0
    foreach ($character in $Argument.ToCharArray()) {
        if ($character -eq '\') {
            $backslashCount += 1
            continue
        }
        if ($character -eq '"') {
            [void]$builder.Append(('\' * (($backslashCount * 2) + 1)))
            [void]$builder.Append('"')
            $backslashCount = 0
            continue
        }
        if ($backslashCount -gt 0) {
            [void]$builder.Append(('\' * $backslashCount))
            $backslashCount = 0
        }
        [void]$builder.Append($character)
    }
    if ($backslashCount -gt 0) {
        [void]$builder.Append(('\' * ($backslashCount * 2)))
    }
    [void]$builder.Append('"')
    return $builder.ToString()
}

function Preserve-A3StaleEvidence {
    param([string]$Directory)

    $names = @(
        "armed.json",
        "go.signal",
        "cancel.signal",
        "result.json",
        "runner-process.json",
        "runner.stdout.log",
        "runner.stderr.log"
    )
    $existing = @($names | Where-Object { [System.IO.File]::Exists((Join-Path $Directory $_)) })
    if ($existing.Count -eq 0) { return $null }

    $archiveId = "prior-{0}-{1}" -f ([DateTimeOffset]::UtcNow.ToString("yyyyMMddTHHmmssfffZ")), ([guid]::NewGuid().ToString("N").Substring(0, 8))
    $archiveDirectory = Join-Path (Join-Path $Directory "history") $archiveId
    [System.IO.Directory]::CreateDirectory($archiveDirectory) | Out-Null
    foreach ($name in $existing) {
        [System.IO.File]::Copy((Join-Path $Directory $name), (Join-Path $archiveDirectory $name), $false)
    }
    return $archiveDirectory
}

$resolvedRunnerPath = [System.IO.Path]::GetFullPath($RunnerPath)
if (-not [System.IO.File]::Exists($resolvedRunnerPath)) {
    throw "A3 runner script is unavailable"
}
$runnerSha256 = (Get-FileHash -LiteralPath $resolvedRunnerPath -Algorithm SHA256).Hash
$ancestorChain = if ($RunnerLifecycleRegressionScenario) { @() } else { @(Get-A3AncestorChain) }
if (Test-A3DesktopDependentAncestor -Chain $ancestorChain) {
    throw "A3 supervisor must run outside the Codex Desktop lifecycle"
}

[System.IO.Directory]::CreateDirectory($ResultDirectory) | Out-Null
$armedPath = Join-Path $ResultDirectory "armed.json"
if ([System.IO.File]::Exists($armedPath)) {
    $priorArmed = [System.IO.File]::ReadAllText($armedPath) | ConvertFrom-Json
    if ($null -ne (Get-Process -Id ([int]$priorArmed.runnerPid) -ErrorAction SilentlyContinue)) {
        throw "an A3 runner is already active"
    }
}
$archiveDirectory = Preserve-A3StaleEvidence -Directory $ResultDirectory

$processPath = Join-Path $ResultDirectory "runner-process.json"
$stdoutPath = Join-Path $ResultDirectory "runner.stdout.log"
$stderrPath = Join-Path $ResultDirectory "runner.stderr.log"
foreach ($path in @($processPath, $stdoutPath, $stderrPath)) {
    if ([System.IO.File]::Exists($path)) { [System.IO.File]::Delete($path) }
}

$powershellPath = (Get-Process -Id $PID).Path
$arguments = [System.Collections.Generic.List[string]]::new()
foreach ($argument in @(
    "-NoProfile",
    "-ExecutionPolicy", "Bypass",
    "-File", $resolvedRunnerPath,
    "-ResultDirectory", $ResultDirectory,
    "-SupervisorPid", [string]$PID
)) {
    $arguments.Add([string]$argument)
}
if ($RunnerLifecycleRegressionScenario) {
    $arguments.Add("-LifecycleRegressionScenario")
    $arguments.Add($RunnerLifecycleRegressionScenario)
}

$startInfo = [System.Diagnostics.ProcessStartInfo]::new()
$startInfo.FileName = $powershellPath
$startInfo.Arguments = ((@($arguments) | ForEach-Object { ConvertTo-A3ProcessArgument -Argument $_ }) -join " ")
$startInfo.WorkingDirectory = (Get-Location).ProviderPath
$startInfo.UseShellExecute = $false
$startInfo.CreateNoWindow = $true
$startInfo.RedirectStandardOutput = $true
$startInfo.RedirectStandardError = $true
$runnerProcess = [System.Diagnostics.Process]::new()
$runnerProcess.StartInfo = $startInfo
$stdoutStream = [System.IO.FileStream]::new(
    $stdoutPath,
    [System.IO.FileMode]::Create,
    [System.IO.FileAccess]::Write,
    [System.IO.FileShare]::ReadWrite
)
$stderrStream = [System.IO.FileStream]::new(
    $stderrPath,
    [System.IO.FileMode]::Create,
    [System.IO.FileAccess]::Write,
    [System.IO.FileShare]::ReadWrite
)
try {
    [void]$runnerProcess.Start()
    $stdoutCopyTask = $runnerProcess.StandardOutput.BaseStream.CopyToAsync($stdoutStream)
    $stderrCopyTask = $runnerProcess.StandardError.BaseStream.CopyToAsync($stderrStream)
} catch {
    $stdoutStream.Dispose()
    $stderrStream.Dispose()
    $runnerProcess.Dispose()
    throw
}
$runnerStartedAt = [DateTimeOffset]::UtcNow
$runningEvidence = [ordered]@{
    schemaVersion = 1
    gate = "Phase 3.5.2a Gate A3 runner process supervision"
    state = "RUNNING"
    supervisorPid = $PID
    supervisorStartedAt = $supervisorStartedAt.ToString("o")
    supervisorAncestorChain = $ancestorChain
    runnerPid = $runnerProcess.Id
    runnerStartedAt = $runnerStartedAt.ToString("o")
    runnerPath = $resolvedRunnerPath
    runnerSha256 = $runnerSha256
    powershellPath = $powershellPath
    stdoutPath = $stdoutPath
    stderrPath = $stderrPath
    staleEvidenceArchive = $archiveDirectory
    resultJsonObserved = $false
    terminationClassification = $null
    exitCode = $null
    exitedAt = $null
    processLifetimeMilliseconds = $null
}
Write-A3JsonAtomically -Path $processPath -Value $runningEvidence

Write-Host "A3 supervisor started runner PID $($runnerProcess.Id)."
Write-Host "Runner process evidence: $processPath"
try {
    $runnerProcess.WaitForExit()
    $stdoutCopyTask.GetAwaiter().GetResult()
    $stderrCopyTask.GetAwaiter().GetResult()
    $stdoutStream.Flush()
    $stderrStream.Flush()
} finally {
    $stdoutStream.Dispose()
    $stderrStream.Dispose()
}
$exitedAt = [DateTimeOffset]::UtcNow
$resultPath = Join-Path $ResultDirectory "result.json"
$resultJsonObserved = $false
if ([System.IO.File]::Exists($resultPath)) {
    try {
        $result = [System.IO.File]::ReadAllText($resultPath) | ConvertFrom-Json
        $resultJsonObserved = ([int]$result.runnerPid -eq $runnerProcess.Id)
    } catch {
        $resultJsonObserved = $false
    }
}

$completedEvidence = [ordered]@{
    schemaVersion = 1
    gate = "Phase 3.5.2a Gate A3 runner process supervision"
    state = "EXITED"
    supervisorPid = $PID
    supervisorStartedAt = $supervisorStartedAt.ToString("o")
    supervisorAncestorChain = $ancestorChain
    runnerPid = $runnerProcess.Id
    runnerStartedAt = $runnerStartedAt.ToString("o")
    runnerPath = $resolvedRunnerPath
    runnerSha256 = $runnerSha256
    powershellPath = $powershellPath
    stdoutPath = $stdoutPath
    stderrPath = $stderrPath
    staleEvidenceArchive = $archiveDirectory
    resultJsonObserved = $resultJsonObserved
    terminationClassification = if ($resultJsonObserved) { "RUNNER_RESULT_OBSERVED" } else { "UNATTRIBUTED_PROCESS_EXIT" }
    exitCode = $runnerProcess.ExitCode
    exitedAt = $exitedAt.ToString("o")
    processLifetimeMilliseconds = [math]::Round(($exitedAt - $runnerStartedAt).TotalMilliseconds)
}
Write-A3JsonAtomically -Path $processPath -Value $completedEvidence

Write-Host "A3 runner exited with code $($runnerProcess.ExitCode)."
Write-Host "Termination classification: $($completedEvidence.terminationClassification)"
exit $runnerProcess.ExitCode
