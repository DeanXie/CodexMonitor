param(
    [string]$ProcmonPath = "F:\AI\CodexMonitor\.tools\sysinternals\procmon\Procmon64.exe",
    [string]$EvidenceRoot = "F:\AI\CodexMonitor\.worktrees\phase-3-5-2a-remote-read-admission\src-tauri\target\phase-3-5-2a-a3-procmon",
    [int]$ShutdownTimeoutSeconds = 15,
    [int]$StableSamples = 3,
    [int]$StableSampleMilliseconds = 500
)

$ErrorActionPreference = "Stop"
$activeCapturePath = Join-Path $EvidenceRoot "active-capture.json"

function Test-IsElevated {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Write-JsonAtomically {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Value
    )

    $directory = [System.IO.Path]::GetDirectoryName($Path)
    [System.IO.Directory]::CreateDirectory($directory) | Out-Null
    $temporaryPath = Join-Path $directory (".{0}.{1}.{2}.tmp" -f [System.IO.Path]::GetFileName($Path), $PID, [Guid]::NewGuid().ToString("N"))
    $backupPath = "$Path.replace-backup-$PID-$([Guid]::NewGuid().ToString('N'))"
    try {
        [System.IO.File]::WriteAllText(
            $temporaryPath,
            ($Value | ConvertTo-Json -Depth 16),
            [System.Text.UTF8Encoding]::new($false)
        )
        if ([System.IO.File]::Exists($Path)) {
            [System.IO.File]::Replace($temporaryPath, $Path, $backupPath)
            [System.IO.File]::Delete($backupPath)
        } else {
            [System.IO.File]::Move($temporaryPath, $Path)
        }
    } finally {
        if ([System.IO.File]::Exists($temporaryPath)) {
            [System.IO.File]::Delete($temporaryPath)
        }
    }
}

function Assert-MicrosoftSignature {
    param([Parameter(Mandatory = $true)][string]$Path)

    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    if ($signature.Status -ne [System.Management.Automation.SignatureStatus]::Valid) {
        throw "ProcMon Authenticode status is not Valid: $($signature.Status)"
    }
    $subject = [string]$signature.SignerCertificate.Subject
    if ($subject -notmatch '(^|,\s*)O=Microsoft Corporation(,|$)') {
        throw "ProcMon signer is not Microsoft Corporation: $subject"
    }
    return $subject
}

function Wait-FileLengthStable {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][int]$RequiredSamples,
        [Parameter(Mandatory = $true)][int]$SampleMilliseconds,
        [Parameter(Mandatory = $true)][int]$TimeoutSeconds
    )

    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
    $lastLength = -1L
    $stableCount = 0
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        if (-not [System.IO.File]::Exists($Path)) {
            Start-Sleep -Milliseconds $SampleMilliseconds
            continue
        }
        $length = [System.IO.FileInfo]::new($Path).Length
        if ($length -eq $lastLength -and $length -gt 0) {
            $stableCount += 1
            if ($stableCount -ge $RequiredSamples) {
                return $length
            }
        } else {
            $stableCount = 0
            $lastLength = $length
        }
        Start-Sleep -Milliseconds $SampleMilliseconds
    }
    throw "PML size did not stabilize before timeout: $Path"
}

if (-not (Test-IsElevated)) {
    throw "ProcMon capture stop/export requires an elevated independent Windows PowerShell."
}

$resolvedProcmonPath = [System.IO.Path]::GetFullPath($ProcmonPath)
if (-not [System.IO.File]::Exists($resolvedProcmonPath)) {
    throw "ProcMon binary does not exist: $resolvedProcmonPath"
}
[void](Assert-MicrosoftSignature -Path $resolvedProcmonPath)

if (-not [System.IO.File]::Exists($activeCapturePath)) {
    throw "No active capture metadata exists: $activeCapturePath"
}
$capture = [System.IO.File]::ReadAllText($activeCapturePath) | ConvertFrom-Json
if ([string]$capture.status -ne "CAPTURING") {
    throw "active-capture.json is not in CAPTURING state."
}

