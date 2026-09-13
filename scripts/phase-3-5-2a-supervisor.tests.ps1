param(
    [Parameter(Mandatory = $true)][string]$RunnerPath,
    [Parameter(Mandatory = $true)][string]$SupervisorPath,
    [Parameter(Mandatory = $true)][string]$GoHelperPath,
    [Parameter(Mandatory = $true)][string]$CancelHelperPath
)

$ErrorActionPreference = "Stop"
$script:Passed = 0
$script:Failed = 0

function Assert-True {
    param([bool]$Condition, [string]$Name)
    if ($Condition) {
        $script:Passed += 1
        Write-Host "PASS $Name"
    } else {
        $script:Failed += 1
        Write-Host "FAIL $Name"
    }
}

function Assert-Equal {
    param($Actual, $Expected, [string]$Name)
    Assert-True -Condition ($Actual -eq $Expected) -Name $Name
}

function Wait-ForFile {
    param([string]$Path, [int]$TimeoutMilliseconds = 5000)
    $deadline = [DateTimeOffset]::UtcNow.AddMilliseconds($TimeoutMilliseconds)
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        if ([System.IO.File]::Exists($Path)) { return $true }
        Start-Sleep -Milliseconds 25
    }
    return $false
}

function Read-JsonFile {
    param([string]$Path)
    return [System.IO.File]::ReadAllText($Path) | ConvertFrom-Json
}

function Wait-ForArmedRunner {
    param([string]$Path, [int]$RunnerPid, [int]$TimeoutMilliseconds = 5000)
    $deadline = [DateTimeOffset]::UtcNow.AddMilliseconds($TimeoutMilliseconds)
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        if ([System.IO.File]::Exists($Path)) {
            try {
                $armed = Read-JsonFile -Path $Path
                if ([int]$armed.runnerPid -eq $RunnerPid) { return $true }
            } catch {
            }
        }
        Start-Sleep -Milliseconds 25
    }
    return $false
}

function Start-SupervisedFixture {
    param([string]$Scenario, [switch]$SeedStaleEvidence)
    $root = Join-Path ([System.IO.Path]::GetTempPath()) ("phase-3-5-2a-a3-lifetime-{0}" -f [guid]::NewGuid())
    New-Item -ItemType Directory -Path $root | Out-Null
    if ($SeedStaleEvidence) {
        [System.IO.File]::WriteAllText(
            (Join-Path $root "armed.json"),
            '{"state":"ARMED_WAITING_FOR_GO","runnerPid":2147480000,"evidence":"stale-armed"}',
            [System.Text.UTF8Encoding]::new($false)
        )
        [System.IO.File]::WriteAllText(
            (Join-Path $root "result.json"),
            '{"runnerPid":2147480000,"evidence":"stale-result"}',
            [System.Text.UTF8Encoding]::new($false)
        )
    }
    $powershellPath = (Get-Process -Id $PID).Path
    $process = Start-Process `
        -FilePath $powershellPath `
        -ArgumentList @(
            "-NoProfile",
            "-ExecutionPolicy", "Bypass",
            "-File", $SupervisorPath,
            "-RunnerPath", $RunnerPath,
            "-ResultDirectory", $root,
            "-RunnerLifecycleRegressionScenario", $Scenario
        ) `
        -PassThru `
        -WindowStyle Hidden
    return [pscustomobject]@{
        root = $root
        supervisorProcess = $process
        armedPath = Join-Path $root "armed.json"
        goPath = Join-Path $root "go.signal"
        cancelPath = Join-Path $root "cancel.signal"
        resultPath = Join-Path $root "result.json"
        processPath = Join-Path $root "runner-process.json"
        stdoutPath = Join-Path $root "runner.stdout.log"
        stderrPath = Join-Path $root "runner.stderr.log"
    }
}

foreach ($required in @($RunnerPath, $SupervisorPath, $GoHelperPath, $CancelHelperPath)) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "required lifecycle artifact is missing: $required"
    }
}

