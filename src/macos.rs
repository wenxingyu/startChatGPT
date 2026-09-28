//! macOS application discovery and the JSON bridge to the native AppKit UI.
use crate::{config, launch_options::parse_launch_options, usage};
use serde_json::{Value, json};
use std::env;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

pub fn main() {
    if let Err(message) = run() {
        eprintln!("{message}");
        let _ = request_ui("error", &json!({"message": message}));
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut usage_only = false;
    let mut widget = false;
    let mut args = Vec::new();
    for arg in env::args_os().skip(1) {
        if arg == "--usage-only" {
            usage_only = true;
        } else if arg == "--usage-widget" {
            usage_only = true;
            widget = true;
        } else {
            args.push(arg);
        }
    }
    let options = parse_launch_options(args)?;
    let saved = config::load()?;
    let proxy = if options.show_settings {
        let response = request_ui("settings", &json!({"proxy": saved.proxy_url()}))?;
        let Some(setting) = setting_from_response(&response)? else {
            return Ok(());
        };
        config::save(&setting)?;
        setting
    } else {
        options.proxy_override.unwrap_or(saved)
    };
    // A standalone quota monitor can use an installed CLI without a desktop app.
    let app = if usage_only {
        find_app().unwrap_or_default()
    } else {
        find_app()?
    };
    if !usage_only {
        let mut arguments = vec![match proxy.launch_argument() {
            Some(argument) => argument,
            None => "--no-proxy-server".into(),
        }];
        for argument in options.forwarded {
            arguments.push(
                argument
                    .into_string()
                    .map_err(|_| "应用参数不是有效的 UTF-8")?,
            );
        }
        request_ui(
            "launch",
            &json!({
                "bundle": app, "arguments": arguments, "proxy": proxy.proxy_url()
            }),
        )?;
    }
    monitor(&app, proxy, !usage_only, widget)
}

fn setting_from_response(value: &Value) -> Result<Option<config::ProxySetting>, String> {
    if value.get("cancelled").and_then(Value::as_bool) == Some(true) {
        return Ok(None);
    }
    match value.get("proxy") {
        Some(Value::Null) => Ok(Some(config::ProxySetting::Direct)),
        Some(Value::String(url)) => config::ProxySetting::proxy(url).map(Some),
        _ => Err("设置窗口返回了无效配置".into()),
    }
}

fn app_candidates(home: Option<&Path>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    // Prefer the Codex bundle when both apps are installed, matching Windows.
    for name in ["Codex.app", "ChatGPT.app"] {
        paths.push(Path::new("/Applications").join(name));
        if let Some(home) = home {
            paths.push(home.join("Applications").join(name));
        }
    }
    paths
}

fn valid_bundle(path: &Path) -> bool {
    path.is_dir() && path.join("Contents/Info.plist").is_file()
}

fn find_app() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("STARTCHATGPT_APP_PATH") {
        let path = PathBuf::from(path);
        if valid_bundle(&path) {
            return Ok(path);
        }
        return Err(format!(
            "STARTCHATGPT_APP_PATH 不是有效的 .app 应用：{}",
            path.display()
        ));
    }
    let home = env::var_os("HOME").map(PathBuf::from);
    app_candidates(home.as_deref())
        .into_iter()
        .find(|path| valid_bundle(path))
        .ok_or_else(|| "没有找到 Codex.app 或 ChatGPT.app；请安装到 /Applications，或用 STARTCHATGPT_APP_PATH 指定应用位置".into())
}

fn ui_path() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("STARTCHATGPT_MACOS_UI") {
        let path = PathBuf::from(path);
        return path
            .is_file()
            .then_some(path)
            .ok_or_else(|| "STARTCHATGPT_MACOS_UI 指定的文件不存在".into());
    }
    let exe = env::current_exe().map_err(|error| error.to_string())?;
    let parent = exe.parent().ok_or("无法确定启动器目录")?;
    for path in [
        parent.join("../Resources/startChatGPT-ui"),
        parent.join("startChatGPT-ui"),
    ] {
        if path.is_file() {
            return Ok(path);
        }
    }
    Err("找不到 macOS 界面组件；请使用 build-macos.sh 构建的完整 .app".into())
}

fn request_ui(mode: &str, value: &Value) -> Result<Value, String> {
    let mut child = Command::new(ui_path()?)
        .arg(mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("无法打开 macOS 界面：{error}"))?;
    let write_result = writeln!(child.stdin.take().ok_or("界面输入不可用")?, "{value}");
    if let Err(error) = write_result {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!("向界面发送请求失败：{error}"));
    }
    let output = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    let response: Value =
        serde_json::from_slice(&output.stdout).map_err(|_| "macOS 界面返回了无效响应")?;
    if let Some(error) = response.get("error").and_then(Value::as_str) {
        return Err(error.into());
    }
    if !output.status.success() {
        return Err("macOS 界面异常退出".into());
    }
    Ok(response)
}

enum Event {
    Updated,
    Refresh,
    Closed,
}

