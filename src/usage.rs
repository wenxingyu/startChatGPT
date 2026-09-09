//! Read account quota through the local Codex app-server. Never start a model turn.
use crate::config::ProxySetting;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};
use windows_sys::Win32::{Foundation::HWND, UI::WindowsAndMessaging::PostMessageW};

#[derive(Clone, Debug, PartialEq)]
pub struct Window {
    pub remaining: f64,
    pub minutes: u64,
    pub resets_at: Option<u64>,
}

impl Window {
    pub fn label(&self) -> String {
        match self.minutes {
            10080 => "1W".into(),
            n if n % 60 == 0 => format!("{}H", n / 60),
            n => format!("{}m", n),
        }
    }

    pub fn description(&self) -> String {
        match self.minutes {
            10080 => "每周额度".into(),
            n if n % 60 == 0 => format!("{} 小时额度", n / 60),
            n => format!("{} 分钟额度", n),
        }
    }
}

#[derive(Clone, Default)]
pub struct State {
    pub windows: [Option<Window>; 2],
    pub updated: Option<Instant>,
    pub error: Option<String>,
}

impl State {
    pub fn stale(&self) -> bool {
        self.error.is_some()
            || self
                .updated
                .is_none_or(|t| t.elapsed() > Duration::from_secs(120))
    }
}

pub fn parse(value: &Value) -> Result<[Option<Window>; 2], String> {
    // A multi-bucket response is authoritative: never display some other quota as Codex.
    let bucket = if let Some(buckets) = value.get("rateLimitsByLimitId").filter(|v| !v.is_null()) {
        buckets.get("codex")
    } else {
        value.get("rateLimits").filter(|b| {
            b.get("limitId")
                .and_then(Value::as_str)
                .is_none_or(|id| id == "codex")
        })
    }
    .ok_or("账户没有返回 Codex 额度")?;
    let window = |key: &str| -> Option<Window> {
        let v = bucket.get(key)?;
        let used = v.get("usedPercent")?.as_f64()?;
        let minutes = v.get("windowDurationMins")?.as_u64()?;
        if !used.is_finite() || minutes == 0 {
            return None;
        }
        Some(Window {
            remaining: (100.0 - used).clamp(0.0, 100.0),
            minutes,
            resets_at: v.get("resetsAt").and_then(Value::as_u64),
        })
    };
    let windows = [window("primary"), window("secondary")];
    if windows.iter().all(Option::is_none) {
        return Err("暂无可用的额度数据".into());
    }
    Ok(windows)
}

struct Bridge {
    child: Child,
    input: ChildStdin,
    messages: mpsc::Receiver<Value>,
    reader: Option<thread::JoinHandle<()>>,
    next_id: u64,
}

impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

fn candidates(app: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    // An explicit executable override is useful for portable CLI installations.
    if let Some(path) = std::env::var_os("STARTCHATGPT_CODEX_EXE") {
        paths.push(path.into());
    }
    if let Some(root) = std::env::var_os("APPDATA") {
        let package = PathBuf::from(root).join("npm/node_modules/@openai/codex");
        let (arch, triple) = if cfg!(target_arch = "aarch64") {
            ("arm64", "aarch64-pc-windows-msvc")
        } else {
            ("x64", "x86_64-pc-windows-msvc")
        };
        paths.push(package.join(format!(
            "node_modules/@openai/codex-win32-{arch}/vendor/{triple}/bin/codex.exe"
        )));
        paths.push(package.join(format!("vendor/{triple}/codex/codex.exe")));
    }
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path).map(|p| p.join("codex.exe")));
    }
    if let Some(parent) = app.parent() {
        paths.push(parent.join("resources/codex.exe"));
    }
    paths
}

