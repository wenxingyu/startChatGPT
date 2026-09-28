//! Relay quota updates and native UI events for the menu bar and widget.
use super::ui::ui_path;
use crate::{config, usage};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

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

pub(super) fn monitor(
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
