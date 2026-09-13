param(
    [switch]$DiagnoseOnly,
    [switch]$PreflightOnly,
    [string]$SettingsPath,
    [string]$TargetId = "phase-3-5-1d-target",
    [string]$ExpectedEndpoint = "127.0.0.1:54060",
    [string]$ExpectedRemoteHostIdentity = "014383f2-41f8-4b13-b9d7-30c511e47cec",
    [string]$DaemonPath = "F:\AI\CodexMonitor\.worktrees\phase-3-5-2a-remote-read-admission\src-tauri\target\phase-3-5-2a-daemon-e2e\debug\codex_monitor_daemon.exe",
    [string]$DaemonDataDirectory = "F:\AI\CodexMonitor\.worktrees\phase-3-5-1d-windows-remote-acceptance\src-tauri\target\phase-3-5-1d-acceptance\daemon-a",
    [int]$StartupTimeoutSeconds = 10
)

$ErrorActionPreference = "Stop"
$startedProcess = $null
$runtimeProvenanceDirectory = Join-Path `
    $PSScriptRoot `
    "src-tauri\target\phase-3-5-2a-a3-diagnostics"

function Get-ProcessParentChain {
    $chain = [System.Collections.Generic.List[object]]::new()
    $currentProcessId = $PID

    for ($depth = 0; $depth -lt 16 -and $currentProcessId -gt 0; $depth += 1) {
        try {
            $process = Get-CimInstance `
                -ClassName Win32_Process `
                -Filter "ProcessId = $currentProcessId" `
                -ErrorAction Stop
        } catch {
            [void]$chain.Add([pscustomobject]@{
                processId = $currentProcessId
                parentProcessId = $null
                name = $null
                observationError = $_.Exception.Message
            })
            break
        }

        if ($null -eq $process) {
            break
        }

        [void]$chain.Add([pscustomobject]@{
            processId = [int]$process.ProcessId
            parentProcessId = [int]$process.ParentProcessId
            name = [string]$process.Name
            observationError = $null
        })

        if ([int]$process.ParentProcessId -le 0 -or
            [int]$process.ParentProcessId -eq $currentProcessId) {
            break
        }
        $currentProcessId = [int]$process.ParentProcessId
    }

    return @($chain)
}

function Write-RuntimeProvenance {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Value
    )

    $directory = Split-Path -Parent $Path
    [void](New-Item -ItemType Directory -Path $directory -Force)

    $temporaryPath = Join-Path `
        $directory `
        (".{0}.{1}.tmp" -f (Split-Path -Leaf $Path), $PID)
    $json = $Value | ConvertTo-Json -Depth 16
    [System.IO.File]::WriteAllText(
        $temporaryPath,
        $json,
        [System.Text.UTF8Encoding]::new($false)
    )

    try {
        if (Test-Path -LiteralPath $Path -PathType Leaf) {
            $backupPath = "{0}.replace-backup-{1}-{2}" -f `
                $Path, `
                $PID, `
                [Guid]::NewGuid().ToString("N")
            [System.IO.File]::Replace($temporaryPath, $Path, $backupPath)
            Remove-Item -LiteralPath $backupPath -Force
        } else {
            [System.IO.File]::Move($temporaryPath, $Path)
        }
    } finally {
        if (Test-Path -LiteralPath $temporaryPath -PathType Leaf) {
            Remove-Item -LiteralPath $temporaryPath -Force
        }
    }
}

function Get-AcceptanceSearchRoots {
    return @(
        @(
            [string]$env:APPDATA
            [string]$env:LOCALAPPDATA
        ) |
            Where-Object { -not [string]::IsNullOrWhiteSpace($_) } |
            Sort-Object -Unique
    )
}

function New-SettingsDiscoveryError {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Stage,
        [Parameter(Mandatory = $true)][System.Exception]$Exception
    )

    $category = if ($Exception -is [System.UnauthorizedAccessException]) {
        "PermissionDenied"
    } elseif ($Exception -is [System.IO.DirectoryNotFoundException] -or
        $Exception -is [System.IO.FileNotFoundException]) {
        "ObjectNotFound"
    } elseif ($Exception -is [System.IO.IOException]) {
        "ReadError"
    } else {
        "InvalidOperation"
    }

    return [pscustomobject]@{
        path = $Path
        stage = $Stage
        exceptionType = $Exception.GetType().FullName
        category = $category
        providerError = $null
        message = $Exception.Message
    }
}

function Get-InnermostException {
    param(
        [Parameter(Mandatory = $true)][System.Exception]$Exception
    )

    $current = $Exception
    while ($null -ne $current.InnerException) {
        $current = $current.InnerException
    }
    return $current
}