impl Bridge {
    fn connect(app: &Path, proxy: &ProxySetting) -> Result<Self, String> {
        for exe in candidates(app).into_iter().filter(|p| p.is_file()) {
            let mut command = Command::new(exe);
            command
                .arg("app-server")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .creation_flags(0x0800_0000);
            // Do not inherit an accidental proxy when direct mode was selected.
            for key in [
                "HTTP_PROXY",
                "HTTPS_PROXY",
                "ALL_PROXY",
                "http_proxy",
                "https_proxy",
                "all_proxy",
            ] {
                command.env_remove(key);
            }
            if let Some(url) = proxy.proxy_url() {
                command
                    .env("HTTP_PROXY", url)
                    .env("HTTPS_PROXY", url)
                    .env("ALL_PROXY", url);
            }
            let Ok(mut child) = command.spawn() else {
                continue;
            };
            let input = child.stdin.take().expect("piped stdin");
            let output = child.stdout.take().expect("piped stdout");
            let (tx, messages) = mpsc::channel();
            let reader = thread::spawn(move || {
                for line in BufReader::new(output).lines() {
                    let Ok(line) = line else {
                        break;
                    };
                    if let Ok(value) = serde_json::from_str(&line)
                        && tx.send(value).is_err()
                    {
                        break;
                    }
                }
            });
            let mut bridge = Self {
                child,
                input,
                messages,
                reader: Some(reader),
                next_id: 1,
            };
            bridge.request("initialize", Some(json!({"clientInfo": {
                "name": "startchatgpt_usage", "title": "Codex Quota Monitor", "version": env!("CARGO_PKG_VERSION")
            }})))?;
            bridge.send(json!({"method":"initialized", "params":{}}))?;
            return Ok(bridge);
        }
        Err("找不到可运行的 Codex CLI；请安装并登录 Codex CLI".into())
    }

    fn send(&mut self, value: Value) -> Result<(), String> {
        writeln!(self.input, "{value}")
            .and_then(|_| self.input.flush())
            .map_err(|_| "Codex 连接已断开".into())
    }

    fn request(&mut self, method: &str, params: Option<Value>) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let mut request = json!({"id":id,"method":method});
        if let Some(params) = params {
            request["params"] = params;
        }
        self.send(request)?;
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let value = self
                .messages
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .map_err(|_| "额度读取超时或连接中断，将自动重试".to_string())?;
            if value.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if value.get("error").is_some() {
                // Avoid exposing server response contents or account identifiers in UI/logs.
                return Err("额度读取失败，请检查 Codex CLI 登录状态与代理".into());
            }
            let result = value
                .get("result")
                .cloned()
                .ok_or("Codex 返回了无效响应".into());
            // The app-server stays connected between the infrequent quota
            // reads, so its inactive code and data pages need not stay resident.
            crate::memory::child_process(&self.child);
            return result;
        }
    }
}

pub enum Action {
    Refresh,
    Stop,
}

pub fn start(
    app: PathBuf,
    proxy: ProxySetting,
    state: Arc<Mutex<State>>,
    notify: Option<(usize, u32)>,
) -> (mpsc::Sender<Action>, thread::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        let mut bridge = None;
        loop {
            let result = (|| {
                if bridge.is_none() {
                    bridge = Some(Bridge::connect(&app, &proxy)?);
                }
                let result = bridge
                    .as_mut()
                    .unwrap()
                    .request("account/rateLimits/read", None)?;
                parse(&result)
            })();
            {
                let mut state = state.lock().unwrap();
                match result {
                    Ok(windows) => {
                        state.windows = windows;
                        state.updated = Some(Instant::now());
                        state.error = None;
                    }
                    Err(error) => {
                        state.error = Some(error);
                        bridge = None;
                    }
                }
            }
            if let Some((hwnd, message)) = notify {
                unsafe {
                    PostMessageW(hwnd as HWND, message, 0, 0);
                }
            }
            match rx.recv_timeout(Duration::from_secs(45)) {
                Ok(Action::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                _ => {}
            }
            // Collapse repeated refresh clicks and honor shutdown before another request.
            if rx.try_iter().any(|a| matches!(a, Action::Stop)) {
                break;
            }
        }
    });
    (tx, worker)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prefers_codex_bucket_and_keeps_unknown_window_unknown() {
        let v = json!({"rateLimitsByLimitId":{"codex":{"primary":{"usedPercent":37,"windowDurationMins":300}}},
            "rateLimits":{"primary":{"usedPercent":99,"windowDurationMins":300}}});
        let windows = parse(&v).unwrap();
        assert_eq!(windows[0].as_ref().unwrap().remaining, 63.0);
        assert!(windows[1].is_none());
        assert!(parse(&json!({"rateLimitsByLimitId":{"other":{}}})).is_err());
    }
    #[test]
    fn supports_legacy_and_clamps_percentages() {
        let windows = parse(
            &json!({"rateLimits":{"primary":{"usedPercent":105,"windowDurationMins":300},
            "secondary":{"usedPercent":-2,"windowDurationMins":10080}}}),
        )
        .unwrap();
        assert_eq!(windows[0].as_ref().unwrap().remaining, 0.0);
        assert_eq!(windows[1].as_ref().unwrap().remaining, 100.0);
        assert_eq!(windows[1].as_ref().unwrap().label(), "1W");
        assert!(parse(&json!({"rateLimits":null})).is_err());
    }
}
