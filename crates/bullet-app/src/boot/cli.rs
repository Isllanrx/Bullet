use anyhow::Result;

pub async fn handle_cli_args(args: &[String]) -> Result<Option<i32>> {
    if args.get(1).map(String::as_str) == Some(bullet_app::injector_install::INSTALL_FLAG) {
        return Ok(Some(
            crate::boot::injector::install_injector_elevated(&args[2..]).await,
        ));
    }

    if args
        .iter()
        .any(|a| a == "--use-native-engine" || a == "--native-engine" || a == "--native")
    {
        bullet_platform::preferences::USE_NATIVE_ENGINE.set_enabled(true)?;
        println!("Bullet Native Engine enabled and persisted to native_engine.json");
        return Ok(Some(0));
    }

    if args.iter().any(|a| a == "--use-ltk" || a == "--ltk") {
        bullet_platform::preferences::USE_NATIVE_ENGINE.set_enabled(false)?;
        println!("LTK Provider enabled and persisted to native_engine.json");
        return Ok(Some(0));
    }

    if args.iter().any(|a| a == "--show-provider") {
        let active = bullet_platform::preferences::USE_NATIVE_ENGINE.load();
        let provider = if active {
            "bullet-native"
        } else {
            "ltk-legacy"
        };
        println!("Active provider: {provider}");
        return Ok(Some(0));
    }

    if args
        .iter()
        .any(|a| a == "--native-diagnostics" || a == "--diagnostics")
    {
        let _ = bullet_platform::preferences::USE_NATIVE_ENGINE.load(); // ignore-ok: load preference before diagnostics
        let exe = std::env::current_exe()?;
        let tools = exe
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Executable has no parent directory"))?;
        let output = match args.iter().position(|a| a == "--diagnostics-output") {
            Some(index) => std::path::PathBuf::from(
                args.get(index + 1)
                    .ok_or_else(|| anyhow::anyhow!("--diagnostics-output requires a path"))?,
            ),
            None => std::env::current_dir()?.join("native-integration-diagnostics.json"),
        };
        if let Err(e) = bullet_app::native_integration::run_native_diagnostics(tools, &output).await
        {
            eprintln!("Diagnostic execution failed: {e}");
            return Ok(Some(1));
        }
        return Ok(Some(0));
    }

    Ok(None)
}

#[cfg(windows)]
unsafe extern "system" {
    fn AttachConsole(dw_process_id: u32) -> i32;
}

pub fn attach_parent_console() {
    #[cfg(windows)]
    unsafe {
        let _ = AttachConsole(0xFFFF_FFFF); // ignore-ok: attach console to parent when run from terminal
    }
}
