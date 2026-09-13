param(
    [ValidateSet("CODEX_EXECUTOR", "WINDOWS_TERMINAL")]
    [string]$Context = "CODEX_EXECUTOR"
)

$ErrorActionPreference = "Stop"

$profilePath = "C:\Users\DeanX\AppData\Roaming\com.dimillian.codexmonitor.phase351d"
$settingsPath = Join-Path $profilePath "settings.json"

function Get-InnermostException {
    param([Parameter(Mandatory = $true)][System.Exception]$Exception)

    $current = $Exception
    while ($null -ne $current.InnerException) {
        $current = $current.InnerException
    }
    return $current
}

function Get-AttributesDiagnostic {
    param([Parameter(Mandatory = $true)][string]$Path)

    try {
        $attributes = [System.IO.File]::GetAttributes($Path)
        return [pscustomobject]@{
            succeeded = $true
            attributes = [string]$attributes
            exceptionType = $null
            hResult = $null
        }
    } catch {
        $exception = Get-InnermostException -Exception $_.Exception
        return [pscustomobject]@{
            succeeded = $false
            attributes = $null
            exceptionType = $exception.GetType().FullName
            hResult = $exception.HResult
        }
    }
}

if (-not ("Phase352aVisibility.NativeProcessSnapshot" -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;

namespace Phase352aVisibility
{
    public sealed class ProcessEntry
    {
        public int ProcessId { get; set; }
        public int ParentProcessId { get; set; }
        public string Name { get; set; }
    }

    public static class NativeProcessSnapshot
    {
        private const uint TH32CS_SNAPPROCESS = 0x00000002;
        private static readonly IntPtr InvalidHandleValue = new IntPtr(-1);

        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
        private struct PROCESSENTRY32
        {
            public uint dwSize;
            public uint cntUsage;
            public uint th32ProcessID;
            public IntPtr th32DefaultHeapID;
            public uint th32ModuleID;
            public uint cntThreads;
            public uint th32ParentProcessID;
            public int pcPriClassBase;
            public uint dwFlags;

            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 260)]
            public string szExeFile;
        }

        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern IntPtr CreateToolhelp32Snapshot(uint flags, uint processId);

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern bool Process32FirstW(IntPtr snapshot, ref PROCESSENTRY32 entry);

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern bool Process32NextW(IntPtr snapshot, ref PROCESSENTRY32 entry);

        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern bool CloseHandle(IntPtr handle);

        public static ProcessEntry[] Capture()
        {
            IntPtr snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if (snapshot == InvalidHandleValue)
            {
                throw new Win32Exception(Marshal.GetLastWin32Error());
            }

            try
            {
                var result = new List<ProcessEntry>();
                var entry = new PROCESSENTRY32();
                entry.dwSize = (uint)Marshal.SizeOf(typeof(PROCESSENTRY32));
                if (!Process32FirstW(snapshot, ref entry))
                {
                    throw new Win32Exception(Marshal.GetLastWin32Error());
                }

                do
                {
                    result.Add(new ProcessEntry
                    {
                        ProcessId = unchecked((int)entry.th32ProcessID),
                        ParentProcessId = unchecked((int)entry.th32ParentProcessID),
                        Name = entry.szExeFile
                    });
                    entry.dwSize = (uint)Marshal.SizeOf(typeof(PROCESSENTRY32));
                }
                while (Process32NextW(snapshot, ref entry));

                return result.ToArray();
            }
            finally
            {
                CloseHandle(snapshot);
            }
        }
    }
}
'@
}

function Get-ParentProcessChain {
    try {
        $entries = [Phase352aVisibility.NativeProcessSnapshot]::Capture()
        $byPid = @{}
        foreach ($entry in $entries) {
            $byPid[[int]$entry.ProcessId] = $entry
        }

        $chain = [System.Collections.Generic.List[object]]::new()
        $seen = [System.Collections.Generic.HashSet[int]]::new()
        $currentPid = [int]$PID
        while ($currentPid -gt 0 -and $byPid.ContainsKey($currentPid) -and $seen.Add($currentPid)) {
            $entry = $byPid[$currentPid]
            [void]$chain.Add([pscustomobject]@{
                processId = [int]$entry.ProcessId
                parentProcessId = [int]$entry.ParentProcessId
                processName = [string]$entry.Name
            })
            $currentPid = [int]$entry.ParentProcessId
        }

        return [pscustomobject]@{
            succeeded = $true
            chain = @($chain)
            exceptionType = $null
            hResult = $null
        }
    } catch {
        $exception = Get-InnermostException -Exception $_.Exception
        return [pscustomobject]@{
            succeeded = $false
            chain = @()
            exceptionType = $exception.GetType().FullName
            hResult = $exception.HResult
        }
    }
}

