use super::*;

#[test]
fn the_shipped_tools_are_the_native_engine_files_bullet_loads() {
    assert_eq!(SHIPPED_TOOLS, ["bullet_launcher.exe", "bullet_bridge.dll"]);
}
