param(
    [Parameter(Mandatory = $true)][string]$RunnerPath,
    [Parameter(Mandatory = $true)][string]$SignalHelperPath,
    [Parameter(Mandatory = $true)][string]$CancelHelperPath
)

$ErrorActionPreference = "Stop"
$script:Passed = 0
$script:Failed = 0

function Assert-True {
    param(
        [Parameter(Mandatory = $true)][bool]$Condition,
        [Parameter(Mandatory = $true)][string]$Name
    )

    if ($Condition) {
        $script:Passed += 1
        Write-Host "PASS $Name"
        return
    }

    $script:Failed += 1
    Write-Host "FAIL $Name"
}

function Assert-Equal {
    param(
        $Actual,
        $Expected,
        [Parameter(Mandatory = $true)][string]$Name
    )

    Assert-True -Condition ($Actual -eq $Expected) -Name $Name
}

$resultRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("phase-3-5-2a-a3-library-{0}" -f [guid]::NewGuid())
try {
    New-Item -ItemType Directory -Path $resultRoot | Out-Null
    $powershellPath = (Get-Process -Id $PID).Path
    $process = Start-Process `
        -FilePath $powershellPath `
        -ArgumentList @(
            "-NoProfile",
            "-ExecutionPolicy", "Bypass",
            "-File", $RunnerPath,
            "-LibraryOnly",
            "-ResultDirectory", $resultRoot
        ) `
        -Wait `
        -PassThru `
        -WindowStyle Hidden

    Assert-True -Condition ($process.ExitCode -eq 0) -Name "library_only_loads_without_running_main"
    Assert-True -Condition (-not (Test-Path -LiteralPath (Join-Path $resultRoot "armed.json"))) -Name "library_only_does_not_arm"
    Assert-True -Condition (-not (Test-Path -LiteralPath (Join-Path $resultRoot "result.json"))) -Name "library_only_does_not_write_result"
    Assert-True -Condition (-not (Test-Path -LiteralPath (Join-Path $resultRoot "go.signal"))) -Name "library_only_does_not_create_go_signal"
} finally {
    Remove-Item -LiteralPath $resultRoot -Recurse -Force -ErrorAction SilentlyContinue
}

. $RunnerPath -LibraryOnly
$desktopAncestor = [pscustomobject]@{
    pid = 101
    parentPid = 1
    name = "ChatGPT.exe"
    executablePath = "C:\Program Files\WindowsApps\OpenAI.Codex_fixture\app\ChatGPT.exe"
}
$standaloneAncestor = [pscustomobject]@{
    pid = 102
    parentPid = 1
    name = "powershell.exe"
    executablePath = "C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"
}
Assert-True -Condition (Test-DesktopDependentAncestor -Chain @($desktopAncestor)) -Name "desktop_managed_runner_fails_lifecycle_gate"
Assert-True -Condition (-not (Test-DesktopDependentAncestor -Chain @($standaloneAncestor))) -Name "standalone_powershell_passes_lifecycle_gate"

$readinessCommand = Get-Command Invoke-A3ReadinessSequence -ErrorAction SilentlyContinue
Assert-True -Condition ($null -ne $readinessCommand) -Name "readiness_sequence_is_available_for_focused_tests"
if ($null -eq $readinessCommand) {
    Write-Host "RESULT passed=$script:Passed failed=$script:Failed"
    exit 1
}

$script:RpcCalls = [System.Collections.Generic.List[string]]::new()
$authFailureRpc = {
    param($Id, $Method, $Params)
    $script:RpcCalls.Add($Method)
    return [pscustomobject]@{
        error = [pscustomobject]@{ message = "invalid token" }
        result = $null
    }
}
$authFailureMessage = $null
try {
    Invoke-A3ReadinessSequence `
        -Rpc $authFailureRpc `
        -Token "fixture-token" `
        -ExpectedRemoteHostIdentity "014383f2-41f8-4b13-b9d7-30c511e47cec" `
        -WorkspaceId "phase-3-5-1d-workspace" | Out-Null
} catch {
    $authFailureMessage = $_.Exception.Message
}
Assert-Equal -Actual $authFailureMessage -Expected "daemon authentication failed" -Name "auth_failure_fails_closed"
Assert-Equal -Actual $script:RpcCalls.Count -Expected 1 -Name "auth_failure_stops_before_daemon_info"
Assert-Equal -Actual $script:RpcCalls[0] -Expected "auth" -Name "auth_failure_only_calls_auth"

function New-ReadyResponses {
    return @{
        auth = [pscustomobject]@{ error = $null; result = [pscustomobject]@{ ok = $true } }
        daemon_info = [pscustomobject]@{
            error = $null
            result = [pscustomobject]@{
                name = "codex-monitor-daemon"
                mode = "tcp"
                protocolVersion = 1
                remoteHostIdentity = "014383f2-41f8-4b13-b9d7-30c511e47cec"
            }
        }
        list_workspaces = [pscustomobject]@{
            error = $null
            result = @(
                [pscustomobject]@{
                    id = "phase-3-5-1d-workspace"
                    connected = $true
                }
            )
        }
    }
}

function Invoke-ReadinessFixture {
    param([hashtable]$Responses)

    $script:RpcCalls = [System.Collections.Generic.List[string]]::new()
    $script:FixtureResponses = $Responses
    $fixtureRpc = {
        param($Id, $Method, $Params)
        $script:RpcCalls.Add($Method)
        return $script:FixtureResponses[$Method]
    }
    return Invoke-A3ReadinessSequence `
        -Rpc $fixtureRpc `
        -Token "fixture-token" `
        -ExpectedRemoteHostIdentity "014383f2-41f8-4b13-b9d7-30c511e47cec" `
        -WorkspaceId "phase-3-5-1d-workspace"
}

$identityResponses = New-ReadyResponses
$identityResponses.daemon_info.result.remoteHostIdentity = "0ae35772-5057-459e-b2b2-f259e492303f"
$identityFailure = $null
try { Invoke-ReadinessFixture -Responses $identityResponses | Out-Null } catch { $identityFailure = $_.Exception.Message }
Assert-Equal -Actual $identityFailure -Expected "remote host identity mismatch" -Name "identity_mismatch_fails_closed"
Assert-Equal -Actual ($script:RpcCalls -join ",") -Expected "auth,daemon_info" -Name "identity_mismatch_stops_before_workspace_read"

$workspaceResponses = New-ReadyResponses
$workspaceResponses.list_workspaces.result[0].connected = $false
$workspaceFailure = $null
try { Invoke-ReadinessFixture -Responses $workspaceResponses | Out-Null } catch { $workspaceFailure = $_.Exception.Message }
Assert-Equal -Actual $workspaceFailure -Expected "acceptance WorkspaceSession is not connected" -Name "workspace_disconnected_fails_closed"
Assert-Equal -Actual ($script:RpcCalls -join ",") -Expected "auth,daemon_info,list_workspaces" -Name "workspace_disconnected_never_calls_connect_workspace"

$serviceResponses = New-ReadyResponses
$serviceResponses.daemon_info.result.name = "wrong-service"
$serviceFailure = $null
try { Invoke-ReadinessFixture -Responses $serviceResponses | Out-Null } catch { $serviceFailure = $_.Exception.Message }
Assert-Equal -Actual $serviceFailure -Expected "remote daemon identity response has an unexpected service name" -Name "wrong_service_fails_closed"

$modeResponses = New-ReadyResponses
$modeResponses.daemon_info.result.mode = "wrong-mode"
$modeFailure = $null
try { Invoke-ReadinessFixture -Responses $modeResponses | Out-Null } catch { $modeFailure = $_.Exception.Message }
Assert-Equal -Actual $modeFailure -Expected "remote daemon identity response has an unexpected mode" -Name "wrong_mode_fails_closed"

$protocolResponses = New-ReadyResponses
$protocolResponses.daemon_info.result.protocolVersion = 2
$protocolFailure = $null
try { Invoke-ReadinessFixture -Responses $protocolResponses | Out-Null } catch { $protocolFailure = $_.Exception.Message }
Assert-Equal -Actual $protocolFailure -Expected "unsupported remote daemon protocol version: 2" -Name "wrong_protocol_fails_closed"

$ready = Invoke-ReadinessFixture -Responses (New-ReadyResponses)
Assert-True -Condition $ready.workspaceConnected -Name "ready_workspace_passes_authenticated_gate"
Assert-Equal -Actual $ready.observedRemoteHostIdentity -Expected "014383f2-41f8-4b13-b9d7-30c511e47cec" -Name "ready_gate_returns_exact_host_identity"
Assert-Equal -Actual ($script:RpcCalls -join ",") -Expected "auth,daemon_info,list_workspaces" -Name "readiness_gate_calls_only_read_methods"

$armCommand = Get-Command New-A3ArmState -ErrorAction SilentlyContinue
$controlWaitCommand = Get-Command Wait-A3ControlSignal -ErrorAction SilentlyContinue
$desktopWaitStateCommand = Get-Command New-A3DesktopWaitState -ErrorAction SilentlyContinue
Assert-True -Condition ($null -ne $armCommand) -Name "arm_state_builder_is_available"
Assert-True -Condition ($null -ne $controlWaitCommand) -Name "control_signal_waiter_is_available"
Assert-True -Condition ($null -ne $desktopWaitStateCommand) -Name "desktop_wait_state_builder_is_available"
Assert-True -Condition (Test-Path -LiteralPath $SignalHelperPath -PathType Leaf) -Name "one_shot_go_signal_helper_exists"
Assert-True -Condition (Test-Path -LiteralPath $CancelHelperPath -PathType Leaf) -Name "one_shot_cancel_signal_helper_exists"
if ($null -eq $armCommand -or $null -eq $controlWaitCommand -or $null -eq $desktopWaitStateCommand -or
    -not (Test-Path -LiteralPath $SignalHelperPath -PathType Leaf) -or
    -not (Test-Path -LiteralPath $CancelHelperPath -PathType Leaf)) {
    Write-Host "RESULT passed=$script:Passed failed=$script:Failed"
    exit 1
}

$signalRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("phase-3-5-2a-a3-signal-{0}" -f [guid]::NewGuid())
try {
    New-Item -ItemType Directory -Path $signalRoot | Out-Null
    $armedPath = Join-Path $signalRoot "armed.json"
    $goPath = Join-Path $signalRoot "go.signal"
    $cancelPath = Join-Path $signalRoot "cancel.signal"
    $armedAt = [DateTimeOffset]::Parse("2026-09-13T01:02:03Z")
    $armNonce = "fixture-arm-nonce"
    $runnerSha = (Get-FileHash -LiteralPath $RunnerPath -Algorithm SHA256).Hash
    $arm = New-A3ArmState `
        -RunnerPid $PID `
        -SupervisorPid 0 `
        -RunnerStartedAt $armedAt `
        -ArmedAt $armedAt `
        -RunnerPath $RunnerPath `
        -RunnerSha256 $runnerSha `
        -ArmNonce $armNonce `
        -GoSignalPath $goPath `
        -CancelSignalPath $cancelPath `
        -Readiness $ready `
        -ThreadId "01a08c05-7880-75a2-976c-2a5895b58723" `
        -WorkspaceId "phase-3-5-1d-workspace" `
        -TargetId "phase-3-5-1d-target" `
        -Endpoint "127.0.0.1:54060" `
        -AncestorChain @() `
        -DesktopProcesses @()
    Assert-Equal -Actual $arm.state -Expected "ARMED_WAITING_FOR_GO" -Name "ready_state_arms_waiting_for_go"
    Assert-Equal -Actual $arm.goWaitMode -Expected "EXPLICIT_SIGNAL_ONLY" -Name "armed_wait_uses_explicit_signal_only"
    Assert-True -Condition (-not ($arm.PSObject.Properties.Name -contains "desktopExitDeadline")) -Name "armed_without_go_has_no_desktop_deadline"
    Assert-Equal -Actual $arm.workspaceConnected -Expected $true -Name "armed_evidence_records_connected_workspace"

    $armJson = $arm | ConvertTo-Json -Depth 20
    [System.IO.File]::WriteAllText($armedPath, $armJson, [System.Text.UTF8Encoding]::new($false))
    Assert-True -Condition ($armJson -notmatch '"token') -Name "armed_evidence_contains_no_token_field"
    $powershellPath = (Get-Process -Id $PID).Path
    $signalProcess = Start-Process `
        -FilePath $powershellPath `
        -ArgumentList @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $SignalHelperPath, "-ResultDirectory", $signalRoot) `
        -Wait `
        -PassThru `
        -WindowStyle Hidden
    Assert-Equal -Actual $signalProcess.ExitCode -Expected 0 -Name "go_helper_creates_one_shot_signal"
    Assert-True -Condition (Test-Path -LiteralPath $goPath -PathType Leaf) -Name "go_signal_is_host_visible_file"
    $goJson = [System.IO.File]::ReadAllText($goPath)
    Assert-True -Condition ($goJson -notmatch '"token') -Name "go_signal_contains_no_token_field"
    $secondSignalProcess = Start-Process `
        -FilePath $powershellPath `
        -ArgumentList @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $SignalHelperPath, "-ResultDirectory", $signalRoot) `
        -Wait `
        -PassThru `
        -WindowStyle Hidden
    Assert-True -Condition ($secondSignalProcess.ExitCode -ne 0) -Name "go_helper_rejects_second_signal"
    Assert-Equal -Actual ([System.IO.File]::ReadAllText($goPath)) -Expected $goJson -Name "rejected_second_signal_does_not_overwrite_first"

    $go = Wait-A3ControlSignal `
        -GoPath $goPath `
        -CancelPath $cancelPath `
        -ExpectedRunnerPid $PID `
        -ExpectedArmNonce $armNonce `
        -ExpectedRunnerSha256 $runnerSha `
        -PollMilliseconds 10
    Assert-Equal -Actual $go.kind -Expected "GO" -Name "valid_go_signal_is_accepted"

    Remove-Item -LiteralPath $goPath -Force
    Assert-True -Condition (-not ($arm.PSObject.Properties.Name -contains "goSignalTimeoutSeconds")) -Name "armed_without_go_has_no_hidden_timeout"

    $goReceivedAt = [DateTimeOffset]::Parse("2026-09-13T01:05:00Z")
    $waiting = New-A3DesktopWaitState -ArmedState $arm -GoReceivedAt $goReceivedAt -TimeoutSeconds 60
    Assert-Equal -Actual $waiting.state -Expected "WAITING_FOR_DESKTOP_EXIT" -Name "go_advances_to_waiting_for_desktop_exit"
    Assert-Equal `
        -Actual $waiting.desktopExitDeadline `
        -Expected $goReceivedAt.AddSeconds(60).ToString("o") `
        -Name "desktop_exit_deadline_starts_at_go"
} finally {
    Remove-Item -LiteralPath $signalRoot -Recurse -Force -ErrorAction SilentlyContinue
}

