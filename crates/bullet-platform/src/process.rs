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
    pub fn get_process_path(pid: u32) -> Result<Option<PathBuf>, PlatformError> {
        let handle = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
            Ok(h) => h,
            Err(_) => return Ok(None),
        };

        let mut buffer = [0u16; 1024];
        let mut size = buffer.len() as u32;

        let res = unsafe {
            QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_FORMAT(0),
                PWSTR(buffer.as_mut_ptr()),
                &mut size,
            )
        };

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

    pub fn find_process_path(exe_name: &str) -> Result<Option<PathBuf>, PlatformError> {
        if let Some(pid) = Self::find_process_by_name(exe_name)? {
            Self::get_process_path(pid)
        } else {
            Ok(None)
        }
    }
    pub fn find_process_by_name(exe_name: &str) -> Result<Option<u32>, PlatformError> {
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }?;

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        let mut has_next = unsafe { Process32FirstW(snapshot, &mut entry) }.is_ok();

        let mut found_pid = None;
        while has_next {
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

            has_next = unsafe { Process32NextW(snapshot, &mut entry) }.is_ok();
        }

        unsafe {
            let _ = CloseHandle(snapshot); // ignore-ok: the snapshot is released either way
        };

        match found_pid {
            Some(pid) => debug!(exe = exe_name, pid, "Process found"),
            None => debug!(exe = exe_name, "Process not running"),
        }

        Ok(found_pid)
    }

    pub fn find_first_thread_id(pid: u32) -> Result<Option<u32>, PlatformError> {
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }?;

        let mut entry = THREADENTRY32 {
            dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };

        let mut has_next = unsafe { Thread32First(snapshot, &mut entry) }.is_ok();

        let mut found_tid = None;
        while has_next {
            if entry.th32OwnerProcessID == pid {
                found_tid = Some(entry.th32ThreadID);
                break;
            }

            has_next = unsafe { Thread32Next(snapshot, &mut entry) }.is_ok();
        }

        unsafe {
            let _ = CloseHandle(snapshot); // ignore-ok: the snapshot is released either way
        };

        if found_tid.is_none() {
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
