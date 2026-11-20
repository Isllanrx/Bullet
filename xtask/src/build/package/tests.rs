use super::*;

#[test]
fn the_shipped_tools_are_the_native_engine_files_bullet_loads() {
    assert_eq!(SHIPPED_TOOLS, ["bullet_launcher.exe", "bullet_bridge.dll"]);
}

#[test]
fn resolve_ltk_dir_disables_when_empty_or_none() {
    let root = std::env::temp_dir().join(format!("bullet_ltk_resolve_{}", std::process::id()));
    assert_eq!(resolve_ltk_dir_from(&root, Some("")), Ok(None));
    assert_eq!(resolve_ltk_dir_from(&root, Some("none")), Ok(None));
    assert_eq!(resolve_ltk_dir_from(&root, Some("NONE")), Ok(None));
}

#[test]
fn resolve_ltk_dir_validates_explicit_override_and_private_default() {
    let base = std::env::temp_dir().join(format!("bullet_ltk_detect_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base); // ignore-ok: clean prior fixture
    let workspace = base.join("Bullet");
    let private_ltk = workspace.join(".private").join("bullet_ltk");
    std::fs::create_dir_all(&private_ltk).expect("create private dir");
    std::fs::write(private_ltk.join("Cargo.toml"), "[workspace]\n").expect("write cargo toml");

    assert_eq!(
        resolve_ltk_dir_from(&workspace, None),
        Ok(Some(private_ltk.clone()))
    );
    assert_eq!(
        resolve_ltk_dir_from(&workspace, Some(".private/bullet_ltk")),
        Ok(Some(private_ltk))
    );
    assert!(resolve_ltk_dir_from(&workspace, Some("missing_dir")).is_err());

    let _ = std::fs::remove_dir_all(&base); // ignore-ok: cleanup test fixture
}
