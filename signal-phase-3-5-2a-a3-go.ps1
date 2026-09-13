param(
    [string]$ResultDirectory = "F:\AI\CodexMonitor\.worktrees\phase-3-5-2a-remote-read-admission\src-tauri\target\phase-3-5-2a-a3"
)

$ErrorActionPreference = "Stop"
$armedPath = Join-Path $ResultDirectory "armed.json"
$goSignalPath = Join-Path $ResultDirectory "go.signal"

if (-not (Test-Path -LiteralPath $armedPath -PathType Leaf)) {
    throw "A3 runner is not armed"
}
if (Test-Path -LiteralPath $goSignalPath) {
    throw "A3 GO signal already exists"
}

$armed = [System.IO.File]::ReadAllText($armedPath) | ConvertFrom-Json
if ([string]$armed.state -ne "ARMED_WAITING_FOR_GO") {
    throw "A3 runner is not waiting for GO"
}
$runner = Get-Process -Id ([int]$armed.runnerPid) -ErrorAction SilentlyContinue
if ($null -eq $runner) {
    throw "A3 runner process is not alive"
}
if (-not (Test-Path -LiteralPath ([string]$armed.runnerPath) -PathType Leaf)) {
    throw "A3 runner script is unavailable"
}
$currentRunnerSha256 = (Get-FileHash -LiteralPath ([string]$armed.runnerPath) -Algorithm SHA256).Hash
if ($currentRunnerSha256 -ne [string]$armed.runnerSha256) {
    throw "A3 runner script hash changed after arming"
}

$signal = [ordered]@{
    schemaVersion = 1
    state = "GO"
    createdAt = [DateTimeOffset]::UtcNow.ToString("o")
    runnerPid = [int]$armed.runnerPid
    runnerSha256 = [string]$armed.runnerSha256
    armNonce = [string]$armed.armNonce
}
$tempPath = "$goSignalPath.tmp-$PID-$([guid]::NewGuid().ToString('N'))"
try {
    $json = $signal | ConvertTo-Json -Depth 8
    [System.IO.File]::WriteAllText($tempPath, $json, [System.Text.UTF8Encoding]::new($false))
    [System.IO.File]::Move($tempPath, $goSignalPath)
} finally {
    if (Test-Path -LiteralPath $tempPath) {
        Remove-Item -LiteralPath $tempPath -Force
    }
}

Write-Host "A3 GO signal created for runner PID $($armed.runnerPid)."
