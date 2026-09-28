//! Find application windows and wait for desktop process exit.
use std::mem::size_of;
use std::path::Path;
use windows_sys::Win32::{
    Foundation::*,
    System::{ProcessStatus::*, Threading::*},
    UI::WindowsAndMessaging::*,
};

pub(super) fn has_visible_window_for(executable: &Path) -> bool {
    let target = executable.to_string_lossy().replace('/', "\\");
    let mut search = WindowSearch {
        target: target.to_lowercase(),
        found: false,
    };
    unsafe {
        EnumWindows(
            Some(enum_window),
            (&mut search as *mut WindowSearch) as LPARAM,
        );
    }
    search.found
}

/// An owned handle that can sleep until a matching process exits.
pub(super) struct ProcessWait(HANDLE);

// Windows process handles may be waited on from any thread.
unsafe impl Send for ProcessWait {}

impl ProcessWait {
    pub(super) fn wait(self) {
        unsafe {
            WaitForSingleObject(self.0, INFINITE);
        }
    }
}

impl Drop for ProcessWait {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

/// Finds a process with this exact executable path and returns a waitable handle.
pub(super) fn process_wait_for(executable: &Path) -> Option<ProcessWait> {
    let target = executable
        .to_string_lossy()
        .replace('/', "\\")
        .to_lowercase();
    let mut process_ids = vec![0u32; 1024];
    loop {
        let mut bytes_returned = 0;
        let capacity_bytes = (process_ids.len() * size_of::<u32>()) as u32;
        let success = unsafe {
            EnumProcesses(
                process_ids.as_mut_ptr(),
                capacity_bytes,
                &mut bytes_returned,
            )
        };
        if success == 0 {
            return None;
        }
        if bytes_returned < capacity_bytes {
            process_ids.truncate(bytes_returned as usize / size_of::<u32>());
            break;
        }
        process_ids.resize(process_ids.len() * 2, 0);
    }

    let mut path = vec![0u16; 32_768];
    for process_id in process_ids {
        if process_id == 0 {
            continue;
        }
        let process = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                0,
                process_id,
            )
        };
        if process.is_null() {
            continue;
        }
        let mut length = path.len() as u32;
        let success =
            unsafe { QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut length) };
        if success != 0
            && String::from_utf16_lossy(&path[..length as usize]).to_lowercase() == target
        {
            return Some(ProcessWait(process));
        }
        unsafe { CloseHandle(process) };
    }
    None
}

struct WindowSearch {
    target: String,
    found: bool,
}

unsafe extern "system" fn enum_window(hwnd: HWND, data: LPARAM) -> i32 {
    unsafe {
        let search = &mut *(data as *mut WindowSearch);
        if IsWindowVisible(hwnd) == 0 || !GetWindow(hwnd, GW_OWNER).is_null() {
            return 1;
        }

        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if GetWindowRect(hwnd, &mut rect) == 0
            || rect.right - rect.left < 240
            || rect.bottom - rect.top < 160
        {
            return 1;
        }

        let mut process_id = 0;
        GetWindowThreadProcessId(hwnd, &mut process_id);
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id);
        if process.is_null() {
            return 1;
        }

        let mut path = vec![0u16; 32_768];
        let mut length = path.len() as u32;
        let success = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut length);
        CloseHandle(process);
        if success != 0 {
            let actual = String::from_utf16_lossy(&path[..length as usize]).to_lowercase();
            if actual == search.target {
                search.found = true;
                return 0;
            }
        }
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_current_process_by_exact_executable_path() {
        assert!(process_wait_for(&std::env::current_exe().unwrap()).is_some());
    }
}
