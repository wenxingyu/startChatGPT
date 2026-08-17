#![windows_subsystem = "windows"]

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

mod config;
mod settings;
mod splash;

const PACKAGE_PREFIX: &str = "OpenAI.Codex_";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

struct LaunchOptions {
    show_settings: bool,
    proxy_override: Option<config::ProxySetting>,
    forwarded: Vec<OsString>,
}

fn main() {
    if let Err(message) = run() {
        show_error(&message);
    }
}

fn run() -> Result<(), String> {
    if option_env!("STARTCHATGPT_SPLASH_PREVIEW").is_some() {
        let mut splash = splash::Splash::new().ok_or("无法创建 Loading 预览窗口")?;
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(8) {
            splash.pump();
            thread::sleep(Duration::from_millis(40));
        }
        return Ok(());
    }

    if option_env!("STARTCHATGPT_SETTINGS_PREVIEW").is_some() {
        settings::show(&config::ProxySetting::default())?;
        return Ok(());
    }

    let options = parse_launch_options(env::args_os().skip(1))?;
    let saved_setting = config::load()?;
    let proxy_setting = if options.show_settings || settings::shift_pressed() {
        let Some(setting) = settings::show(&saved_setting)? else {
            return Ok(());
        };
        config::save(&setting)?;
        setting
    } else {
        options.proxy_override.unwrap_or(saved_setting)
    };

    let exe = find_chatgpt()?;
    let working_dir = exe
        .parent()
        .ok_or_else(|| format!("无效的 ChatGPT 路径：{}", exe.display()))?;

    let mut splash = splash::Splash::new();
    let mut command = Command::new(&exe);
    if let Some(argument) = proxy_setting.launch_argument() {
        command.arg(argument);
    }
    let mut child = command
        .args(options.forwarded)
        .current_dir(working_dir)
        .spawn()
        .map_err(|error| format!("启动 {} 失败：{error}", exe.display()))?;

    if splash.is_none() {
        return Ok(());
    }

    let started = Instant::now();
    let timeout = Duration::from_secs(60);
    loop {
        if let Some(window) = splash.as_mut() {
            window.pump();
        }

        if started.elapsed() >= Duration::from_millis(500) && splash::has_visible_window_for(&exe) {
            return Ok(());
        }

        if let Ok(Some(status)) = child.try_wait()
            && !status.success()
        {
            return Err(format!("ChatGPT 启动进程异常退出：{status}"));
        }

        if started.elapsed() >= timeout {
            return Err("等待 ChatGPT 主窗口超时（60 秒）".into());
        }

        thread::sleep(Duration::from_millis(40));
    }
}

fn parse_launch_options(args: impl IntoIterator<Item = OsString>) -> Result<LaunchOptions, String> {
    let mut options = LaunchOptions {
        show_settings: false,
        proxy_override: None,
        forwarded: Vec::new(),
    };
    for argument in args {
        let Some(text) = argument.to_str() else {
            options.forwarded.push(argument);
            continue;
        };
        if text == "--settings" {
            options.show_settings = true;
        } else if text == "--no-proxy" {
            options.proxy_override = Some(config::ProxySetting::Direct);
        } else if let Some(value) = text.strip_prefix("--proxy=") {
            options.proxy_override = Some(config::ProxySetting::proxy(value)?);
        } else {
            options.forwarded.push(argument);
        }
    }
    Ok(options)
}

fn find_chatgpt() -> Result<PathBuf, String> {
    match find_by_scanning_windows_apps() {
        Ok(path) => Ok(path),
        Err(scan_error) => {
            find_by_appx_package().map_err(|appx_error| format!("{scan_error}\n{appx_error}"))
        }
    }
}

