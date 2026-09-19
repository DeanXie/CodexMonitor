#[cfg(windows)]
use super::legacy_migration_entry::{
    LegacyProcessStopGuard, LegacyProcessStopProvider, StopEvidenceAcquisition, StopEvidenceState,
};
#[cfg(windows)]
use std::path::{Path, PathBuf};

#[cfg(windows)]
pub(crate) struct WindowsLegacyProcessStopProvider {
    expected_executable: PathBuf,
    excluded_pid: u32,
}

#[cfg(windows)]
impl WindowsLegacyProcessStopProvider {
    pub(crate) fn new(expected_executable: PathBuf, excluded_pid: u32) -> Result<Self, String> {
        if !expected_executable.is_absolute() {
            return Err("legacy executable path must be absolute".to_string());
        }
        Ok(Self {
            expected_executable,
            excluded_pid,
        })
    }
}

#[cfg(windows)]
struct WindowsLegacyProcessStopGuard {
    expected_executable: PathBuf,
    excluded_pid: u32,
    source_root: PathBuf,
}

#[cfg(windows)]
impl LegacyProcessStopProvider for WindowsLegacyProcessStopProvider {
    fn acquire(&self, source_root: &Path) -> Result<StopEvidenceAcquisition, String> {
        let source_root = canonical_existing_directory(source_root)?;
        match inspect_processes(&self.expected_executable, self.excluded_pid)? {
            StopEvidenceState::Running => {
                Ok(StopEvidenceAcquisition::Blocked(StopEvidenceState::Running))
            }
            StopEvidenceState::Unknown => {
                Ok(StopEvidenceAcquisition::Blocked(StopEvidenceState::Unknown))
            }
            StopEvidenceState::VerifiedQuiescentWithinSupportedScope => Ok(
                StopEvidenceAcquisition::Verified(Box::new(WindowsLegacyProcessStopGuard {
                    expected_executable: self.expected_executable.clone(),
                    excluded_pid: self.excluded_pid,
                    source_root,
                })),
            ),
        }
    }
}

#[cfg(windows)]
impl LegacyProcessStopGuard for WindowsLegacyProcessStopGuard {
    fn revalidate(&mut self, source_root: &Path) -> Result<StopEvidenceState, String> {
        if canonical_existing_directory(source_root)? != self.source_root {
            return Ok(StopEvidenceState::Unknown);
        }
        inspect_processes(&self.expected_executable, self.excluded_pid)
    }
}

#[cfg(windows)]
fn canonical_existing_directory(path: &Path) -> Result<PathBuf, String> {
    if !path.is_dir() {
        return Err("legacy source root is unavailable".to_string());
    }
    std::fs::canonicalize(path).map_err(|error| format!("bind legacy source root: {error}"))
}

#[cfg(windows)]
fn inspect_processes(
    expected_executable: &Path,
    excluded_pid: u32,
) -> Result<StopEvidenceState, String> {
    use std::mem::size_of;
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_NO_MORE_FILES, FILETIME, INVALID_HANDLE_VALUE,
        WAIT_TIMEOUT,
    };
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, WaitForSingleObject,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    const SYNCHRONIZE_ACCESS: u32 = 0x0010_0000;
    let expected = match std::fs::canonicalize(expected_executable) {
        Ok(path) if path.is_file() => path,
        _ => return Ok(StopEvidenceState::Unknown),
    };
    let Some(expected_name) = expected.file_name().and_then(|name| name.to_str()) else {
        return Ok(StopEvidenceState::Unknown);
    };
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Ok(StopEvidenceState::Unknown);
    }
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
    let mut has_entry = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
    if !has_entry {
        let error = unsafe { GetLastError() };
        unsafe { CloseHandle(snapshot) };
        return Ok(if error == ERROR_NO_MORE_FILES {
            StopEvidenceState::VerifiedQuiescentWithinSupportedScope
        } else {
            StopEvidenceState::Unknown
        });
    }
    let mut result = StopEvidenceState::VerifiedQuiescentWithinSupportedScope;
    while has_entry {
        let pid = entry.th32ProcessID;
        if pid != excluded_pid {
            let end = entry
                .szExeFile
                .iter()
                .position(|value| *value == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
            if name.eq_ignore_ascii_case(expected_name) {
                let process = unsafe {
                    OpenProcess(
                        PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE_ACCESS,
                        0,
                        pid,
                    )
                };
                if process.is_null() {
                    result = StopEvidenceState::Unknown;
                    break;
                }
                let mut path_buffer = vec![0u16; 32768];
                let mut path_len = path_buffer.len() as u32;
                let queried = unsafe {
                    QueryFullProcessImageNameW(process, 0, path_buffer.as_mut_ptr(), &mut path_len)
                };
                let mut creation: FILETIME = unsafe { std::mem::zeroed() };
                let mut exit: FILETIME = unsafe { std::mem::zeroed() };
                let mut kernel: FILETIME = unsafe { std::mem::zeroed() };
                let mut user: FILETIME = unsafe { std::mem::zeroed() };
                let times_ok = unsafe {
                    GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user)
                } != 0;
                if queried == 0 || !times_ok {
                    unsafe { CloseHandle(process) };
                    result = StopEvidenceState::Unknown;
                    break;
                }
                let observed =
                    PathBuf::from(String::from_utf16_lossy(&path_buffer[..path_len as usize]));
                let same_executable = std::fs::canonicalize(&observed)
                    .map(|path| paths_equal(&path, &expected))
                    .unwrap_or(false);
                let still_running = unsafe { WaitForSingleObject(process, 0) } == WAIT_TIMEOUT;
                unsafe { CloseHandle(process) };
                if same_executable && still_running {
                    result = StopEvidenceState::Running;
                    break;
                }
            }
        }
        has_entry = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
        if !has_entry && unsafe { GetLastError() } != ERROR_NO_MORE_FILES {
            result = StopEvidenceState::Unknown;
        }
    }
    unsafe { CloseHandle(snapshot) };
    Ok(result)
}

#[cfg(windows)]
fn paths_equal(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(right.to_string_lossy().as_ref())
}

#[cfg(not(windows))]
pub(crate) struct WindowsLegacyProcessStopProvider;

#[cfg(not(windows))]
impl WindowsLegacyProcessStopProvider {
    pub(crate) fn new(
        _expected_executable: std::path::PathBuf,
        _excluded_pid: u32,
    ) -> Result<Self, String> {
        Err("native legacy process stop evidence is Windows-only".to_string())
    }
}
