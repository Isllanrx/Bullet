use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::core::PWSTR;

use tracing::debug;

use crate::error::PlatformError;

pub struct ProcessFinder;

impl ProcessFinder {
    /// Retrieve the full executable file path of a process by its PID.
    pub fn get_process_path(pid: u32) -> Result<Option<PathBuf>, PlatformError> {
        // SAFETY: OpenProcess with PROCESS_QUERY_LIMITED_INFORMATION to read the image path.
        let handle = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
            Ok(h) => h,
            Err(_) => return Ok(None),
        };

        let mut buffer = [0u16; 1024];
        let mut size = buffer.len() as u32;

        // SAFETY: QueryFullProcessImageNameW writes the null-terminated wide string into buffer.
        let res = unsafe {
            QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_FORMAT(0),
                PWSTR(buffer.as_mut_ptr()),
                &mut size,
            )
        };

        // SAFETY: Always close the process handle.
        unsafe {
            let _ = CloseHandle(handle); // ignore-ok: handle released at teardown; nothing to recover from
        };

        if res.is_ok() && size > 0 {
            let path_os = OsString::from_wide(&buffer[..size as usize]);
            Ok(Some(PathBuf::from(path_os)))
        } else {
            Ok(None)
        }
    }

    /// Find the executable path of a process by its name (e.g. "LeagueClient.exe").
    pub fn find_process_path(exe_name: &str) -> Result<Option<PathBuf>, PlatformError> {
        if let Some(pid) = Self::find_process_by_name(exe_name)? {
            Self::get_process_path(pid)
        } else {
            Ok(None)
        }
    }
    /// Find the Process ID (PID) of an executable by image name (e.g. "League of Legends.exe").
    pub fn find_process_by_name(exe_name: &str) -> Result<Option<u32>, PlatformError> {
        // SAFETY: CreateToolhelp32Snapshot with TH32CS_SNAPPROCESS takes a snapshot of all processes.
        // The returned handle is checked for INVALID_HANDLE_VALUE and closed before return.
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }?;

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        // SAFETY: Process32FirstW is called with a valid snapshot handle and initialized struct.
        let mut has_next = unsafe { Process32FirstW(snapshot, &mut entry) }.is_ok();

        let mut found_pid = None;
        while has_next {
            // Find nul-terminator in UTF-16 array
            let len = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            let name_os = OsString::from_wide(&entry.szExeFile[..len]);

            if let Some(name_str) = name_os.to_str() {
                if name_str.eq_ignore_ascii_case(exe_name) {
                    found_pid = Some(entry.th32ProcessID);
                    break;
                }
            }

            // SAFETY: Process32NextW is called on valid snapshot handle until failure.
            has_next = unsafe { Process32NextW(snapshot, &mut entry) }.is_ok();
        }

        // SAFETY: Close the snapshot handle.
        unsafe {
            let _ = CloseHandle(snapshot); // ignore-ok: the snapshot is released either way
        };

        match found_pid {
            Some(pid) => debug!(exe = exe_name, pid, "Process found"),
            // Not finding the game or the client is an ordinary state (they are not running), so
            // this stays at debug — but it is no longer invisible.
            None => debug!(exe = exe_name, "Process not running"),
        }

        Ok(found_pid)
    }

    /// Find the first thread ID belonging to a given process ID.
    pub fn find_first_thread_id(pid: u32) -> Result<Option<u32>, PlatformError> {
        // SAFETY: CreateToolhelp32Snapshot with TH32CS_SNAPTHREAD takes a snapshot of all threads.
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }?;

        let mut entry = THREADENTRY32 {
            dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };

        // SAFETY: Thread32First called on valid snapshot.
        let mut has_next = unsafe { Thread32First(snapshot, &mut entry) }.is_ok();

        let mut found_tid = None;
        while has_next {
            if entry.th32OwnerProcessID == pid {
                found_tid = Some(entry.th32ThreadID);
                break;
            }

            // SAFETY: Thread32Next called on valid snapshot.
            has_next = unsafe { Thread32Next(snapshot, &mut entry) }.is_ok();
        }

        // SAFETY: Close snapshot handle.
        unsafe {
            let _ = CloseHandle(snapshot); // ignore-ok: the snapshot is released either way
        };

        if found_tid.is_none() {
            // No thread for a PID means the process is gone. Callers use this as a liveness probe
            // (stale lockfile, stale instance lock), so the distinction matters.
            debug!(pid, "No thread found for this PID; the process is gone");
        }

        Ok(found_tid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_current_process() {
        let current_pid = std::process::id();
        let found = ProcessFinder::find_first_thread_id(current_pid).unwrap();
        assert!(found.is_some(), "should find thread for current process");
    }

    #[test]
    fn test_non_existent_process() {
        let found =
            ProcessFinder::find_process_by_name("non_existent_bullet_process_xyz123.exe").unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_get_current_process_path() {
        let current_pid = std::process::id();
        let path = ProcessFinder::get_process_path(current_pid).unwrap();
        assert!(path.is_some(), "should resolve current process binary path");
        let path = path.unwrap();
        assert!(path.exists(), "process path must exist on disk");
    }
}
