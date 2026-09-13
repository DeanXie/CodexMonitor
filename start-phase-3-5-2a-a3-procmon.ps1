param(
    [switch]$PreflightOnly,
    [string]$ProcmonPath = "F:\AI\CodexMonitor\.tools\sysinternals\procmon\Procmon64.exe",
    [string]$PmcPath = "F:\AI\CodexMonitor\.worktrees\phase-3-5-2a-remote-read-admission\src-tauri\target\phase-3-5-2a-a3-procmon\phase-3-5-2a-a3-path-filter.pmc",
    [string]$EvidenceRoot = "F:\AI\CodexMonitor\.worktrees\phase-3-5-2a-remote-read-admission\src-tauri\target\phase-3-5-2a-a3-procmon",
    [string]$ExpectedProcmonSha256 = "FC3AF5317C707E0555AD6E7590AD65CEB5C5085B053B41221944AC3CA3492D9C",
    [string]$ExpectedPmcSha256 = "5892372F0A7566E8CA84EB90F4735823CDF5E09F0C9C4AB4356C2966CE18AD2C",
    [int]$StartupTimeoutSeconds = 15
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

function Get-ExactProcmonProcesses {
    param([Parameter(Mandatory = $true)][string]$ExpectedPath)

    $normalizedExpectedPath = [System.IO.Path]::GetFullPath($ExpectedPath)
    return @(
        Get-CimInstance Win32_Process -ErrorAction Stop |
            Where-Object {
                $_.Name -in @("Procmon.exe", "Procmon64.exe", "Procmon64a.exe")
            } |
            ForEach-Object {
                $observedExecutablePath = [string]$_.ExecutablePath
                [pscustomobject]@{
                    processId = [int]$_.ProcessId
                    name = [string]$_.Name
                    executablePath = $observedExecutablePath
                    exactBinary = if ([string]::IsNullOrWhiteSpace($observedExecutablePath)) {
                        $false
                    } else {
                        [string]::Equals(
                            [System.IO.Path]::GetFullPath($observedExecutablePath),
                            $normalizedExpectedPath,
                            [System.StringComparison]::OrdinalIgnoreCase
                        )
                    }
                }
            }
    )
}

function Assert-FileHash {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$ExpectedSha256,
        [Parameter(Mandatory = $true)][string]$Label
    )

    if (-not [System.IO.File]::Exists($Path)) {
        throw "$Label does not exist: $Path"
    }
    if ($ExpectedSha256 -notmatch '^[0-9A-Fa-f]{64}$') {
        throw "$Label expected SHA256 is not frozen."
    }
    $actual = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToUpperInvariant()
    if ($actual -ne $ExpectedSha256.ToUpperInvariant()) {
        throw "$Label SHA256 mismatch: expected $ExpectedSha256, received $actual"
    }
    return $actual
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

$resolvedProcmonPath = [System.IO.Path]::GetFullPath($ProcmonPath)
$resolvedPmcPath = [System.IO.Path]::GetFullPath($PmcPath)
$resolvedEvidenceRoot = [System.IO.Path]::GetFullPath($EvidenceRoot)
[System.IO.Directory]::CreateDirectory($resolvedEvidenceRoot) | Out-Null

$procmonSha256 = Assert-FileHash -Path $resolvedProcmonPath -ExpectedSha256 $ExpectedProcmonSha256 -Label "ProcMon binary"
$pmcSha256 = Assert-FileHash -Path $resolvedPmcPath -ExpectedSha256 $ExpectedPmcSha256 -Label "ProcMon configuration"
$signer = Assert-MicrosoftSignature -Path $resolvedProcmonPath
$fileVersion = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($resolvedProcmonPath).FileVersion
$conflicts = @(Get-ExactProcmonProcesses -ExpectedPath $resolvedProcmonPath)
if ($conflicts.Count -gt 0) {
    throw "An existing ProcMon process conflicts with capture startup: $($conflicts | ConvertTo-Json -Compress)"
}

if ([System.IO.File]::Exists($activeCapturePath)) {
    $previous = [System.IO.File]::ReadAllText($activeCapturePath) | ConvertFrom-Json
    if ([string]$previous.status -eq "CAPTURING") {
        throw "active-capture.json already records an active capture; stop it before starting another."
    }
}

$preflight = [pscustomobject]@{
    status = "PREFLIGHT_PASS"
    preflightOnly = [bool]$PreflightOnly
    elevated = Test-IsElevated
    procmonPath = $resolvedProcmonPath
    procmonVersion = $fileVersion
    procmonSha256 = $procmonSha256
    procmonSigner = $signer
    pmcPath = $resolvedPmcPath
    pmcSha256 = $pmcSha256
    evidenceRoot = $resolvedEvidenceRoot
    existingProcmonProcessCount = $conflicts.Count
    checkedAt = [DateTimeOffset]::UtcNow.ToString("o")
}

if ($PreflightOnly) {
    $preflight | ConvertTo-Json -Depth 8
    exit 0
}

if (-not (Test-IsElevated)) {
    throw "ProcMon capture startup requires an elevated independent Windows PowerShell."
}

$captureId = "phase-3-5-2a-a3-{0}-{1}" -f [DateTimeOffset]::UtcNow.ToString("yyyyMMddTHHmmssfffZ"), [Guid]::NewGuid().ToString("N").Substring(0, 8)
$pmlPath = Join-Path $resolvedEvidenceRoot "$captureId.raw.pml"
$metadataPath = Join-Path $resolvedEvidenceRoot "$captureId.metadata.json"
$startedAt = [DateTimeOffset]::UtcNow
$startedProcess = $null

try {
    $arguments = @(
        "/AcceptEula"
        "/Quiet"
        "/Minimized"
        "/LoadConfig"
        ('"{0}"' -f $resolvedPmcPath)
        "/BackingFile"
        ('"{0}"' -f $pmlPath)
    )
    $startedProcess = Start-Process -FilePath $resolvedProcmonPath -ArgumentList $arguments -PassThru
    if ($null -eq $startedProcess -or $startedProcess.Id -le 0) {
        throw "Start-Process did not return a valid ProcMon PID."
    }

    $idleProcess = Start-Process -FilePath $resolvedProcmonPath -ArgumentList @("/AcceptEula", "/WaitForIdle", "/Quiet") -PassThru -Wait
    if ($idleProcess.ExitCode -ne 0) {
        throw "ProcMon /WaitForIdle failed with exit code $($idleProcess.ExitCode)."
    }

    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($StartupTimeoutSeconds)
    $observed = $null
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        $observed = @(Get-ExactProcmonProcesses -ExpectedPath $resolvedProcmonPath | Where-Object processId -eq $startedProcess.Id)
        if ($observed.Count -eq 1 -and
            $observed[0].exactBinary -and
            [System.IO.File]::Exists($pmlPath)) {
            break
        }
        Start-Sleep -Milliseconds 250
    }

    if ($observed.Count -ne 1 -or -not $observed[0].exactBinary) {
        throw "The started ProcMon PID is not running from the exact verified binary."
    }
    if (-not [System.IO.File]::Exists($pmlPath)) {
        throw "ProcMon did not initialize the requested PML backing file."
    }

    $capture = [pscustomobject]@{
        status = "CAPTURING"
        captureId = $captureId
        startedAt = $startedAt.ToString("o")
        procmonPid = [int]$startedProcess.Id
        procmonPath = $resolvedProcmonPath
        procmonVersion = $fileVersion
        procmonSha256 = $procmonSha256
        procmonSigner = $signer
        pmcPath = $resolvedPmcPath
        pmcSha256 = $pmcSha256
        pmlPath = $pmlPath
        pmlObservedLength = [System.IO.FileInfo]::new($pmlPath).Length
        metadataPath = $metadataPath
    }
    Write-JsonAtomically -Path $metadataPath -Value $capture
    Write-JsonAtomically -Path $activeCapturePath -Value $capture
    $capture | ConvertTo-Json -Depth 8
} catch {
    if ($null -ne $startedProcess) {
        try {
            Start-Process -FilePath $resolvedProcmonPath -ArgumentList @("/AcceptEula", "/Terminate", "/Quiet") -Wait | Out-Null
        } catch {
            # Preserve the original startup failure; process state is reported by the caller.
        }
    }
    throw
}
