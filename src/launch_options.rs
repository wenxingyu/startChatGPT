use crate::config;
use std::ffi::OsString;

pub struct LaunchOptions {
    pub show_settings: bool,
    pub proxy_override: Option<config::ProxySetting>,
    pub forwarded: Vec<OsString>,
}

pub fn parse_launch_options(
    args: impl IntoIterator<Item = OsString>,
) -> Result<LaunchOptions, String> {
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

#[cfg(test)]
mod tests {
    use super::*;
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