$dummyDesktop = [pscustomobject]@{
    pid = 424242
    parentPid = 1
    name = "ChatGPT.exe"
    executablePath = "C:\Program Files\WindowsApps\OpenAI.Codex_fixture\app\ChatGPT.exe"
}
$desktopStillOpen = Wait-CodexDesktopExit `
    -InitialProcesses @($dummyDesktop) `
    -TimeoutSeconds 0.05 `
    -StabilityMilliseconds 10 `
    -PollMilliseconds 5 `
    -DesktopTreeProvider { @($dummyDesktop) } `
    -ProcessInventoryProvider { @($dummyDesktop) }
Assert-True -Condition (-not $desktopStillOpen.exited) -Name "desktop_still_open_does_not_advance"

$desktopReleased = Wait-CodexDesktopExit `
    -InitialProcesses @($dummyDesktop) `
    -TimeoutSeconds 0.2 `
    -StabilityMilliseconds 10 `
    -PollMilliseconds 5 `
    -DesktopTreeProvider { @() } `
    -ProcessInventoryProvider { @() }
Assert-True -Condition $desktopReleased.exited -Name "desktop_exit_requires_stable_absence"

$finalSequenceCommand = Get-Command Invoke-A3FinalRpcSequence -ErrorAction SilentlyContinue
Assert-True -Condition ($null -ne $finalSequenceCommand) -Name "final_rpc_sequence_is_available"
if ($null -eq $finalSequenceCommand) {
    Write-Host "RESULT passed=$script:Passed failed=$script:Failed"
    exit 1
}

