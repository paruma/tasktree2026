#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bridge;
#[cfg(windows)]
mod pipe;

use bridge::{Bridge, Envelope, Reply};
use std::sync::Arc;
use tauri::{
    ipc::Channel,
    menu::{CheckMenuItem, Menu},
    Manager,
};

struct WindowMenu {
    menu: Menu<tauri::Wry>,
    always_on_top: CheckMenuItem<tauri::Wry>,
}

#[tauri::command]
fn show_window_menu(
    window: tauri::WebviewWindow,
    state: tauri::State<WindowMenu>,
) -> Result<(), String> {
    state
        .always_on_top
        .set_checked(window.is_always_on_top().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    window.popup_menu(&state.menu).map_err(|e| e.to_string())
}

#[tauri::command]
fn connect_pipe(state: tauri::State<Arc<Bridge>>, channel: Channel<Envelope>) {
    state.connect(channel);
}

#[tauri::command]
fn complete_pipe_request(
    state: tauri::State<Arc<Bridge>>,
    id: u64,
    reply: Reply,
) -> Result<(), String> {
    state.complete(id, reply)
}

fn main() {
    let builder = tauri::Builder::default();
    #[cfg(windows)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _, _| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }));
    builder
        .setup(|app| {
            let always_on_top = CheckMenuItem::with_id(
                app,
                "always-on-top",
                "常に最前面に表示",
                true,
                false,
                None::<&str>,
            )?;
            let menu = Menu::with_items(app, &[&always_on_top])?;
            app.manage(WindowMenu {
                menu,
                always_on_top,
            });
            let bridge = Arc::new(Bridge::default());
            app.manage(bridge.clone());
            #[cfg(windows)]
            pipe::start(bridge)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            show_window_menu,
            connect_pipe,
            complete_pipe_request
        ])
        .on_menu_event(|app, event| {
            if event.id().as_ref() == "always-on-top" {
                if let Some(window) = app.get_webview_window("main") {
                    let result = window
                        .is_always_on_top()
                        .and_then(|enabled| window.set_always_on_top(!enabled));
                    if let Err(error) = result {
                        eprintln!("最前面表示を変更できませんでした: {error}");
                    }
                    if let Ok(enabled) = window.is_always_on_top() {
                        let _ = app.state::<WindowMenu>().always_on_top.set_checked(enabled);
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("Task Tree を起動できませんでした");
}
