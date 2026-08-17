use std::path::PathBuf;

fn main() {
    println!("cargo::rerun-if-env-changed=STARTCHATGPT_SPLASH_PREVIEW");
    let resource = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("chatgpt_icon_windows_amd64.syso");

    println!("cargo::rerun-if-changed={}", resource.display());
    println!(
        "cargo::rustc-link-arg-bin=startChatGPT={}",
        resource.display()
    );
}
