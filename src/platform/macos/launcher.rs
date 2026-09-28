//! Apply launcher options and start the desktop application or quota monitor.
use super::{
    discovery::find_app,
    monitor::monitor,
    ui::{request_ui, setting_from_response},
};
use crate::{config, launch_options::parse_launch_options};
use serde_json::json;
use std::env;

pub(super) fn main() {
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
