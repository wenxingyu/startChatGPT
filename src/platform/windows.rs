//! Windows launcher, native UI, and OS process helpers.
mod launcher;
mod memory;
mod packaged;
mod process;
mod settings;
mod splash;
mod usage_tray;
mod usage_widget;

use crate::{config::ProxySetting, usage};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use windows_sys::Win32::{Foundation::HWND, UI::WindowsAndMessaging::PostMessageW};

pub(super) fn run() {
    launcher::main();
}

pub(super) fn trim_child_process(child: &std::process::Child) {
    memory::child_process(child);
}

// Translate service notifications into native window messages at the UI boundary.
fn start_usage(
    app: PathBuf,
    proxy: ProxySetting,
    state: Arc<Mutex<usage::State>>,
    notify: Option<(usize, u32)>,
) -> (mpsc::Sender<usage::Action>, thread::JoinHandle<()>) {
    let notify = notify.map(|(hwnd, message)| {
        Box::new(move || unsafe {
            PostMessageW(hwnd as HWND, message, 0, 0);
        }) as Box<dyn Fn() + Send>
    });
    usage::start_with_notify(app, proxy, state, notify)
}