$procmonSha256 = (Get-FileHash -LiteralPath $resolvedProcmonPath -Algorithm SHA256).Hash.ToUpperInvariant()
if ($procmonSha256 -ne ([string]$capture.procmonSha256).ToUpperInvariant()) {
    throw "ProcMon binary SHA256 changed after capture startup."
}
$pmcSha256 = (Get-FileHash -LiteralPath ([string]$capture.pmcPath) -Algorithm SHA256).Hash.ToUpperInvariant()
if ($pmcSha256 -ne ([string]$capture.pmcSha256).ToUpperInvariant()) {
    throw "ProcMon configuration SHA256 changed after capture startup."
}

$terminate = Start-Process -FilePath $resolvedProcmonPath -ArgumentList @("/AcceptEula", "/Terminate", "/Quiet") -PassThru -Wait
if ($terminate.ExitCode -ne 0) {
    throw "ProcMon /Terminate failed with exit code $($terminate.ExitCode)."
}

$deadline = [DateTimeOffset]::UtcNow.AddSeconds($ShutdownTimeoutSeconds)
do {
    $remaining = @(
        Get-CimInstance Win32_Process -ErrorAction Stop |
            Where-Object { $_.Name -in @("Procmon.exe", "Procmon64.exe", "Procmon64a.exe") }
    )
    if ($remaining.Count -eq 0) {
        break
    }
    Start-Sleep -Milliseconds 250
} while ([DateTimeOffset]::UtcNow -lt $deadline)

if ($remaining.Count -gt 0) {
    throw "ProcMon did not terminate before timeout."
}

$pmlPath = [System.IO.Path]::GetFullPath([string]$capture.pmlPath)
$pmlLength = Wait-FileLengthStable -Path $pmlPath -RequiredSamples $StableSamples -SampleMilliseconds $StableSampleMilliseconds -TimeoutSeconds $ShutdownTimeoutSeconds
$csvPath = [System.IO.Path]::ChangeExtension($pmlPath, ".filtered.csv")
if ([System.IO.File]::Exists($csvPath)) {
    throw "CSV evidence path already exists; refusing overwrite: $csvPath"
}

$exportArguments = @(
    "/AcceptEula"
    "/Quiet"
    "/OpenLog"
    ('"{0}"' -f $pmlPath)
    "/LoadConfig"
    ('"{0}"' -f ([string]$capture.pmcPath))
    "/SaveApplyFilter"
    "/SaveAs"
    ('"{0}"' -f $csvPath)
)
$export = Start-Process -FilePath $resolvedProcmonPath -ArgumentList $exportArguments -PassThru -Wait
if ($export.ExitCode -ne 0) {
    throw "ProcMon CSV export failed with exit code $($export.ExitCode)."
}
if (-not [System.IO.File]::Exists($csvPath) -or [System.IO.FileInfo]::new($csvPath).Length -le 0) {
    throw "ProcMon CSV export did not create non-empty evidence."
}

$stoppedAt = [DateTimeOffset]::UtcNow
$completed = [ordered]@{}
foreach ($property in $capture.PSObject.Properties) {
    $completed[$property.Name] = $property.Value
}
$completed.status = "COMPLETED"
$completed.stoppedAt = $stoppedAt.ToString("o")
$completed.pmlLength = $pmlLength
$completed.pmlSha256 = (Get-FileHash -LiteralPath $pmlPath -Algorithm SHA256).Hash.ToUpperInvariant()
$completed.csvPath = $csvPath
$completed.csvLength = [System.IO.FileInfo]::new($csvPath).Length
$completed.csvSha256 = (Get-FileHash -LiteralPath $csvPath -Algorithm SHA256).Hash.ToUpperInvariant()
$completed.procmonVersion = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($resolvedProcmonPath).FileVersion
$completed.pmcSha256 = $pmcSha256

$metadataPath = [string]$capture.metadataPath
Write-JsonAtomically -Path $metadataPath -Value ([pscustomobject]$completed)
Write-JsonAtomically -Path $activeCapturePath -Value ([pscustomobject]$completed)
([pscustomobject]$completed) | ConvertTo-Json -Depth 12
