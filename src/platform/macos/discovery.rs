//! Find the ChatGPT application bundle in system or user Applications.
use std::env;
use std::path::{Path, PathBuf};

fn app_candidates(home: Option<&Path>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    paths.push(Path::new("/Applications/ChatGPT.app").to_owned());
    if let Some(home) = home {
        paths.push(home.join("Applications/ChatGPT.app"));
    }
    paths
}

fn valid_bundle(path: &Path) -> bool {
    path.is_dir() && path.join("Contents/Info.plist").is_file()
}

pub(super) fn find_app() -> Result<PathBuf, String> {
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
        .ok_or_else(|| {
            "没有找到 ChatGPT.app；请安装到 /Applications，或用 STARTCHATGPT_APP_PATH 指定应用位置"
                .into()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_only_uses_chatgpt_and_supports_user_applications() {
        assert_eq!(
            app_candidates(Some(Path::new("/Users/test"))),
            [
                PathBuf::from("/Applications/ChatGPT.app"),
                PathBuf::from("/Users/test/Applications/ChatGPT.app")
            ]
        );
        assert_eq!(
            app_candidates(None),
            [PathBuf::from("/Applications/ChatGPT.app")]
        );
    }
}
