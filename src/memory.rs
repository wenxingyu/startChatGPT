//! Release idle executable and font/GDI pages from resident memory.
//!
//! Windows will page them back in on demand. This does not discard application
//! state and is especially useful for this mostly-idle notification-area app.
use std::os::windows::io::AsRawHandle;
use std::process::Child;
use windows_sys::Win32::{Foundation::HANDLE, System::Threading::*};

fn trim(process: HANDLE) {
    // SIZE_T(-1) for both limits asks Windows to remove as many currently
    // unused pages as possible. Failure is harmless, so this remains best-effort.
    unsafe {
        SetProcessWorkingSetSize(process, usize::MAX, usize::MAX);
    }
}

pub fn current_process() {
    trim(unsafe { GetCurrentProcess() });
}

pub fn child_process(child: &Child) {
    trim(child.as_raw_handle().cast());
}