fn find_by_scanning_windows_apps() -> Result<PathBuf, String> {
    let program_files = env::var_os("ProgramW6432")
        .or_else(|| env::var_os("ProgramFiles"))
        .unwrap_or_else(|| r"C:\Program Files".into());
    let root = PathBuf::from(program_files).join("WindowsApps");
    let entries =
        fs::read_dir(&root).map_err(|error| format!("无法读取 {}：{error}", root.display()))?;

    let mut newest: Option<(Vec<u64>, PathBuf)> = None;
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }

        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some((version, arch)) = parse_package_directory(name) else {
            continue;
        };
        if arch != package_arch() {
            continue;
        }

        let exe = entry.path().join("app").join("chatgpt.exe");
        if !exe.is_file() {
            continue;
        }

        if newest
            .as_ref()
            .is_none_or(|(current, _)| compare_version(&version, current).is_gt())
        {
            newest = Some((version, exe));
        }
    }

    newest.map(|(_, path)| path).ok_or_else(|| {
        format!(
            "在 {} 中没有找到适用于 {} 的 OpenAI.Codex 安装",
            root.display(),
            package_arch()
        )
    })
}

fn find_by_appx_package() -> Result<PathBuf, String> {
    let script = "(Get-AppxPackage -Name 'OpenAI.Codex' | Sort-Object Version -Descending | Select-Object -First 1 -ExpandProperty InstallLocation)";
    let mut command = Command::new("powershell.exe");
    command
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW);

    let output = command
        .output()
        .map_err(|error| format!("查询 OpenAI.Codex Appx 包失败：{error}"))?;
    if !output.status.success() {
        return Err(format!(
            "查询 OpenAI.Codex Appx 包失败，退出代码：{}",
            output.status
        ));
    }

    let install_dir = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if install_dir.is_empty() {
        return Err("系统没有返回 OpenAI.Codex 的安装位置".into());
    }

    let exe = Path::new(&install_dir).join("app").join("chatgpt.exe");
    exe.is_file()
        .then_some(exe.clone())
        .ok_or_else(|| format!("Appx 安装目录中不存在 {}", exe.display()))
}

fn parse_package_directory(name: &str) -> Option<(Vec<u64>, &str)> {
    let mut parts = name.strip_prefix(PACKAGE_PREFIX)?.split('_');
    let version = parts
        .next()?
        .split('.')
        .map(str::parse)
        .collect::<Result<Vec<u64>, _>>()
        .ok()?;
    let arch = parts.next()?;
    Some((version, arch))
}

fn compare_version(a: &[u64], b: &[u64]) -> std::cmp::Ordering {
    let length = a.len().max(b.len());
    (0..length)
        .map(|index| {
            a.get(index)
                .copied()
                .unwrap_or(0)
                .cmp(&b.get(index).copied().unwrap_or(0))
        })
        .find(|ordering| !ordering.is_eq())
        .unwrap_or(std::cmp::Ordering::Equal)
}

const fn package_arch() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    return "x64";
    #[cfg(target_arch = "x86")]
    return "x86";
    #[cfg(target_arch = "aarch64")]
    return "arm64";
    #[allow(unreachable_code)]
    ""
}

#[cfg(windows)]
fn show_error(message: &str) {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(
            window: *mut c_void,
            text: *const u16,
            caption: *const u16,
            kind: u32,
        ) -> i32;
    }

    let text: Vec<u16> = std::ffi::OsStr::new(message)
        .encode_wide()
        .chain(Some(0))
        .collect();
    let caption: Vec<u16> = "startChatGPT 启动失败\0".encode_utf16().collect();
    unsafe {
        MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), 0x10);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_package_directory() {
        let (version, arch) =
            parse_package_directory("OpenAI.Codex_26.810.7004.0_x64__2p2nqsd0c76g0").unwrap();
        assert_eq!(version, [26, 810, 7004, 0]);
        assert_eq!(arch, "x64");
    }

    #[test]
    fn compares_versions_numerically() {
        assert!(compare_version(&[26, 900, 1, 0], &[26, 810, 7004, 0]).is_gt());
        assert!(compare_version(&[26, 810, 7004], &[26, 810, 7004, 0]).is_eq());
    }

    #[test]
    fn parses_launcher_options_without_forwarding_them() {
        let options = parse_launch_options([
            "--settings".into(),
            "--proxy=http://127.0.0.1:7890".into(),
            "--some-chatgpt-option".into(),
        ])
        .unwrap();
        assert!(options.show_settings);
        assert_eq!(
            options.proxy_override,
            Some(config::ProxySetting::Proxy("http://127.0.0.1:7890".into()))
        );
        assert_eq!(options.forwarded, ["--some-chatgpt-option"]);
    }
}
