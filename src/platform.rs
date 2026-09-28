//! Select the native launcher and expose the small shared platform interface.
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod macos;
#[cfg(windows)]
mod windows;

pub(crate) fn run() {
    #[cfg(windows)]
    windows::run();
    #[cfg(target_os = "macos")]
    macos::run();
    #[cfg(not(any(windows, target_os = "macos")))]
    compile_error!("startChatGPT supports Windows and macOS only");
}

pub(crate) fn trim_child_process(_child: &std::process::Child) {
    #[cfg(windows)]
    windows::trim_child_process(_child);
}
