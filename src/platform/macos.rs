//! macOS launcher backed by the Swift AppKit helper in native/macos.
mod discovery;
mod launcher;
mod monitor;
mod ui;

pub(super) fn run() {
    launcher::main();
}
