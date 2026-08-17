use std::env;
use std::fs;
use std::path::PathBuf;

pub const DEFAULT_PROXY: &str = "http://127.0.0.1:10808";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProxySetting {
    Proxy(String),
    Direct,
}

impl Default for ProxySetting {
    fn default() -> Self {
        Self::Proxy(DEFAULT_PROXY.into())
    }
}

impl ProxySetting {
    pub fn proxy(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        let value = value.trim();
        if value.is_empty() {
            return Err("代理地址不能为空".into());
        }
        if value.chars().any(char::is_whitespace) {
            return Err("代理地址中不能包含空格".into());
        }
        if !["http://", "https://", "socks4://", "socks5://"]
            .iter()
            .any(|prefix| value.starts_with(prefix))
        {
            return Err("代理地址必须以 http://、https://、socks4:// 或 socks5:// 开头".into());
        }
        Ok(Self::Proxy(value.into()))
    }

    pub fn proxy_url(&self) -> Option<&str> {
        match self {
            Self::Proxy(value) => Some(value),
            Self::Direct => None,
        }
    }

    pub fn launch_argument(&self) -> Option<String> {
        self.proxy_url()
            .map(|value| format!("--proxy-server={value}"))
    }
}

pub fn load() -> Result<ProxySetting, String> {
    let path = config_path()?;
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ProxySetting::default());
        }
        Err(error) => return Err(format!("读取代理设置 {} 失败：{error}", path.display())),
    };
    parse(&content).map_err(|error| format!("代理设置 {} 无效：{error}", path.display()))
}

pub fn save(setting: &ProxySetting) -> Result<(), String> {
    let path = config_path()?;
    let parent = path.parent().ok_or("无法确定代理设置目录")?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("创建代理设置目录 {} 失败：{error}", parent.display()))?;
    let content = match setting {
        ProxySetting::Proxy(value) => format!("proxy={value}\n"),
        ProxySetting::Direct => "direct\n".into(),
    };
    fs::write(&path, content)
        .map_err(|error| format!("保存代理设置 {} 失败：{error}", path.display()))
}

fn config_path() -> Result<PathBuf, String> {
    env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|path| path.join("startChatGPT").join("config.txt"))
        .ok_or_else(|| "系统没有提供 APPDATA 目录".into())
}

fn parse(content: &str) -> Result<ProxySetting, String> {
    let content = content.trim();
    if content.eq_ignore_ascii_case("direct") {
        return Ok(ProxySetting::Direct);
    }
    let value = content.strip_prefix("proxy=").ok_or("无法识别设置内容")?;
    ProxySetting::proxy(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_proxy_and_direct_modes() {
        assert_eq!(
            parse("proxy=http://127.0.0.1:7890\n").unwrap(),
            ProxySetting::Proxy("http://127.0.0.1:7890".into())
        );
        assert_eq!(parse("direct\n").unwrap(), ProxySetting::Direct);
    }

    #[test]
    fn validates_proxy_scheme() {
        assert!(ProxySetting::proxy("socks5://127.0.0.1:10808").is_ok());
        assert!(ProxySetting::proxy("127.0.0.1:10808").is_err());
        assert!(ProxySetting::proxy("http://bad address").is_err());
    }
}
