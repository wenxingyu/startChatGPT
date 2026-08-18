fn main() {
    println!("cargo::rerun-if-env-changed=STARTCHATGPT_SPLASH_PREVIEW");
    println!("cargo::rerun-if-env-changed=STARTCHATGPT_SETTINGS_PREVIEW");
    println!("cargo::rerun-if-changed=assets/chatgpt.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut version_resource = winresource::WindowsResource::new();
        version_resource
            .set_icon("assets/chatgpt.ico")
            .set_language(0x0804);
        version_resource
            .compile()
            .expect("failed to compile Windows version information");
    }
}
