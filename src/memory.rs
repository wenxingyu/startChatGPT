//! Release idle executable and font/GDI pages from resident memory.
//!
//! Windows will page them back in on demand. This does not discard application
//! state and is especially useful for this mostly-idle notification-area app.
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
use std::process::Child;
#[cfg(windows)]
use windows_sys::Win32::{Foundation::HANDLE, System::Threading::*};

#[cfg(windows)]
fn trim(process: HANDLE) {
    // SIZE_T(-1) for both limits asks Windows to remove as many currently
    // unused pages as possible. Failure is harmless, so this remains best-effort.
    unsafe {
        SetProcessWorkingSetSize(process, usize::MAX, usize::MAX);
    }
}

#[cfg(windows)]
pub fn current_process() {
    trim(unsafe { GetCurrentProcess() });
}

pub fn child_process(_child: &Child) {
    #[cfg(windows)]
    trim(_child.as_raw_handle().cast());
}
