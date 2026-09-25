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

        // Shown in Explorer > Properties > Details. winres defaults ProductName to the crate name
        // ("bullet-app"); FileVersion/ProductVersion already come from the workspace version.
        let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
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
            .set("ProductVersion", &version);

        let profile = std::env::var("PROFILE").unwrap_or_default();
        if profile == "release" {
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
        // A silent failure here ships an exe without icon, version info and the asInvoker
        // manifest that keeps it unelevated, so the build must stop.
        if let Err(e) = res.compile() {
            panic!("failed to compile Windows resources (icon, version info, manifest): {e}");
        }
    }
}
