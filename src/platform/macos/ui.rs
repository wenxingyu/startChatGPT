//! Locate the AppKit helper and exchange one-shot JSON requests with it.
use crate::config;
use serde_json::Value;
use std::env;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub(super) fn setting_from_response(value: &Value) -> Result<Option<config::ProxySetting>, String> {
    if value.get("cancelled").and_then(Value::as_bool) == Some(true) {
        return Ok(None);
    }
    match value.get("proxy") {
        Some(Value::Null) => Ok(Some(config::ProxySetting::Direct)),
        Some(Value::String(url)) => config::ProxySetting::proxy(url).map(Some),
        _ => Err("设置窗口返回了无效配置".into()),
    }
}

pub(super) fn ui_path() -> Result<PathBuf, String> {
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

pub(super) fn request_ui(mode: &str, value: &Value) -> Result<Value, String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
}
