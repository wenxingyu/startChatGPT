#![cfg_attr(windows, windows_subsystem = "windows")]

mod config;
mod launch_options;
mod memory;
mod usage;

#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod macos;
#[cfg(windows)]
mod packaged;
#[cfg(windows)]
mod settings;
#[cfg(windows)]
mod splash;
#[cfg(windows)]
mod usage_tray;
#[cfg(windows)]
mod usage_widget;
#[cfg(windows)]
mod windows_launcher;

fn main() {
    #[cfg(windows)]
    windows_launcher::main();
    #[cfg(target_os = "macos")]
    macos::main();
    #[cfg(not(any(windows, target_os = "macos")))]
    compile_error!("startChatGPT supports Windows and macOS only");
}
