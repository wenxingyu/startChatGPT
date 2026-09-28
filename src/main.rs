#![cfg_attr(windows, windows_subsystem = "windows")]

mod config;
mod launch_options;
mod platform;
mod usage;

fn main() {
    platform::run();
}
