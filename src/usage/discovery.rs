//! Locate a runnable Codex CLI, including platform-specific install locations.
use std::path::{Path, PathBuf};

pub(super) fn candidates(app: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    // An explicit executable override is useful for portable CLI installations.
    if let Some(path) = std::env::var_os("STARTCHATGPT_CODEX_EXE") {
        paths.push(path.into());
    }
    #[cfg(windows)]
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
        paths.extend(std::env::split_paths(&path).map(|p| p.join(cli_name())));
    }
    #[cfg(windows)]
    if let Some(parent) = app.parent() {
        paths.push(parent.join("resources/codex.exe"));
    }
    #[cfg(target_os = "macos")]
    {
        // Finder-launched apps do not inherit the interactive shell's PATH.
        paths.push("/opt/homebrew/bin/codex".into());
        paths.push("/usr/local/bin/codex".into());
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            paths.push(home.join(".npm-global/bin/codex"));
            paths.push(home.join(".local/bin/codex"));
        }
        if !app.as_os_str().is_empty() {
            paths.push(app.join("Contents/Resources/codex"));
            paths.push(app.join("Contents/Resources/codex-cli/CodexCLI.app/Contents/MacOS/codex"));
        }
    }
    paths
}

const fn cli_name() -> &'static str {
    if cfg!(windows) { "codex.exe" } else { "codex" }
}

#[cfg(target_os = "macos")]
pub(super) fn macos_cli_path() -> std::ffi::OsString {
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let mut paths: Vec<PathBuf> = std::env::split_paths(&inherited).collect();
    // npm's codex wrapper uses /usr/bin/env node. Finder's PATH often omits
    // Homebrew, so finding the wrapper alone does not make it runnable.
    for path in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"] {
        let path = PathBuf::from(path);
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    std::env::join_paths(paths).unwrap_or(inherited)
}