function Get-FileSystemAttributesDiagnostic {
    param(
        [Parameter(Mandatory = $true)][string]$Path
    )

    try {
        $attributes = [System.IO.File]::GetAttributes($Path)
        return [pscustomobject]@{
            succeeded = $true
            attributes = $attributes.ToString()
            exceptionType = $null
            hResult = $null
            message = $null
        }
    } catch {
        $exception = Get-InnermostException -Exception $_.Exception
        return [pscustomobject]@{
            succeeded = $false
            attributes = $null
            exceptionType = $exception.GetType().FullName
            hResult = $exception.HResult
            message = $exception.Message
        }
    }
}

function Get-SettingsPathVisibilityDiagnostic {
    param(
        [Parameter(Mandatory = $true)][string]$ResolvedSettingsPath
    )

    try {
        $directoryPaths = [System.Collections.Generic.List[string]]::new()
        $currentDirectory = [System.IO.Path]::GetDirectoryName($ResolvedSettingsPath)
        while (-not [string]::IsNullOrWhiteSpace($currentDirectory)) {
            [void]$directoryPaths.Add($currentDirectory)
            $parent = [System.IO.Directory]::GetParent($currentDirectory)
            if ($null -eq $parent) {
                break
            }
            $currentDirectory = $parent.FullName
        }

        $orderedDirectoryPaths = @($directoryPaths.ToArray())
        [Array]::Reverse($orderedDirectoryPaths)
        $directoryDiagnostics = @(
            $orderedDirectoryPaths | ForEach-Object {
                [pscustomobject]@{
                    path = [string]$_
                    directoryExists = [System.IO.Directory]::Exists([string]$_)
                    getAttributes = Get-FileSystemAttributesDiagnostic -Path ([string]$_)
                }
            }
        )

        $settingsAttributes = Get-FileSystemAttributesDiagnostic -Path $ResolvedSettingsPath
        $profileDirectory = [System.IO.Path]::GetDirectoryName($ResolvedSettingsPath)
        $roamingPath = if ([string]::IsNullOrWhiteSpace([string]$env:APPDATA)) {
            $null
        } else {
            [System.IO.Path]::GetFullPath([string]$env:APPDATA)
        }
        $roamingEnumeration = [pscustomobject]@{
            path = $roamingPath
            succeeded = $false
            matchingProfileDirectories = @()
            acceptanceProfileVisible = $false
            exceptionType = $null
            hResult = $null
            message = $null
        }

        if (-not [string]::IsNullOrWhiteSpace($roamingPath)) {
            try {
                $matchingProfileDirectories = @(
                    [System.IO.Directory]::EnumerateDirectories(
                        $roamingPath,
                        "com.dimillian.codexmonitor*",
                        [System.IO.SearchOption]::TopDirectoryOnly
                    ) | ForEach-Object {
                        [pscustomobject]@{
                            name = [System.IO.Path]::GetFileName([string]$_)
                            fullName = [System.IO.Path]::GetFullPath([string]$_)
                        }
                    }
                )
                $roamingEnumeration.succeeded = $true
                $roamingEnumeration.matchingProfileDirectories = $matchingProfileDirectories
                $roamingEnumeration.acceptanceProfileVisible = [bool]@(
                    $matchingProfileDirectories | Where-Object {
                        [string]::Equals(
                            [string]$_.fullName,
                            [string]$profileDirectory,
                            [System.StringComparison]::OrdinalIgnoreCase
                        )
                    }
                ).Count
            } catch {
                $exception = Get-InnermostException -Exception $_.Exception
                $roamingEnumeration.exceptionType = $exception.GetType().FullName
                $roamingEnumeration.hResult = $exception.HResult
                $roamingEnumeration.message = $exception.Message
            }
        }

        return [pscustomobject]@{
            settingsPath = $ResolvedSettingsPath
            getFullPathResult = [System.IO.Path]::GetFullPath($ResolvedSettingsPath)
            parentDirectories = $directoryDiagnostics
            settingsFile = [pscustomobject]@{
                path = $ResolvedSettingsPath
                fileExists = [System.IO.File]::Exists($ResolvedSettingsPath)
                getAttributes = $settingsAttributes
            }
            roamingProfileEnumeration = $roamingEnumeration
            diagnosticError = $null
        }
    } catch {
        $exception = Get-InnermostException -Exception $_.Exception
        return [pscustomobject]@{
            settingsPath = $ResolvedSettingsPath
            getFullPathResult = $null
            parentDirectories = @()
            settingsFile = $null
            roamingProfileEnumeration = $null
            diagnosticError = [pscustomobject]@{
                exceptionType = $exception.GetType().FullName
                hResult = $exception.HResult
                message = $exception.Message
            }
        }
    }
}

