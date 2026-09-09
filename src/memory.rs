//! Release idle executable and font/GDI pages from resident memory.
//!
//! Windows will page them back in on demand. This does not discard application
//! state and is especially useful for this mostly-idle notification-area app.
use std::ffi::c_void;
use std::os::windows::io::AsRawHandle;
use std::process::Child;

type Handle = *mut c_void;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> Handle;
    fn SetProcessWorkingSetSize(process: Handle, minimum: usize, maximum: usize) -> i32;
}

fn trim(process: Handle) {
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