$finalDisconnectedResponses = New-ReadyResponses
$finalDisconnectedResponses.list_workspaces.result[0].connected = $false
$script:FixtureResponses = $finalDisconnectedResponses
$script:RpcCalls = [System.Collections.Generic.List[string]]::new()
$script:ResumeParams = $null
$finalDisconnectedRpc = {
    param($Id, $Method, $Params)
    $script:RpcCalls.Add($Method)
    if ($Method -eq "resume_thread") {
        $script:ResumeParams = $Params
    }
    return $script:FixtureResponses[$Method]
}
$finalDisconnectedError = $null
try {
    Invoke-A3FinalRpcSequence `
        -Rpc $finalDisconnectedRpc `
        -Token "fixture-token" `
        -ExpectedRemoteHostIdentity "014383f2-41f8-4b13-b9d7-30c511e47cec" `
        -WorkspaceId "phase-3-5-1d-workspace" `
        -ThreadId "01a08c05-7880-75a2-976c-2a5895b58723" | Out-Null
} catch {
    $finalDisconnectedError = $_.Exception.Message
}
Assert-Equal -Actual $finalDisconnectedError -Expected "acceptance WorkspaceSession is not connected" -Name "final_workspace_disconnected_fails_closed"
Assert-Equal -Actual ($script:RpcCalls -join ",") -Expected "auth,daemon_info,list_workspaces" -Name "final_workspace_disconnected_sends_no_resume"

$finalReadyResponses = New-ReadyResponses
$finalReadyResponses.resume_thread = [pscustomobject]@{
    error = $null
    result = [pscustomobject]@{
        thread = [pscustomobject]@{ id = "01a08c05-7880-75a2-976c-2a5895b58723" }
    }
}
$script:FixtureResponses = $finalReadyResponses
$script:RpcCalls = [System.Collections.Generic.List[string]]::new()
$script:ResumeParams = $null
$finalReadyRpc = {
    param($Id, $Method, $Params)
    $script:RpcCalls.Add($Method)
    if ($Method -eq "resume_thread") {
        $script:ResumeParams = $Params
    }
    return $script:FixtureResponses[$Method]
}
$finalReady = Invoke-A3FinalRpcSequence `
    -Rpc $finalReadyRpc `
    -Token "fixture-token" `
    -ExpectedRemoteHostIdentity "014383f2-41f8-4b13-b9d7-30c511e47cec" `
    -WorkspaceId "phase-3-5-1d-workspace" `
    -ThreadId "01a08c05-7880-75a2-976c-2a5895b58723"
Assert-Equal -Actual ($script:RpcCalls -join ",") -Expected "auth,daemon_info,list_workspaces,resume_thread" -Name "final_ready_sends_exactly_one_resume_after_readiness"
Assert-Equal -Actual $script:ResumeParams.workspaceId -Expected "phase-3-5-1d-workspace" -Name "resume_uses_exact_workspace_id"
Assert-Equal -Actual $script:ResumeParams.threadId -Expected "01a08c05-7880-75a2-976c-2a5895b58723" -Name "resume_uses_exact_full_thread_id"
Assert-Equal -Actual $script:ResumeParams.Keys.Count -Expected 2 -Name "resume_parameters_contain_only_workspace_and_thread_ids"
Assert-True -Condition (-not ($script:ResumeParams.Keys -match 'force|takeover')) -Name "resume_parameters_have_no_force_takeover"
Assert-Equal -Actual $finalReady.resumeResponse.result.thread.id -Expected "01a08c05-7880-75a2-976c-2a5895b58723" -Name "final_sequence_preserves_resume_response"
Assert-Equal -Actual @($script:RpcCalls | Where-Object { $_ -in @("thread/start", "start_thread") }).Count -Expected 0 -Name "final_sequence_has_no_thread_start"
Assert-Equal -Actual @($script:RpcCalls | Where-Object { $_ -in @("turn/start", "start_turn") }).Count -Expected 0 -Name "final_sequence_has_no_turn_start"

$script:BeforeResumeCalled = $false
$script:ResumeObservedAfterCallback = $false
$script:FixtureResponses = $finalReadyResponses
$orderedRpc = {
    param($Id, $Method, $Params)
    if ($Method -eq "resume_thread") {
        $script:ResumeObservedAfterCallback = $script:BeforeResumeCalled
    }
    return $script:FixtureResponses[$Method]
}
$orderedState = [ordered]@{
    observedRemoteHostIdentity = $null
    workspaceConnectedBeforeResume = $null
    finalReadinessPassed = $false
    canonicalBefore = $null
    canonicalBeforeAssertions = $null
    noTargetMutationBeforeResume = $null
    turnCountBefore = $null
    resumeAttemptCount = 0
}
$orderedStage = [pscustomobject]@{ value = "FINAL_READINESS" }
$canonicalSnapshotProvider = {
    param([string]$ExactThreadId)
    $script:BeforeResumeCalled = $true
    return [pscustomobject]@{
        total = 1
        unique = 1
        duplicate = 0
        exactRows = if ($ExactThreadId -eq "01a08c05-7880-75a2-976c-2a5895b58723") { 1 } else { 0 }
        codexHomeIdentity = "codex-home:fixture"
        fullThreadId = $ExactThreadId
        rolloutPath = "C:\fixture\target-rollout.jsonl"
        turnCount = 1
    }
}
$orderedFinal = Invoke-A3FinalRpcSequence `
    -Rpc $orderedRpc `
    -Token "fixture-token" `
    -ExpectedRemoteHostIdentity "014383f2-41f8-4b13-b9d7-30c511e47cec" `
    -WorkspaceId "phase-3-5-1d-workspace" `
    -ThreadId "01a08c05-7880-75a2-976c-2a5895b58723" `
    -State $orderedState `
    -Stage $orderedStage `
    -CanonicalSnapshotProvider $canonicalSnapshotProvider
Assert-True -Condition $script:BeforeResumeCalled -Name "canonical_before_hook_runs_after_final_readiness"
Assert-True -Condition $script:ResumeObservedAfterCallback -Name "resume_runs_after_canonical_before_hook"
Assert-Equal -Actual $orderedFinal.canonicalBefore.turnCount -Expected 1 -Name "final_sequence_returns_canonical_before_snapshot"
Assert-True -Condition $orderedState.noTargetMutationBeforeResume -Name "canonical_before_confirms_no_target_mutation_before_resume"

$canonicalBeforeUnrelatedGrowth = [pscustomobject]@{
    total = 633
    unique = 633
    duplicate = 0
    exactRows = 1
    codexHomeIdentity = "codex-home:fixture"
    fullThreadId = "01a08c05-7880-75a2-976c-2a5895b58723"
    rolloutPath = "C:\fixture\target-rollout.jsonl"
    turnCount = 1
}
$canonicalAfterUnrelatedGrowth = [pscustomobject]@{
    total = 634
    unique = 634
    duplicate = 0
    exactRows = 1
    codexHomeIdentity = "codex-home:fixture"
    fullThreadId = "01a08c05-7880-75a2-976c-2a5895b58723"
    rolloutPath = "C:\fixture\target-rollout.jsonl"
    turnCount = 1
}
$unrelatedGrowthGate = Get-A3CanonicalGateEvaluation `
    -Before $canonicalBeforeUnrelatedGrowth `
    -After $canonicalAfterUnrelatedGrowth `
    -ExpectedThreadId "01a08c05-7880-75a2-976c-2a5895b58723"
Assert-True `
    -Condition (@($unrelatedGrowthGate.assertions.Values | Where-Object { -not $_ }).Count -eq 0) `
    -Name "canonical_gate_allows_unrelated_global_growth"
Assert-Equal -Actual $unrelatedGrowthGate.telemetry.beforeTotal -Expected 633 -Name "canonical_gate_records_before_total_as_telemetry"
Assert-Equal -Actual $unrelatedGrowthGate.telemetry.afterTotal -Expected 634 -Name "canonical_gate_records_after_total_as_telemetry"
Assert-Equal -Actual $unrelatedGrowthGate.telemetry.totalDelta -Expected 1 -Name "canonical_gate_records_unrelated_growth_delta"

$duplicateBefore = $canonicalBeforeUnrelatedGrowth.PSObject.Copy()
$duplicateBefore.duplicate = 1
$duplicateGate = Get-A3CanonicalGateEvaluation -Before $duplicateBefore -After $canonicalAfterUnrelatedGrowth -ExpectedThreadId "01a08c05-7880-75a2-976c-2a5895b58723"
Assert-True -Condition (-not $duplicateGate.assertions.canonicalDuplicateZeroBefore) -Name "canonical_gate_rejects_duplicate_before_resume"

$duplicateTarget = $canonicalBeforeUnrelatedGrowth.PSObject.Copy()
$duplicateTarget.exactRows = 2
$duplicateTargetGate = Get-A3CanonicalGateEvaluation -Before $duplicateTarget -After $canonicalAfterUnrelatedGrowth -ExpectedThreadId "01a08c05-7880-75a2-976c-2a5895b58723"
Assert-True -Condition (-not $duplicateTargetGate.assertions.exactTargetRowBefore) -Name "canonical_gate_rejects_two_target_rows"

$changedKeyAfter = $canonicalAfterUnrelatedGrowth.PSObject.Copy()
$changedKeyAfter.codexHomeIdentity = "codex-home:other"
$changedKeyGate = Get-A3CanonicalGateEvaluation -Before $canonicalBeforeUnrelatedGrowth -After $changedKeyAfter -ExpectedThreadId "01a08c05-7880-75a2-976c-2a5895b58723"
Assert-True -Condition (-not $changedKeyGate.assertions.targetCodexThreadKeyUnchanged) -Name "canonical_gate_rejects_target_key_change"

$changedRolloutAfter = $canonicalAfterUnrelatedGrowth.PSObject.Copy()
$changedRolloutAfter.rolloutPath = "C:\fixture\unexpected-rollout.jsonl"
$changedRolloutGate = Get-A3CanonicalGateEvaluation -Before $canonicalBeforeUnrelatedGrowth -After $changedRolloutAfter -ExpectedThreadId "01a08c05-7880-75a2-976c-2a5895b58723"
Assert-True -Condition (-not $changedRolloutGate.assertions.targetRolloutPathUnchanged) -Name "canonical_gate_rejects_target_rollout_change"

$changedTurnBefore = $canonicalBeforeUnrelatedGrowth.PSObject.Copy()
$changedTurnBefore.turnCount = 2
$changedTurnGate = Get-A3CanonicalGateEvaluation -Before $changedTurnBefore -After $canonicalAfterUnrelatedGrowth -ExpectedThreadId "01a08c05-7880-75a2-976c-2a5895b58723"
Assert-True -Condition (-not $changedTurnGate.assertions.targetTurnCountBeforeOne) -Name "canonical_gate_rejects_unexpected_turn_count_before_resume"

$script:RpcCalls = [System.Collections.Generic.List[string]]::new()
$invalidBeforeRpc = {
    param($Id, $Method, $Params)
    $script:RpcCalls.Add($Method)
    return $finalReadyResponses[$Method]
}
$invalidBeforeState = [ordered]@{
    observedRemoteHostIdentity = $null
    workspaceConnectedBeforeResume = $null
    finalReadinessPassed = $false
    canonicalBefore = $null
    canonicalBeforeAssertions = $null
    noTargetMutationBeforeResume = $null
    turnCountBefore = $null
    resumeAttemptCount = 0
}
$invalidBeforeStage = [pscustomobject]@{ value = "FINAL_READINESS" }
$invalidBeforeProvider = {
    param([string]$ExactThreadId)
    return [pscustomobject]@{
        total = 633
        unique = 633
        duplicate = 0
        exactRows = 1
        codexHomeIdentity = "codex-home:fixture"
        fullThreadId = $ExactThreadId
        rolloutPath = "C:\fixture\target-rollout.jsonl"
        turnCount = 2
    }
}
$invalidBeforeError = $null
try {
    Invoke-A3FinalRpcSequence `
        -Rpc $invalidBeforeRpc `
        -Token "fixture-token" `
        -ExpectedRemoteHostIdentity "014383f2-41f8-4b13-b9d7-30c511e47cec" `
        -WorkspaceId "phase-3-5-1d-workspace" `
        -ThreadId "01a08c05-7880-75a2-976c-2a5895b58723" `
        -State $invalidBeforeState `
        -Stage $invalidBeforeStage `
        -CanonicalSnapshotProvider $invalidBeforeProvider | Out-Null
} catch {
    $invalidBeforeError = $_.Exception.Message
}
Assert-True -Condition ($invalidBeforeError -match "canonical pre-resume invariant failed") -Name "canonical_before_failure_is_explicit"
Assert-Equal -Actual ($script:RpcCalls -join ",") -Expected "auth,daemon_info,list_workspaces" -Name "canonical_before_failure_sends_no_resume"
Assert-Equal -Actual $invalidBeforeState.resumeAttemptCount -Expected 0 -Name "canonical_before_failure_records_zero_resume_attempts"

function Invoke-RealFileScenario {
    param([Parameter(Mandatory = $true)][string]$Scenario)

    $scenarioRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("phase-3-5-2a-a3-file-scope-{0}" -f [guid]::NewGuid())
    $outputPath = Join-Path $scenarioRoot "result.json"
    try {
        New-Item -ItemType Directory -Path $scenarioRoot | Out-Null
        $powershellPath = (Get-Process -Id $PID).Path
        $process = Start-Process `
            -FilePath $powershellPath `
            -ArgumentList @(
                "-NoProfile",
                "-ExecutionPolicy", "Bypass",
                "-File", $RunnerPath,
                "-FileScopeRegressionScenario", $Scenario,
                "-FileScopeRegressionOutputPath", $outputPath
            ) `
            -Wait `
            -PassThru `
            -WindowStyle Hidden
        Assert-Equal -Actual $process.ExitCode -Expected 0 -Name "real_file_${Scenario}_exits_successfully"
        Assert-True -Condition (Test-Path -LiteralPath $outputPath -PathType Leaf) -Name "real_file_${Scenario}_writes_result"
        if (-not (Test-Path -LiteralPath $outputPath -PathType Leaf)) {
            return $null
        }
        return [System.IO.File]::ReadAllText($outputPath) | ConvertFrom-Json
    } finally {
        Remove-Item -LiteralPath $scenarioRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

$realAuthFailure = Invoke-RealFileScenario -Scenario "AUTH_FAILURE"
if ($null -ne $realAuthFailure) {
    Assert-Equal -Actual $realAuthFailure.status -Expected "AUTH_FAILURE" -Name "real_file_auth_failure_fails_closed"
    Assert-Equal -Actual ($realAuthFailure.invokedMethods -join ",") -Expected "auth" -Name "real_file_auth_dispatch_reaches_actual_rpc_adapter"
    Assert-True -Condition (-not $realAuthFailure.armed) -Name "real_file_auth_failure_never_arms"
}

$realCanonicalGrowth = Invoke-RealFileScenario -Scenario "CANONICAL_GATE_UNRELATED_GROWTH"
if ($null -ne $realCanonicalGrowth) {
    Assert-Equal -Actual $realCanonicalGrowth.status -Expected "PASS" -Name "real_file_canonical_gate_allows_unrelated_growth"
    Assert-Equal -Actual $realCanonicalGrowth.canonicalTelemetry.beforeTotal -Expected 633 -Name "real_file_canonical_gate_records_before_total"
    Assert-Equal -Actual $realCanonicalGrowth.canonicalTelemetry.afterTotal -Expected 634 -Name "real_file_canonical_gate_records_after_total"
    Assert-Equal -Actual $realCanonicalGrowth.resumeRpcCount -Expected 0 -Name "real_file_canonical_gate_regression_sends_no_resume"
}

$realUtf8NoBomCanonical = Invoke-RealFileScenario -Scenario "UTF8_NO_BOM_CANONICAL_SNAPSHOT"
if ($null -ne $realUtf8NoBomCanonical) {
    Assert-Equal -Actual $realUtf8NoBomCanonical.status -Expected "PASS" -Name "real_file_utf8_no_bom_canonical_snapshot_passes"
    Assert-Equal -Actual $realUtf8NoBomCanonical.canonicalDuplicate -Expected 0 -Name "real_file_utf8_no_bom_snapshot_has_no_duplicate"
    Assert-Equal -Actual $realUtf8NoBomCanonical.exactRows -Expected 1 -Name "real_file_utf8_no_bom_snapshot_preserves_exact_target"
    Assert-Equal -Actual $realUtf8NoBomCanonical.turnCount -Expected 1 -Name "real_file_utf8_no_bom_snapshot_preserves_turn_count"
    Assert-Equal -Actual $realUtf8NoBomCanonical.rolloutLines -Expected 2 -Name "real_file_utf8_no_bom_snapshot_reads_all_lines"
    Assert-Equal -Actual $realUtf8NoBomCanonical.resumeRpcCount -Expected 0 -Name "real_file_utf8_no_bom_snapshot_sends_no_resume"
}

$realSharedWriterCanonical = Invoke-RealFileScenario -Scenario "SHARED_WRITER_CANONICAL_SNAPSHOT"
if ($null -ne $realSharedWriterCanonical) {
    Assert-Equal -Actual $realSharedWriterCanonical.status -Expected "PASS" -Name "real_file_canonical_snapshot_reads_while_shared_writer_is_open"
    Assert-Equal -Actual $realSharedWriterCanonical.canonicalDuplicate -Expected 0 -Name "real_file_shared_writer_snapshot_has_no_duplicate"
    Assert-Equal -Actual $realSharedWriterCanonical.exactRows -Expected 1 -Name "real_file_shared_writer_snapshot_preserves_exact_target"
    Assert-Equal -Actual $realSharedWriterCanonical.turnCount -Expected 1 -Name "real_file_shared_writer_snapshot_preserves_turn_count"
    Assert-Equal -Actual $realSharedWriterCanonical.rolloutLines -Expected 2 -Name "real_file_shared_writer_snapshot_reads_all_lines"
    Assert-Equal -Actual $realSharedWriterCanonical.resumeRpcCount -Expected 0 -Name "real_file_shared_writer_snapshot_sends_no_resume"
}

$realIdentityMismatch = Invoke-RealFileScenario -Scenario "IDENTITY_MISMATCH"
if ($null -ne $realIdentityMismatch) {
    Assert-Equal -Actual $realIdentityMismatch.status -Expected "IDENTITY_MISMATCH" -Name "real_file_identity_mismatch_never_arms"
    Assert-Equal -Actual ($realIdentityMismatch.invokedMethods -join ",") -Expected "auth,daemon_info" -Name "real_file_identity_mismatch_stops_before_workspace"
}

$realWorkspaceDisconnected = Invoke-RealFileScenario -Scenario "WORKSPACE_DISCONNECTED"
if ($null -ne $realWorkspaceDisconnected) {
    Assert-Equal -Actual $realWorkspaceDisconnected.status -Expected "WORKSPACE_DISCONNECTED" -Name "real_file_workspace_disconnected_never_arms"
    Assert-Equal -Actual ($realWorkspaceDisconnected.invokedMethods -join ",") -Expected "auth,daemon_info,list_workspaces" -Name "real_file_workspace_disconnected_never_connects_workspace"
}

$realReadyNoGo = Invoke-RealFileScenario -Scenario "READY_NO_GO"
if ($null -ne $realReadyNoGo) {
    Assert-Equal -Actual $realReadyNoGo.status -Expected "ARMED_WAITING_FOR_GO" -Name "real_file_ready_enters_armed_waiting_for_go"
    Assert-True -Condition (-not $realReadyNoGo.goReceived) -Name "real_file_armed_without_go_does_not_advance"
    Assert-Equal -Actual $realReadyNoGo.resumeRpcCount -Expected 0 -Name "real_file_armed_without_go_sends_no_resume"
}

$realGoTransition = Invoke-RealFileScenario -Scenario "GO_TRANSITION"
if ($null -ne $realGoTransition) {
    Assert-Equal -Actual $realGoTransition.status -Expected "WAITING_FOR_DESKTOP_EXIT" -Name "real_file_go_starts_desktop_exit_state"
    Assert-True -Condition $realGoTransition.goReceived -Name "real_file_go_is_received_before_desktop_exit_state"
    Assert-Equal -Actual $realGoTransition.resumeRpcCount -Expected 0 -Name "real_file_go_transition_sends_no_resume"
}

$realFinalDisconnected = Invoke-RealFileScenario -Scenario "FINAL_WORKSPACE_DISCONNECTED"
if ($null -ne $realFinalDisconnected) {
    Assert-Equal -Actual $realFinalDisconnected.status -Expected "FINAL_WORKSPACE_DISCONNECTED" -Name "real_file_final_readiness_failure_fails_closed"
    Assert-Equal -Actual $realFinalDisconnected.resumeRpcCount -Expected 0 -Name "real_file_final_readiness_failure_sends_no_resume"
}

$realFinalReady = Invoke-RealFileScenario -Scenario "FINAL_READY"
if ($null -ne $realFinalReady) {
    Assert-Equal -Actual $realFinalReady.status -Expected "PASS" -Name "real_file_final_ready_passes"
    Assert-Equal -Actual ($realFinalReady.invokedMethods -join ",") -Expected "auth,daemon_info,list_workspaces,resume_thread" -Name "real_file_final_ready_sends_exactly_one_resume"
    Assert-Equal -Actual $realFinalReady.returnedFullThreadId -Expected "01a08c05-7880-75a2-976c-2a5895b58723" -Name "real_file_final_ready_preserves_exact_thread_id"
    Assert-Equal -Actual $realFinalReady.threadStartCount -Expected 0 -Name "real_file_final_ready_has_no_thread_start"
    Assert-Equal -Actual $realFinalReady.turnStartCount -Expected 0 -Name "real_file_final_ready_has_no_turn_start"
    Assert-Equal -Actual $realFinalReady.connectWorkspaceCount -Expected 0 -Name "real_file_final_ready_has_no_connect_workspace"
    Assert-Equal -Actual $realFinalReady.retryCount -Expected 0 -Name "real_file_final_ready_has_no_retry"
    Assert-True -Condition (-not $realFinalReady.forceTakeover) -Name "real_file_final_ready_has_no_force_takeover"
}

$fullLifecycleAuthFailure = Invoke-RealFileScenario -Scenario "FULL_LIFECYCLE_AUTH_FAILURE"
if ($null -ne $fullLifecycleAuthFailure) {
    Assert-Equal -Actual $fullLifecycleAuthFailure.status -Expected "PREARM_AUTH_FAILURE" -Name "real_file_full_lifecycle_auth_failure_fails_before_arm"
    Assert-True -Condition (-not $fullLifecycleAuthFailure.armed) -Name "real_file_full_lifecycle_auth_failure_never_arms"
    Assert-Equal -Actual $fullLifecycleAuthFailure.resumeRpcCount -Expected 0 -Name "real_file_full_lifecycle_auth_failure_sends_no_resume"
}

$fullLifecycleFinalIdentityMismatch = Invoke-RealFileScenario -Scenario "FULL_LIFECYCLE_FINAL_IDENTITY_MISMATCH"
if ($null -ne $fullLifecycleFinalIdentityMismatch) {
    Assert-Equal -Actual $fullLifecycleFinalIdentityMismatch.status -Expected "FINAL_IDENTITY_MISMATCH" -Name "real_file_full_lifecycle_final_identity_mismatch_fails_closed"
    Assert-True -Condition $fullLifecycleFinalIdentityMismatch.prearmPassed -Name "real_file_full_lifecycle_identity_mismatch_passes_prearm"
    Assert-True -Condition $fullLifecycleFinalIdentityMismatch.desktopStable -Name "real_file_full_lifecycle_identity_mismatch_reaches_stable_desktop_exit"
    Assert-Equal -Actual $fullLifecycleFinalIdentityMismatch.resumeRpcCount -Expected 0 -Name "real_file_full_lifecycle_identity_mismatch_sends_no_resume"
}

$fullLifecycleFinalWorkspaceDisconnected = Invoke-RealFileScenario -Scenario "FULL_LIFECYCLE_FINAL_WORKSPACE_DISCONNECTED"
if ($null -ne $fullLifecycleFinalWorkspaceDisconnected) {
    Assert-Equal -Actual $fullLifecycleFinalWorkspaceDisconnected.status -Expected "FINAL_WORKSPACE_DISCONNECTED" -Name "real_file_full_lifecycle_final_workspace_disconnect_fails_closed"
    Assert-True -Condition $fullLifecycleFinalWorkspaceDisconnected.prearmPassed -Name "real_file_full_lifecycle_workspace_disconnect_passes_prearm"
    Assert-True -Condition $fullLifecycleFinalWorkspaceDisconnected.desktopStable -Name "real_file_full_lifecycle_workspace_disconnect_reaches_stable_desktop_exit"
    Assert-Equal -Actual $fullLifecycleFinalWorkspaceDisconnected.resumeRpcCount -Expected 0 -Name "real_file_full_lifecycle_workspace_disconnect_sends_no_resume"
}

$fullLifecycleFinalReady = Invoke-RealFileScenario -Scenario "FULL_LIFECYCLE_FINAL_READY"
if ($null -ne $fullLifecycleFinalReady) {
    Assert-Equal -Actual $fullLifecycleFinalReady.status -Expected "PASS" -Name "real_file_full_lifecycle_final_ready_passes"
    Assert-True -Condition $fullLifecycleFinalReady.prearmPassed -Name "real_file_full_lifecycle_final_ready_passes_prearm"
    Assert-True -Condition $fullLifecycleFinalReady.armed -Name "real_file_full_lifecycle_final_ready_arms"
    Assert-True -Condition $fullLifecycleFinalReady.goReceived -Name "real_file_full_lifecycle_final_ready_receives_go"
    Assert-True -Condition $fullLifecycleFinalReady.desktopStable -Name "real_file_full_lifecycle_final_ready_reaches_stable_desktop_exit"
    Assert-True -Condition $fullLifecycleFinalReady.finalReadinessPassed -Name "real_file_full_lifecycle_final_ready_passes_final_readiness"
    Assert-Equal -Actual ($fullLifecycleFinalReady.invokedMethods -join ",") -Expected "auth,daemon_info,list_workspaces,auth,daemon_info,list_workspaces,resume_thread" -Name "real_file_full_lifecycle_final_ready_dispatch_order"
    Assert-Equal -Actual $fullLifecycleFinalReady.resumeRpcCount -Expected 1 -Name "real_file_full_lifecycle_final_ready_resumes_once"
    Assert-Equal -Actual $fullLifecycleFinalReady.returnedFullThreadId -Expected "01a08c05-7880-75a2-976c-2a5895b58723" -Name "real_file_full_lifecycle_final_ready_preserves_exact_thread_id"
    Assert-Equal -Actual $fullLifecycleFinalReady.threadStartCount -Expected 0 -Name "real_file_full_lifecycle_final_ready_has_no_thread_start"
    Assert-Equal -Actual $fullLifecycleFinalReady.turnStartCount -Expected 0 -Name "real_file_full_lifecycle_final_ready_has_no_turn_start"
    Assert-Equal -Actual $fullLifecycleFinalReady.connectWorkspaceCount -Expected 0 -Name "real_file_full_lifecycle_final_ready_has_no_connect_workspace"
    Assert-Equal -Actual $fullLifecycleFinalReady.retryCount -Expected 0 -Name "real_file_full_lifecycle_final_ready_has_no_retry"
    Assert-True -Condition (-not $fullLifecycleFinalReady.forceTakeover) -Name "real_file_full_lifecycle_final_ready_has_no_force_takeover"
}

Write-Host "RESULT passed=$script:Passed failed=$script:Failed"
if ($script:Failed -ne 0) {
    exit 1
}