$cancelFixture = $null
try {
    $cancelFixture = Start-SupervisedFixture -Scenario "ARMED_CONTROL" -SeedStaleEvidence
    Assert-True -Condition (Wait-ForFile -Path $cancelFixture.processPath) -Name "supervisor_writes_running_process_evidence"
    $running = Read-JsonFile -Path $cancelFixture.processPath
    Assert-True -Condition (Wait-ForArmedRunner -Path $cancelFixture.armedPath -RunnerPid ([int]$running.runnerPid)) -Name "real_file_runner_arms_without_go"
    $armed = Read-JsonFile -Path $cancelFixture.armedPath
    Assert-Equal -Actual $armed.state -Expected "ARMED_WAITING_FOR_GO" -Name "armed_state_waits_for_explicit_control"
    Assert-Equal -Actual $armed.goWaitMode -Expected "EXPLICIT_SIGNAL_ONLY" -Name "armed_state_has_no_hidden_go_timeout"
    Assert-Equal -Actual $armed.runnerPid -Expected $running.runnerPid -Name "armed_pid_is_supervised_child_pid"
    Assert-Equal -Actual $armed.supervisorPid -Expected $running.supervisorPid -Name "armed_state_records_supervisor_pid"
    Assert-True -Condition (-not [string]::IsNullOrWhiteSpace([string]$running.staleEvidenceArchive)) -Name "supervisor_records_stale_evidence_archive"
    Assert-True -Condition (Test-Path -LiteralPath (Join-Path ([string]$running.staleEvidenceArchive) "armed.json") -PathType Leaf) -Name "stale_armed_evidence_is_preserved"
    Assert-True -Condition (Test-Path -LiteralPath (Join-Path ([string]$running.staleEvidenceArchive) "result.json") -PathType Leaf) -Name "stale_result_is_not_mixed_with_current_invocation"

    Start-Sleep -Milliseconds 750
    Assert-True -Condition ($null -ne (Get-Process -Id ([int]$armed.runnerPid) -ErrorAction SilentlyContinue)) -Name "armed_without_go_remains_alive"
    Assert-True -Condition (-not (Test-Path -LiteralPath $cancelFixture.goPath)) -Name "armed_wait_does_not_create_go"
    Assert-True -Condition (-not (Test-Path -LiteralPath $cancelFixture.resultPath)) -Name "armed_wait_does_not_create_terminal_result"

    $powershellPath = (Get-Process -Id $PID).Path
    $cancel = Start-Process `
        -FilePath $powershellPath `
        -ArgumentList @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $CancelHelperPath, "-ResultDirectory", $cancelFixture.root) `
        -Wait `
        -PassThru `
        -WindowStyle Hidden
    Assert-Equal -Actual $cancel.ExitCode -Expected 0 -Name "explicit_cancel_signal_is_accepted"
    $cancelFixture.supervisorProcess.WaitForExit(5000) | Out-Null
    Assert-True -Condition $cancelFixture.supervisorProcess.HasExited -Name "cancelled_runner_exits_cleanly"
    Assert-True -Condition (Wait-ForFile -Path $cancelFixture.resultPath) -Name "cancelled_runner_writes_result"
    $cancelResult = Read-JsonFile -Path $cancelFixture.resultPath
    $cancelProcess = Read-JsonFile -Path $cancelFixture.processPath
    Assert-Equal -Actual $cancelResult.status -Expected "CANCELLED_BEFORE_GO" -Name "cancel_result_is_explicit"
    Assert-Equal -Actual $cancelResult.resumeAttemptCount -Expected 0 -Name "cancel_sends_no_resume"
    Assert-Equal -Actual $cancelProcess.state -Expected "EXITED" -Name "supervisor_marks_clean_child_exited"
    Assert-True -Condition (-not [string]::IsNullOrWhiteSpace([string]$cancelProcess.exitedAt)) -Name "supervisor_records_clean_child_exited_at"
    Assert-Equal -Actual $cancelProcess.exitCode -Expected 0 -Name "supervisor_records_clean_cancel_exit_code"
    Assert-Equal -Actual $cancelProcess.terminationClassification -Expected "RUNNER_RESULT_OBSERVED" -Name "supervisor_distinguishes_result_backed_exit"
    Assert-True -Condition (Test-Path -LiteralPath $cancelFixture.stdoutPath -PathType Leaf) -Name "supervisor_persists_stdout"
    Assert-True -Condition (Test-Path -LiteralPath $cancelFixture.stderrPath -PathType Leaf) -Name "supervisor_persists_stderr"
} finally {
    if ($null -ne $cancelFixture) {
        if (-not $cancelFixture.supervisorProcess.HasExited) { Stop-Process -Id $cancelFixture.supervisorProcess.Id -Force -ErrorAction SilentlyContinue }
        Remove-Item -LiteralPath $cancelFixture.root -Recurse -Force -ErrorAction SilentlyContinue
    }
}

$failureFixture = $null
try {
    $failureFixture = Start-SupervisedFixture -Scenario "UNHANDLED_FAILURE"
    $failureFixture.supervisorProcess.WaitForExit(5000) | Out-Null
    Assert-True -Condition $failureFixture.supervisorProcess.HasExited -Name "unhandled_fixture_runner_terminates"
    Assert-True -Condition (Wait-ForFile -Path $failureFixture.processPath) -Name "supervisor_persists_unhandled_exit_evidence"
    $failureProcess = Read-JsonFile -Path $failureFixture.processPath
    Assert-Equal -Actual $failureProcess.state -Expected "EXITED" -Name "supervisor_marks_failed_child_exited"
    Assert-True -Condition (-not [string]::IsNullOrWhiteSpace([string]$failureProcess.exitedAt)) -Name "supervisor_records_failed_child_exited_at"
    Assert-Equal -Actual $failureProcess.exitCode -Expected 1 -Name "supervisor_records_nonzero_runner_exit"
    Assert-Equal -Actual $failureProcess.terminationClassification -Expected "UNATTRIBUTED_PROCESS_EXIT" -Name "missing_result_is_not_forged_as_a3_outcome"
    Assert-Equal -Actual $failureProcess.resultJsonObserved -Expected $false -Name "unhandled_exit_records_missing_result"
    Assert-True -Condition (Test-Path -LiteralPath $failureFixture.stdoutPath -PathType Leaf) -Name "unhandled_exit_preserves_stdout_file"
    Assert-True -Condition (Test-Path -LiteralPath $failureFixture.stderrPath -PathType Leaf) -Name "unhandled_exit_preserves_stderr_file"
    Assert-True -Condition ((Get-Item -LiteralPath $failureFixture.stderrPath).Length -gt 0) -Name "unhandled_exit_preserves_stderr_content"
} finally {
    if ($null -ne $failureFixture) {
        if (-not $failureFixture.supervisorProcess.HasExited) { Stop-Process -Id $failureFixture.supervisorProcess.Id -Force -ErrorAction SilentlyContinue }
        Remove-Item -LiteralPath $failureFixture.root -Recurse -Force -ErrorAction SilentlyContinue
    }
}

$goFixture = $null
try {
    $goFixture = Start-SupervisedFixture -Scenario "ARMED_CONTROL"
    Assert-True -Condition (Wait-ForFile -Path $goFixture.armedPath) -Name "go_fixture_runner_arms"
    $powershellPath = (Get-Process -Id $PID).Path
    $go = Start-Process `
        -FilePath $powershellPath `
        -ArgumentList @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $GoHelperPath, "-ResultDirectory", $goFixture.root) `
        -Wait `
        -PassThru `
        -WindowStyle Hidden
    Assert-Equal -Actual $go.ExitCode -Expected 0 -Name "explicit_go_signal_is_accepted"
    $goFixture.supervisorProcess.WaitForExit(5000) | Out-Null
    Assert-True -Condition $goFixture.supervisorProcess.HasExited -Name "go_transition_fixture_completes"
    $goResult = Read-JsonFile -Path $goFixture.resultPath
    Assert-Equal -Actual $goResult.status -Expected "WAITING_FOR_DESKTOP_EXIT" -Name "go_starts_desktop_exit_state"
    Assert-Equal -Actual $goResult.resumeAttemptCount -Expected 0 -Name "go_transition_alone_sends_no_resume"
} finally {
    if ($null -ne $goFixture) {
        if (-not $goFixture.supervisorProcess.HasExited) { Stop-Process -Id $goFixture.supervisorProcess.Id -Force -ErrorAction SilentlyContinue }
        Remove-Item -LiteralPath $goFixture.root -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Write-Host "RESULT passed=$script:Passed failed=$script:Failed"
if ($script:Failed -ne 0) { exit 1 }