function Invoke-SettingsOpenReadDiagnostic {
    param(
        [Parameter(Mandatory = $true)][string]$ResolvedSettingsPath
    )

    $stream = $null
    try {
        $stream = [System.IO.File]::OpenRead($ResolvedSettingsPath)
        return [pscustomobject]@{
            attempted = $true
            succeeded = $true
            exceptionType = $null
            hResult = $null
            message = $null
        }
    } catch {
        $exception = Get-InnermostException -Exception $_.Exception
        return [pscustomobject]@{
            attempted = $true
            succeeded = $false
            exceptionType = $exception.GetType().FullName
            hResult = $exception.HResult
            message = $exception.Message
        }
    } finally {
        if ($null -ne $stream) {
            $stream.Dispose()
        }
    }
}

function Resolve-AcceptanceSettings {
    param(
        [Parameter(Mandatory = $true)][string]$ExactTargetId,
        [Parameter(Mandatory = $true)][string]$ExactEndpoint,
        [Parameter(Mandatory = $true)][string]$ExactRemoteHostIdentity,
        [Parameter(Mandatory = $true)][string]$ActualScriptPath
    )

    $searchRoots = @(Get-AcceptanceSearchRoots)
    $candidatePaths = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::OrdinalIgnoreCase
    )
    $searchRootDiagnostics = [System.Collections.Generic.List[object]]::new()
    $discoveryErrors = [System.Collections.Generic.List[object]]::new()
    foreach ($root in $searchRoots) {
        if (-not [System.IO.Directory]::Exists($root)) {
            $errorDiagnostic = New-SettingsDiscoveryError `
                -Path $root `
                -Stage "SEARCH_ROOT" `
                -Exception ([System.IO.DirectoryNotFoundException]::new(
                    "Settings search root does not exist."
                ))
            [void]$discoveryErrors.Add($errorDiagnostic)
            [void]$searchRootDiagnostics.Add([pscustomobject]@{
                path = $root
                exists = $false
                enumerationSucceeded = $false
                profileDirectoryCount = 0
            })
            continue
        }

        try {
            $profileDirectories = @(
                [System.IO.Directory]::EnumerateDirectories(
                    $root,
                    "com.dimillian.codexmonitor*",
                    [System.IO.SearchOption]::TopDirectoryOnly
                )
            )
        } catch {
            $errorDiagnostic = New-SettingsDiscoveryError `
                -Path $root `
                -Stage "ENUMERATE_PROFILES" `
                -Exception $_.Exception
            [void]$discoveryErrors.Add($errorDiagnostic)
            [void]$searchRootDiagnostics.Add([pscustomobject]@{
                path = $root
                exists = $true
                enumerationSucceeded = $false
                profileDirectoryCount = 0
            })
            continue
        }

        [void]$searchRootDiagnostics.Add([pscustomobject]@{
            path = $root
            exists = $true
            enumerationSucceeded = $true
            profileDirectoryCount = $profileDirectories.Count
        })

        foreach ($profileDirectory in $profileDirectories) {
            $candidatePath = [System.IO.Path]::Combine($profileDirectory, "settings.json")
            try {
                if ([System.IO.File]::Exists($candidatePath)) {
                    [void]$candidatePaths.Add([System.IO.Path]::GetFullPath($candidatePath))
                }
            } catch {
                [void]$discoveryErrors.Add((New-SettingsDiscoveryError `
                    -Path $candidatePath `
                    -Stage "CHECK_SETTINGS_FILE" `
                    -Exception $_.Exception))
            }
        }
    }

    $candidateDiagnostics = [System.Collections.Generic.List[object]]::new()
    $exactMatches = [System.Collections.Generic.List[object]]::new()
    $exactMatchCount = 0
    foreach ($path in @($candidatePaths | Sort-Object)) {
        try {
            $settingsJson = [System.IO.File]::ReadAllText($path)
        } catch {
            $errorDiagnostic = New-SettingsDiscoveryError `
                -Path $path `
                -Stage "READ_SETTINGS_FILE" `
                -Exception $_.Exception
            [void]$discoveryErrors.Add($errorDiagnostic)
            $candidateDiagnostics.Add([pscustomobject]@{
                path = $path
                readable = $false
                backendMode = $null
                activeRemoteBackendId = $null
                remoteBackendHost = $null
                availableTargetIds = @()
                matchedTargetId = $null
                targetHost = $null
                targetRemoteHostIdentity = $null
                tokenPresent = $false
                mismatchFields = @("readable")
                error = $errorDiagnostic
            })
            continue
        }

        try {
            $settings = $settingsJson | ConvertFrom-Json
        } catch {
            $errorDiagnostic = [pscustomobject]@{
                path = $path
                stage = "PARSE_SETTINGS_FILE"
                exceptionType = $_.Exception.GetType().FullName
                category = "InvalidData"
                providerError = $null
                message = "settings.json is not valid JSON."
            }
            [void]$discoveryErrors.Add($errorDiagnostic)
            $candidateDiagnostics.Add([pscustomobject]@{
                path = $path
                readable = $false
                backendMode = $null
                activeRemoteBackendId = $null
                remoteBackendHost = $null
                availableTargetIds = @()
                matchedTargetId = $null
                targetHost = $null
                targetRemoteHostIdentity = $null
                tokenPresent = $false
                mismatchFields = @("readable")
                error = $errorDiagnostic
            })
            continue
        }

        $targets = @($settings.remoteBackends)
        $idMatches = @($targets | Where-Object { [string]$_.id -eq $ExactTargetId })
        $mismatchFields = [System.Collections.Generic.List[string]]::new()
        $matchedTarget = $null
        if ($idMatches.Count -eq 0) {
            $mismatchFields.Add("targetId")
        } elseif ($idMatches.Count -gt 1) {
            $mismatchFields.Add("targetCardinality")
        } else {
            $matchedTarget = $idMatches[0]
            if ([string]$matchedTarget.host -ne $ExactEndpoint) {
                $mismatchFields.Add("endpoint")
            }
            if ([string]$matchedTarget.remoteHostIdentity -ne $ExactRemoteHostIdentity) {
                $mismatchFields.Add("remoteHostIdentity")
            }
        }

        $tokenPresent = [bool]([string]$settings.remoteBackendToken)
        if ($null -ne $matchedTarget -and [string]$matchedTarget.token) {
            $tokenPresent = $true
        }

        $token = [string]$matchedTarget.token
        if (-not $token) {
            $token = [string]$settings.remoteBackendToken
        }

        if ($mismatchFields.Count -eq 0) {
            $exactMatchCount += 1
            $exactMatches.Add([pscustomobject]@{
                settingsPath = $path
                target = $matchedTarget
                token = $token
            })
        }

        $candidateDiagnostics.Add([pscustomobject]@{
            path = $path
            readable = $true
            backendMode = [string]$settings.backendMode
            activeRemoteBackendId = [string]$settings.activeRemoteBackendId
            remoteBackendHost = [string]$settings.remoteBackendHost
            availableTargetIds = @($targets | ForEach-Object { [string]$_.id })
            matchedTargetId = if ($null -ne $matchedTarget) { [string]$matchedTarget.id } else { $null }
            targetHost = if ($null -ne $matchedTarget) { [string]$matchedTarget.host } else { $null }
            targetRemoteHostIdentity = if ($null -ne $matchedTarget) {
                [string]$matchedTarget.remoteHostIdentity
            } else {
                $null
            }
            tokenPresent = $tokenPresent
            mismatchFields = @($mismatchFields)
            error = $null
        })
    }

    $exactMatch = if ($exactMatchCount -eq 1) { $exactMatches[0] } else { $null }
    return [pscustomobject]@{
        diagnostics = [pscustomobject]@{
            settingsSource = "DISCOVERY"
            psCommandPath = $ActualScriptPath
            scriptSha256 = (Get-FileHash -LiteralPath $ActualScriptPath -Algorithm SHA256).Hash
            powerShellEdition = [string]$PSVersionTable.PSEdition
            powerShellVersion = $PSVersionTable.PSVersion.ToString()
            currentUser = [System.Security.Principal.WindowsIdentity]::GetCurrent().Name
            userProfile = [string]$env:USERPROFILE
            appData = [string]$env:APPDATA
            localAppData = [string]$env:LOCALAPPDATA
            currentWorkingDirectory = (Get-Location).Path
            expectedTargetId = $ExactTargetId
            expectedEndpoint = $ExactEndpoint
            expectedRemoteHostIdentity = $ExactRemoteHostIdentity
            searchRoots = $searchRoots
            searchRootDiagnostics = @($searchRootDiagnostics)
            discoveryErrors = @($discoveryErrors)
            candidateSettingsPaths = @($candidatePaths | Sort-Object)
            candidates = @($candidateDiagnostics)
            candidateCount = $candidatePaths.Count
            exactMatchCount = $exactMatchCount
            exactMatchPath = if ($null -ne $exactMatch) { [string]$exactMatch.settingsPath } else { $null }
            matchedTargetId = if ($null -ne $exactMatch) { [string]$exactMatch.target.id } else { $null }
            matchedEndpoint = if ($null -ne $exactMatch) { [string]$exactMatch.target.host } else { $null }
            matchedRemoteHostIdentity = if ($null -ne $exactMatch) {
                [string]$exactMatch.target.remoteHostIdentity
            } else {
                $null
            }
            tokenPresent = [bool]($null -ne $exactMatch -and [string]$exactMatch.token)
        }
        settingsPath = if ($null -ne $exactMatch) { [string]$exactMatch.settingsPath } else { $null }
        target = if ($null -ne $exactMatch) { $exactMatch.target } else { $null }
        token = if ($null -ne $exactMatch) { [string]$exactMatch.token } else { $null }
    }
}

function Resolve-ExplicitAcceptanceSettings {
    param(
        [string]$ExplicitSettingsPath,
        [Parameter(Mandatory = $true)][string]$ExactTargetId,
        [Parameter(Mandatory = $true)][string]$ExactEndpoint,
        [Parameter(Mandatory = $true)][string]$ExactRemoteHostIdentity,
        [Parameter(Mandatory = $true)][string]$ActualScriptPath
    )

    $mismatchFields = [System.Collections.Generic.List[string]]::new()
    $settingsError = $null
    $resolvedSettingsPath = $null
    $readAttempted = $false
    $readSucceeded = $false
    $pathVisibility = $null
    $openReadProbe = $null
    $settings = $null
    $matchedTarget = $null
    $token = $null

    if ([string]::IsNullOrWhiteSpace($ExplicitSettingsPath)) {
        [void]$mismatchFields.Add("settingsPath")
        $settingsError = [pscustomobject]@{
            stage = "VALIDATE_EXPLICIT_SETTINGS_PATH"
            exceptionType = $null
            category = "InvalidArgument"
            message = "SettingsPath is required for PreflightOnly and Normal execution."
        }
    } else {
        try {
            $resolvedSettingsPath = [System.IO.Path]::GetFullPath($ExplicitSettingsPath)
        } catch {
            [void]$mismatchFields.Add("settingsPath")
            $settingsError = [pscustomobject]@{
                stage = "VALIDATE_EXPLICIT_SETTINGS_PATH"
                exceptionType = $_.Exception.GetType().FullName
                category = "InvalidArgument"
                message = $_.Exception.Message
            }
        }
    }

    if ($null -ne $resolvedSettingsPath -and $mismatchFields.Count -eq 0) {
        $readAttempted = $true
        try {
            $settingsJson = [System.IO.File]::ReadAllText($resolvedSettingsPath)
            $readSucceeded = $true
        } catch {
            [void]$mismatchFields.Add("readable")
            $readException = Get-InnermostException -Exception $_.Exception
            $readCategory = if ($readException -is [System.UnauthorizedAccessException]) {
                "PermissionDenied"
            } elseif ($readException -is [System.IO.FileNotFoundException] -or
                $readException -is [System.IO.DirectoryNotFoundException]) {
                "ObjectNotFound"
            } elseif ($readException -is [System.IO.IOException]) {
                "ReadError"
            } else {
                "InvalidOperation"
            }
            $settingsError = [pscustomobject]@{
                stage = "READ_EXPLICIT_SETTINGS_FILE"
                settingsPath = $resolvedSettingsPath
                exceptionType = $readException.GetType().FullName
                hResult = $readException.HResult
                category = $readCategory
                message = $readException.Message
            }
            $pathVisibility = Get-SettingsPathVisibilityDiagnostic `
                -ResolvedSettingsPath $resolvedSettingsPath
            $openReadProbe = Invoke-SettingsOpenReadDiagnostic `
                -ResolvedSettingsPath $resolvedSettingsPath
        }
    }

    if ($readSucceeded) {
        try {
            $settings = $settingsJson | ConvertFrom-Json
        } catch {
            [void]$mismatchFields.Add("validJson")
            $parseException = $_.Exception
            $settingsError = [pscustomobject]@{
                stage = "PARSE_EXPLICIT_SETTINGS_FILE"
                settingsPath = $resolvedSettingsPath
                exceptionType = $parseException.GetType().FullName
                hResult = $parseException.HResult
                category = "InvalidData"
                message = "The explicit settings file is not valid JSON."
            }
        }
    }

    if ($null -ne $settings) {
        if ([string]$settings.backendMode -ne "remote") {
            [void]$mismatchFields.Add("backendMode")
        }
        if ([string]$settings.activeRemoteBackendId -ne $ExactTargetId) {
            [void]$mismatchFields.Add("activeRemoteBackendId")
        }

        $targets = @($settings.remoteBackends)
        $idMatches = @($targets | Where-Object { [string]$_.id -eq $ExactTargetId })
        if ($idMatches.Count -eq 0) {
            [void]$mismatchFields.Add("targetId")
        } elseif ($idMatches.Count -gt 1) {
            [void]$mismatchFields.Add("targetCardinality")
        } else {
            $matchedTarget = $idMatches[0]
            if ([string]$matchedTarget.host -ne $ExactEndpoint) {
                [void]$mismatchFields.Add("endpoint")
            }
            if ([string]$matchedTarget.remoteHostIdentity -ne $ExactRemoteHostIdentity) {
                [void]$mismatchFields.Add("remoteHostIdentity")
            }
        }

        if ($null -ne $matchedTarget) {
            $token = [string]$matchedTarget.token
        }
        if ([string]::IsNullOrWhiteSpace($token)) {
            $token = [string]$settings.remoteBackendToken
        }
        if ([string]::IsNullOrWhiteSpace($token)) {
            [void]$mismatchFields.Add("tokenPresent")
            $token = $null
        }
    }

    $isValid = $mismatchFields.Count -eq 0
    return [pscustomobject]@{
        diagnostics = [pscustomobject]@{
            settingsSource = "EXPLICIT"
            settingsPath = $resolvedSettingsPath
            readAttempted = $readAttempted
            readSucceeded = $readSucceeded
            pathVisibility = $pathVisibility
            openReadProbe = $openReadProbe
            exceptionType = if ($null -ne $settingsError) { $settingsError.exceptionType } else { $null }
            exceptionHResult = if ($null -ne $settingsError) { $settingsError.hResult } else { $null }
            exceptionMessage = if ($null -ne $settingsError) { $settingsError.message } else { $null }
            psCommandPath = $ActualScriptPath
            scriptSha256 = (Get-FileHash -LiteralPath $ActualScriptPath -Algorithm SHA256).Hash
            powerShellEdition = [string]$PSVersionTable.PSEdition
            powerShellVersion = $PSVersionTable.PSVersion.ToString()
            currentUser = [System.Security.Principal.WindowsIdentity]::GetCurrent().Name
            userProfile = [string]$env:USERPROFILE
            appData = [string]$env:APPDATA
            localAppData = [string]$env:LOCALAPPDATA
            currentWorkingDirectory = (Get-Location).Path
            expectedTargetId = $ExactTargetId
            expectedEndpoint = $ExactEndpoint
            expectedRemoteHostIdentity = $ExactRemoteHostIdentity
            backendMode = if ($null -ne $settings) { [string]$settings.backendMode } else { $null }
            activeRemoteBackendId = if ($null -ne $settings) {
                [string]$settings.activeRemoteBackendId
            } else {
                $null
            }
            matchedTargetId = if ($null -ne $matchedTarget) { [string]$matchedTarget.id } else { $null }
            matchedEndpoint = if ($null -ne $matchedTarget) { [string]$matchedTarget.host } else { $null }
            matchedRemoteHostIdentity = if ($null -ne $matchedTarget) {
                [string]$matchedTarget.remoteHostIdentity
            } else {
                $null
            }
            tokenPresent = [bool](-not [string]::IsNullOrWhiteSpace($token))
            mismatchFields = @($mismatchFields)
            error = $settingsError
        }
        settingsPath = $resolvedSettingsPath
        target = $matchedTarget
        token = $token
        valid = $isValid
    }
}

function Test-TcpEndpointReachable {
    param(
        [Parameter(Mandatory = $true)][string]$HostName,
        [Parameter(Mandatory = $true)][int]$Port,
        [int]$TimeoutMilliseconds = 500
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

function Get-TcpListenerOwnerProcessIds {
    param(
        [Parameter(Mandatory = $true)][string]$HostName,
        [Parameter(Mandatory = $true)][int]$Port
    )

    $netstatPath = Join-Path $env:SystemRoot "System32\netstat.exe"
    if (-not (Test-Path -LiteralPath $netstatPath -PathType Leaf)) {
        throw "netstat.exe is unavailable; listener ownership cannot be verified."
    }

    $netstatOutput = @(& $netstatPath -ano -p TCP)
    if ($LASTEXITCODE -ne 0) {
        throw "netstat.exe failed; listener ownership cannot be verified."
    }

    $escapedEndpoint = [regex]::Escape("${HostName}:$Port")
    $listenerPattern = "^\s*TCP\s+$escapedEndpoint\s+\S+\s+LISTENING\s+(\d+)\s*$"
    return @($netstatOutput |
        ForEach-Object {
            if ([string]$_ -match $listenerPattern) {
                [int]$Matches[1]
            }
        } |
        Sort-Object -Unique)
}

function Invoke-DaemonRpc {
    param(
        [Parameter(Mandatory = $true)][int]$Id,
        [Parameter(Mandatory = $true)][string]$Method,
        [Parameter(Mandatory = $true)]$Params,
        [Parameter(Mandatory = $true)][System.IO.StreamWriter]$Writer,
        [Parameter(Mandatory = $true)][System.IO.StreamReader]$Reader
    )

    $request = @{ id = $Id; method = $Method; params = $Params } | ConvertTo-Json -Compress -Depth 8
    $Writer.WriteLine($request)
    while ($true) {
        $line = $Reader.ReadLine()
        if ($null -eq $line) {
            throw "Daemon transport ended during startup verification."
        }
        $message = $line | ConvertFrom-Json
        if ($message.id -eq $Id) {
            return $message
        }
    }
}

try {
    if ($DiagnoseOnly -and $PreflightOnly) {
        throw "DiagnoseOnly and PreflightOnly are mutually exclusive."
    }

    if ($DiagnoseOnly) {
        $settingsResolution = Resolve-AcceptanceSettings `
            -ExactTargetId $TargetId `
            -ExactEndpoint $ExpectedEndpoint `
            -ExactRemoteHostIdentity $ExpectedRemoteHostIdentity `
            -ActualScriptPath $PSCommandPath
    } else {
        $settingsResolution = Resolve-ExplicitAcceptanceSettings `
            -ExplicitSettingsPath $SettingsPath `
            -ExactTargetId $TargetId `
            -ExactEndpoint $ExpectedEndpoint `
            -ExactRemoteHostIdentity $ExpectedRemoteHostIdentity `
            -ActualScriptPath $PSCommandPath
    }
    $settingsDiagnostics = $settingsResolution.diagnostics

    $parentChain = @(Get-ProcessParentChain)
    $settingsDiagnostics | Add-Member -NotePropertyName capturedAtUtc `
        -NotePropertyValue ([DateTimeOffset]::UtcNow.ToString("O"))
    $settingsDiagnostics | Add-Member -NotePropertyName processPid `
        -NotePropertyValue $PID
    $settingsDiagnostics | Add-Member -NotePropertyName parentProcessId `
        -NotePropertyValue $(if ($parentChain.Count -gt 0) {
            $parentChain[0].parentProcessId
        } else {
            $null
        })
    $settingsDiagnostics | Add-Member -NotePropertyName parentChain `
        -NotePropertyValue $parentChain

    if (-not $DiagnoseOnly) {
        $executionMode = if ($PreflightOnly) { "PREFLIGHT" } else { "NORMAL" }
        $settingsDiagnostics | Add-Member -NotePropertyName executionMode `
            -NotePropertyValue $executionMode
        $runtimeProvenancePath = Join-Path `
            $runtimeProvenanceDirectory `
            $(if ($PreflightOnly) {
                "preflight-path-provenance.json"
            } else {
                "normal-path-provenance.json"
            })
        Write-RuntimeProvenance `
            -Path $runtimeProvenancePath `
            -Value $settingsDiagnostics
    }

    if ($DiagnoseOnly) {
        $settingsDiagnostics | ConvertTo-Json -Depth 12
        if ([int]$settingsDiagnostics.exactMatchCount -ne 1) {
            throw "DiagnoseOnly failed closed because exactMatchCount was $($settingsDiagnostics.exactMatchCount), not 1."
        }
        return
    }

    if (-not [bool]$settingsResolution.valid) {
        $mismatchSummary = @($settingsDiagnostics.mismatchFields) -join ", "
        throw "Explicit SettingsPath validation failed closed: $mismatchSummary."
    }

    if (-not (Test-Path -LiteralPath $DaemonPath -PathType Leaf)) {
        throw "Acceptance daemon executable was not found."
    }
    if (-not (Test-Path -LiteralPath $DaemonDataDirectory -PathType Container)) {
        throw "Acceptance daemon data directory was not found."
    }

    $identityPath = Join-Path $DaemonDataDirectory "remote-host-identity.json"
    if (-not (Test-Path -LiteralPath $identityPath -PathType Leaf)) {
        throw "Remote HostIdentity store was not found."
    }
    $identityStore = [System.IO.File]::ReadAllText($identityPath) | ConvertFrom-Json
    if ([int]$identityStore.schemaVersion -ne 1 -or
        [string]$identityStore.remoteHostIdentity -ne $ExpectedRemoteHostIdentity) {
        throw "Remote HostIdentity store does not match the expected Host A identity."
    }

    $token = [string]$settingsResolution.token

    if ($ExpectedEndpoint -notmatch "^([^:]+):(\d+)$") {
        throw "Expected endpoint is invalid."
    }
    $hostName = $Matches[1]
    $port = [int]$Matches[2]

    if (Test-TcpEndpointReachable -HostName $hostName -Port $port) {
        throw "Endpoint $ExpectedEndpoint already has a listener; refusing to start a duplicate daemon."
    }

    $settingsDiagnostics | ConvertTo-Json -Depth 12
    if ($PreflightOnly) {
        return
    }

    $stdoutPath = Join-Path $DaemonDataDirectory "daemon.phase-3-5-2a-a3.stdout.log"
    $stderrPath = Join-Path $DaemonDataDirectory "daemon.phase-3-5-2a-a3.stderr.log"
    $startedProcess = Start-Process `
        -FilePath $DaemonPath `
        -ArgumentList @(
            "--listen", $ExpectedEndpoint,
            "--data-dir", $DaemonDataDirectory,
            "--token", $token
        ) `
        -RedirectStandardOutput $stdoutPath `
        -RedirectStandardError $stderrPath `
        -WindowStyle Hidden `
        -PassThru

    if ($null -eq $startedProcess -or $startedProcess.Id -le 0) {
        throw "Start-Process did not return a valid daemon process."
    }

    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($StartupTimeoutSeconds)
    $listenerReady = $false
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        if ($startedProcess.HasExited) {
            throw "Acceptance daemon exited before opening its listener."
        }
        if (Test-TcpEndpointReachable -HostName $hostName -Port $port) {
            $listenerOwnerProcessIds = @(Get-TcpListenerOwnerProcessIds `
                -HostName $hostName `
                -Port $port)
            if ($listenerOwnerProcessIds.Count -ne 1 -or
                [int]$listenerOwnerProcessIds[0] -ne $startedProcess.Id) {
                throw "Endpoint listener is not owned exclusively by the started acceptance daemon PID."
            }
            $listenerReady = $true
            break
        }
        Start-Sleep -Milliseconds 200
    }
    if (-not $listenerReady) {
        throw "Acceptance daemon did not open its listener within the bounded startup timeout."
    }

    $tcp = [System.Net.Sockets.TcpClient]::new()
    try {
        $tcp.Connect($hostName, $port)
        $stream = $tcp.GetStream()
        $writer = [System.IO.StreamWriter]::new($stream, [System.Text.UTF8Encoding]::new($false))
        $writer.AutoFlush = $true
        $reader = [System.IO.StreamReader]::new($stream, [System.Text.UTF8Encoding]::new($false))

        $auth = Invoke-DaemonRpc -Id 1 -Method "auth" -Params @{ token = $token } -Writer $writer -Reader $reader
        if ($auth.error) {
            throw "Acceptance daemon rejected the configured token."
        }

        $daemonInfo = Invoke-DaemonRpc -Id 2 -Method "daemon_info" -Params @{} -Writer $writer -Reader $reader
        if ($daemonInfo.error) {
            throw "Authenticated daemon_info failed."
        }
        if ([string]$daemonInfo.result.name -ne "codex-monitor-daemon" -or
            [string]$daemonInfo.result.mode -ne "tcp" -or
            [int]$daemonInfo.result.protocolVersion -ne 1 -or
            [string]$daemonInfo.result.remoteHostIdentity -ne $ExpectedRemoteHostIdentity) {
            throw "Authenticated daemon_info did not match the expected service/protocol/HostIdentity."
        }
    } finally {
        if ($null -ne $tcp) {
            $tcp.Dispose()
        }
    }

    Write-Host "Acceptance settings matched: $($settingsResolution.settingsPath)"
    Write-Host "Acceptance daemon verified: PID=$($startedProcess.Id), listenerOwnerPid=$($listenerOwnerProcessIds[0]), endpoint=$ExpectedEndpoint, RemoteHostIdentity=$ExpectedRemoteHostIdentity"
    Write-Host "The authentication token was not printed. A3 runner was not started."
    return
} catch {
    if ($null -ne $startedProcess -and -not $startedProcess.HasExited) {
        Stop-Process -Id $startedProcess.Id -Force -ErrorAction SilentlyContinue
    }
    throw $_.Exception.Message
}