fn snapshot(state: &usage::State) -> Value {
    json!({
        "stale": state.stale(), "error": state.error,
        "windows": state.windows.iter().map(|window| window.as_ref().map(|w| json!({
            "label": w.label(), "description": w.description(),
            "remaining": w.remaining, "resetsAt": w.resets_at
        }))).collect::<Vec<_>>()
    })
}

fn monitor(
    app: &Path,
    proxy: config::ProxySetting,
    watch_app: bool,
    widget: bool,
) -> Result<(), String> {
    let mut ui = Command::new(ui_path()?)
        .arg("monitor")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("无法创建菜单栏额度显示：{error}"))?;
    let mut input = ui.stdin.take().ok_or("界面输入不可用")?;
    if let Err(error) = writeln!(
        input,
        "{}",
        json!({"bundle": app, "watchApp": watch_app, "widget": widget})
    ) {
        let _ = ui.kill();
        let _ = ui.wait();
        return Err(error.to_string());
    }
    let output = ui.stdout.take().ok_or("界面输出不可用")?;
    let mut output = BufReader::new(output);
    let mut ready = String::new();
    let handshake = output
        .read_line(&mut ready)
        .map_err(|error| error.to_string())
        .and_then(|_| {
            serde_json::from_str::<Value>(&ready).map_err(|_| "菜单栏界面未完成初始化".into())
        });
    let handshake = match handshake {
        Ok(value) => value,
        Err(error) => {
            let _ = ui.kill();
            let _ = ui.wait();
            return Err(error);
        }
    };
    if handshake["alreadyRunning"] == true {
        drop(input);
        let _ = ui.wait();
        return Ok(());
    }
    if handshake["ready"] != true {
        let _ = ui.kill();
        let _ = ui.wait();
        return Err(handshake["error"]
            .as_str()
            .unwrap_or("菜单栏界面初始化失败")
            .into());
    }
    let (tx, rx) = mpsc::channel();
    let reader_tx = tx.clone();
    let reader = thread::spawn(move || {
        for line in output.lines() {
            let Ok(line) = line else { break };
            if let Ok(value) = serde_json::from_str::<Value>(&line)
                && value["action"] == "refresh"
            {
                let _ = reader_tx.send(Event::Refresh);
            }
        }
        let _ = reader_tx.send(Event::Closed);
    });
    let state = Arc::new(Mutex::new(usage::State::default()));
    let notify_tx = tx.clone();
    let (actions, worker) = usage::start_with_notify(
        app.to_owned(),
        proxy,
        state.clone(),
        Some(Box::new(move || {
            let _ = notify_tx.send(Event::Updated);
        })),
    );
    let _ = tx.send(Event::Updated);
    let mut result = Ok(());
    while let Ok(event) = rx.recv() {
        match event {
            Event::Refresh => {
                let _ = actions.send(usage::Action::Refresh);
            }
            Event::Closed => break,
            Event::Updated => {
                let value = snapshot(&state.lock().unwrap());
                if let Err(error) = writeln!(input, "{value}").and_then(|_| input.flush()) {
                    result = Err(format!("菜单栏界面连接已断开：{error}"));
                    break;
                }
            }
        }
    }
    let _ = actions.send(usage::Action::Stop);
    drop(input);
    if result.is_err() {
        let _ = ui.kill();
    }
    let status = ui.wait().map_err(|error| error.to_string());
    let _ = reader.join();
    let _ = worker.join();
    result?;
    if !status?.success() {
        return Err("菜单栏界面异常退出".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_distinguish_cancel_direct_proxy_and_malformed_response() {
        assert_eq!(
            setting_from_response(&json!({"cancelled": true})).unwrap(),
            None
        );
        assert_eq!(
            setting_from_response(&json!({"proxy": null})).unwrap(),
            Some(config::ProxySetting::Direct)
        );
        assert_eq!(
            setting_from_response(&json!({"proxy": "http://127.0.0.1:7890"})).unwrap(),
            Some(config::ProxySetting::Proxy("http://127.0.0.1:7890".into()))
        );
        assert!(setting_from_response(&json!({})).is_err());
        assert!(setting_from_response(&json!({"proxy": "invalid"})).is_err());
    }

    #[test]
    fn discovery_prefers_codex_and_supports_user_applications() {
        assert_eq!(
            app_candidates(Some(Path::new("/Users/test"))),
            [
                PathBuf::from("/Applications/Codex.app"),
                PathBuf::from("/Users/test/Applications/Codex.app"),
                PathBuf::from("/Applications/ChatGPT.app"),
                PathBuf::from("/Users/test/Applications/ChatGPT.app")
            ]
        );
    }

    #[test]
    fn stale_snapshot_preserves_last_known_quota_and_unknown_window() {
        let state = usage::State {
            windows: [
                Some(usage::Window {
                    remaining: 63.0,
                    minutes: 300,
                    resets_at: Some(123),
                }),
                None,
            ],
            updated: Some(std::time::Instant::now()),
            error: Some("连接中断".into()),
        };
        let value = snapshot(&state);
        assert_eq!(value["stale"], true);
        assert_eq!(value["windows"][0]["remaining"], 63.0);
        assert_eq!(value["windows"][0]["resetsAt"], 123);
        assert!(value["windows"][1].is_null());
    }
}
