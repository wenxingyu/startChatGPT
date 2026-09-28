//! Own the local app-server process and exchange JSON requests over its pipes.
use super::discovery::candidates;
#[cfg(target_os = "macos")]
use super::discovery::macos_cli_path;
use crate::config::ProxySetting;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

pub(super) struct Bridge {
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
        // A descendant can keep stdout open after the app-server exits. Never
        // wait indefinitely for that pipe: dropping the handle detaches it.
        if let Some(reader) = self.reader.take().filter(|reader| reader.is_finished()) {
            let _ = reader.join();
        }
    }
}

impl Bridge {
    pub(super) fn connect(app: &Path, proxy: &ProxySetting) -> Result<Self, String> {
        for exe in candidates(app).into_iter().filter(|p| p.is_file()) {
            let mut command = Command::new(exe);
            command
                .arg("app-server")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null());
            #[cfg(windows)]
            command.creation_flags(0x0800_0000);
            #[cfg(target_os = "macos")]
            command.env("PATH", macos_cli_path());
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
            #[cfg(target_os = "macos")]
            for key in ["NO_PROXY", "no_proxy"] {
                command.env_remove(key);
            }
            if let Some(url) = proxy.proxy_url() {
                command
                    .env("HTTP_PROXY", url)
                    .env("HTTPS_PROXY", url)
                    .env("ALL_PROXY", url);
            }
            let Ok(child) = command.spawn() else {
                continue;
            };
            let mut bridge = Self::from_child(child);
            // A stale or incompatible bundled CLI must not prevent trying the
            // next candidate. Drop the failed bridge before reconnecting.
            if bridge.request("initialize", Some(json!({"clientInfo": {
                "name": "startchatgpt_usage", "title": "Codex Quota Monitor", "version": env!("CARGO_PKG_VERSION")
            }}))).is_err() || bridge.send(json!({"method":"initialized", "params":{}})).is_err() {
                continue;
            }
            return Ok(bridge);
        }
        Err("找不到可运行的 Codex CLI；请安装并登录 Codex CLI".into())
    }

    pub(super) fn from_child(mut child: Child) -> Self {
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
        Self {
            child,
            input,
            messages,
            reader: Some(reader),
            next_id: 1,
        }
    }

    fn send(&mut self, value: Value) -> Result<(), String> {
        writeln!(self.input, "{value}")
            .and_then(|_| self.input.flush())
            .map_err(|_| "Codex 连接已断开".into())
    }

    pub(super) fn request(&mut self, method: &str, params: Option<Value>) -> Result<Value, String> {
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
            crate::platform::trim_child_process(&self.child);
            return result;
        }
    }
}
