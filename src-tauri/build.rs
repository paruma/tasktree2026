fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "show_window_menu",
            "connect_pipe",
            "complete_pipe_request",
        ]),
    ))
    .expect("Tauri のビルド設定を生成できませんでした");
}
