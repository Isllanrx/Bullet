fn main() {
    #[cfg(windows)]
    {
        let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let root_ico = manifest_dir
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("assets")
            .join("bullet.ico");
        let local_ico = manifest_dir.join("assets").join("bullet.ico");

        let icon_to_use = if root_ico.exists() {
            root_ico
        } else {
            local_ico
        };

        println!("cargo:rerun-if-changed={}", icon_to_use.display());

        let mut res = winres::WindowsResource::new();
        res.set_icon(&icon_to_use.to_string_lossy());

        let raw_version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
        let display_version = if let Some(stripped) = raw_version.strip_suffix(".0") {
            stripped.to_string()
        } else {
            raw_version.clone()
        };
        res.set("ProductName", "Bullet")
            .set("FileDescription", "Bullet - League of Legends skin changer")
            .set("CompanyName", "Isllan Toso")
            .set(
                "LegalCopyright",
                "Copyright (c) 2026 Isllan Toso. MIT License.",
            )
            .set("InternalName", "bullet")
            .set("OriginalFilename", "bullet.exe")
            .set("Comments", "https://github.com/Isllanrx/Bullet")
            .set("ProductVersion", &display_version)
            .set("FileVersion", &display_version);

        let profile = std::env::var("PROFILE").unwrap_or_default();
        if profile == "release" {
            if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
                println!("cargo:rustc-link-arg-bins=/RELEASE");
            }
            res.set_manifest(
                r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
<trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
        <requestedPrivileges>
            <requestedExecutionLevel level="asInvoker" uiAccess="false" />
        </requestedPrivileges>
    </security>
</trustInfo>
</assembly>
"#,
            );
        }
        if let Err(e) = res.compile() {
            panic!("failed to compile Windows resources (icon, version info, manifest): {e}");
        }
    }
}