function Get-IntegrityDiagnostic {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    $integritySid = $null
    foreach ($group in $identity.Groups) {
        $sid = $group.Translate([Security.Principal.SecurityIdentifier]).Value
        if ($sid -like "S-1-16-*") {
            $integritySid = $sid
            break
        }
    }

    $integrityLevel = "UNKNOWN"
    if (-not [string]::IsNullOrWhiteSpace($integritySid)) {
        $rid = [int]($integritySid.Split("-")[-1])
        $integrityLevel = switch ($rid) {
            { $_ -ge 20480 } { "PROTECTED"; break }
            { $_ -ge 16384 } { "SYSTEM"; break }
            { $_ -ge 12288 } { "HIGH"; break }
            { $_ -ge 8448 } { "MEDIUM_PLUS"; break }
            { $_ -ge 8192 } { "MEDIUM"; break }
            { $_ -ge 4096 } { "LOW"; break }
            default { "UNTRUSTED" }
        }
    }

    return [pscustomobject]@{
        username = $identity.Name
        userSid = $identity.User.Value
        elevated = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
        integritySid = $integritySid
        integrityLevel = $integrityLevel
    }
}

$openRead = $null
$stream = $null
try {
    $stream = [System.IO.File]::OpenRead($settingsPath)
    $openRead = [pscustomobject]@{
        attempted = $true
        succeeded = $true
        exceptionType = $null
        hResult = $null
    }
} catch {
    $exception = Get-InnermostException -Exception $_.Exception
    $openRead = [pscustomobject]@{
        attempted = $true
        succeeded = $false
        exceptionType = $exception.GetType().FullName
        hResult = $exception.HResult
    }
} finally {
    if ($null -ne $stream) {
        $stream.Dispose()
    }
}

$roamingEnumeration = $null
try {
    $matchingProfiles = @(
        [System.IO.Directory]::EnumerateDirectories(
            [string]$env:APPDATA,
            "com.dimillian.codexmonitor*",
            [System.IO.SearchOption]::TopDirectoryOnly
        ) |
            ForEach-Object { [System.IO.Path]::GetFullPath([string]$_) } |
            Sort-Object
    )
    $targetVisible = @(
        $matchingProfiles |
            Where-Object {
                [string]::Equals(
                    $_,
                    $profilePath,
                    [System.StringComparison]::OrdinalIgnoreCase
                )
            }
    ).Count -eq 1
    $roamingEnumeration = [pscustomobject]@{
        succeeded = $true
        matchingProfiles = $matchingProfiles
        targetVisible = $targetVisible
        exceptionType = $null
        hResult = $null
    }
} catch {
    $exception = Get-InnermostException -Exception $_.Exception
    $roamingEnumeration = [pscustomobject]@{
        succeeded = $false
        matchingProfiles = @()
        targetVisible = $false
        exceptionType = $exception.GetType().FullName
        hResult = $exception.HResult
    }
}

$scriptPath = [System.IO.Path]::GetFullPath($PSCommandPath)
$result = [pscustomobject]@{
    context = $Context
    timestamp = [DateTimeOffset]::Now.ToString("o")
    processPid = [int]$PID
    scriptPath = $scriptPath
    scriptSha256 = (Get-FileHash -LiteralPath $scriptPath -Algorithm SHA256).Hash.ToUpperInvariant()
    powerShell = [pscustomobject]@{
        edition = [string]$PSVersionTable.PSEdition
        version = [string]$PSVersionTable.PSVersion
    }
    parentProcessChain = Get-ParentProcessChain
    identity = Get-IntegrityDiagnostic
    environment = [pscustomobject]@{
        currentWorkingDirectory = [System.Environment]::CurrentDirectory
        powerShellLocation = [string](Get-Location)
        userProfile = [string]$env:USERPROFILE
        appData = [string]$env:APPDATA
        localAppData = [string]$env:LOCALAPPDATA
    }
    profile = [pscustomobject]@{
        path = $profilePath
        directoryExists = [System.IO.Directory]::Exists($profilePath)
        getAttributes = Get-AttributesDiagnostic -Path $profilePath
    }
    settings = [pscustomobject]@{
        path = $settingsPath
        fileExists = [System.IO.File]::Exists($settingsPath)
        getAttributes = Get-AttributesDiagnostic -Path $settingsPath
        openRead = $openRead
    }
    roamingDirectEnumeration = $roamingEnumeration
}

$result | ConvertTo-Json -Depth 12
