fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_profile",
            "get_app_status",
            "open_settings",
            "open_panel",
            "show_pet",
            "hide_pet",
            "quit_app",
        ]),
    ))
    .expect("Tauri build configuration failed");
}
