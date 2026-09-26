use std::ffi::OsString;
use std::os::windows::{ffi::OsStrExt, process::CommandExt};
use std::path::PathBuf;
use std::process::Command;
use windows::Win32::System::Com::{
    CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::UI::Shell::{
    AO_NONE, ApplicationActivationManager, IApplicationActivationManager,
};
use windows::core::PCWSTR;

pub struct RegisteredApp {
    pub executable: PathBuf,
    pub aumid: String,
}

pub fn find_registered_app() -> Result<RegisteredApp, String> {
    // Query the current user's registration, not possibly stale/staged WindowsApps directories.
    // Explicit UTF-8 avoids Windows PowerShell's OEM encoding for non-ASCII install paths.
    let script = r#"$ErrorActionPreference = 'Stop';
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false);
$p = Get-AppxPackage -Name 'OpenAI.Codex' | Sort-Object Version -Descending | Select-Object -First 1;
if (!$p) { throw 'OpenAI.Codex is not registered for the current user' };
$a = (Get-AppxPackageManifest $p).Package.Applications.Application |
    Where-Object { $_.Executable -match '(^|[/\\])chatgpt\.exe$' } | Select-Object -First 1;
if (!$a) { throw 'ChatGPT application entry was not found in the package manifest' };
@{ executable = (Join-Path $p.InstallLocation $a.Executable); aumid = "$($p.PackageFamilyName)!$($a.Id)" } | ConvertTo-Json -Compress"#;
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(0x0800_0000)
        .output()
        .map_err(|error| format!("查询 ChatGPT 应用包失败：{error}"))?;
    if !output.status.success() {
        return Err(format!(
            "查询 ChatGPT 应用包失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("读取 ChatGPT 应用包信息失败：{error}"))?;
    let executable = value["executable"]
        .as_str()
        .ok_or("应用包缺少可执行文件路径")?;
    let aumid = value["aumid"].as_str().ok_or("应用包缺少应用标识符")?;
    let executable = PathBuf::from(executable);
    if !executable.is_file() {
        return Err(format!("应用包中不存在 {}", executable.display()));
    }
    Ok(RegisteredApp {
        executable,
        aumid: aumid.to_owned(),
    })
}

struct ComApartment;
impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

pub fn activate(aumid: &str, arguments: &[OsString]) -> Result<u32, String> {
    let aumid: Vec<u16> = aumid.encode_utf16().chain(Some(0)).collect();
    let arguments = command_line(arguments)?;
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|error| format!("初始化 Windows 应用激活失败：{error}"))?;
        let _apartment = ComApartment;
        // Out-of-process activation keeps launch arguments alive even if our launcher exits.
        let manager: IApplicationActivationManager =
            CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_LOCAL_SERVER)
                .map_err(|error| format!("创建 Windows 应用激活管理器失败：{error}"))?;
        manager
            .ActivateApplication(PCWSTR(aumid.as_ptr()), PCWSTR(arguments.as_ptr()), AO_NONE)
            .map_err(|error| format!("通过 Windows 应用包启动 ChatGPT 失败：{error}"))
    }
}

fn command_line(arguments: &[OsString]) -> Result<Vec<u16>, String> {
    let mut result = Vec::new();
    for (index, argument) in arguments.iter().enumerate() {
        if index > 0 {
            result.push(b' ' as u16);
        }
        let argument: Vec<u16> = argument.encode_wide().collect();
        if argument.contains(&0) {
            return Err("ChatGPT 参数包含空字符".into());
        }
        // Windows argv escaping: double backslashes before quotes and before the closing quote.
        result.push(b'"' as u16);
        let mut slashes = 0;
        for ch in argument {
            if ch == b'\\' as u16 {
                slashes += 1;
                continue;
            }
            let count = if ch == b'"' as u16 {
                slashes * 2 + 1
            } else {
                slashes
            };
            result.extend(std::iter::repeat_n(b'\\' as u16, count));
            result.push(ch);
            slashes = 0;
        }
        result.extend(std::iter::repeat_n(b'\\' as u16, slashes * 2));
        result.push(b'"' as u16);
    }
    result.push(0);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::UI::Shell::CommandLineToArgvW;

    #[test]
    fn activation_arguments_round_trip_through_windows_parser() {
        let args: Vec<OsString> = [
            "ChatGPT.exe",
            "",
            "--proxy-server=http://127.0.0.1:10808",
            "a b",
            "a\"b",
            "C:\\目录 空格\\",
            "\\\\\"",
            "中文🙂",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        let line = command_line(&args).unwrap();
        unsafe {
            let mut count = 0;
            let argv = CommandLineToArgvW(PCWSTR(line.as_ptr()), &mut count);
            assert!(!argv.is_null());
            assert_eq!(count as usize, args.len());
            for (index, expected) in args.iter().enumerate() {
                let pointer = PCWSTR((*argv.add(index)).0);
                let actual = pointer.as_wide();
                assert_eq!(actual, expected.encode_wide().collect::<Vec<_>>());
            }
            LocalFree(Some(HLOCAL(argv.cast())));
        }
    }

    #[test]
    fn rejects_embedded_null() {
        assert!(command_line(&["bad\0argument".into()]).is_err());
    }

    #[test]
    #[ignore = "requires an installed ChatGPT package and activates its window"]
    fn activates_registered_chatgpt() {
        let app = find_registered_app().unwrap();
        let pid = activate(&app.aumid, &[]).unwrap();
        assert_ne!(pid, 0);
        let started = std::time::Instant::now();
        while !crate::splash::has_visible_window_for(&app.executable) {
            assert!(started.elapsed() < std::time::Duration::from_secs(60));
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        println!("Activated {} (PID {pid})", app.aumid);
    }
}
