mod diagnostics;
mod input;
mod logger;
mod session;
mod state;
mod tray;

use remote_buddy_core::config::{MappingConfig, default_config_path, load_config};
use state::{AppSnapshot, AppState, SettingsUpdate, VoiceSettingsUpdate};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use windows::Win32::Foundation::RECT;
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, IsZoomed, SW_MAXIMIZE, SW_RESTORE, ShowWindow,
};

static LAYOUT_REPAIR_GENERATION: AtomicU64 = AtomicU64::new(0);
static STARTUP_ELEVATION_ERROR: OnceLock<String> = OnceLock::new();

pub fn maybe_self_elevate() {
    if remote_buddy_windows::elevation::is_elevated()
        || remote_buddy_windows::elevation::is_elevated_invocation()
        || remote_buddy_windows::elevation::elevation_disabled_by_argument()
    {
        return;
    }
    let run_as_admin = load_config(&default_config_path())
        .map(|config| config.run_as_admin)
        .unwrap_or(true);
    if !run_as_admin {
        return;
    }
    match remote_buddy_windows::elevation::relaunch_self_elevated() {
        Ok(()) => std::process::exit(0),
        Err(error) => {
            let _ = STARTUP_ELEVATION_ERROR.set(error.to_string());
        }
    }
}

#[tauri::command]
async fn get_app_state(state: State<'_, AppState>) -> Result<AppSnapshot, String> {
    Ok(state.snapshot().await)
}

#[tauri::command]
async fn discover_devices(state: State<'_, AppState>) -> Result<AppSnapshot, String> {
    state.discover_devices().await
}

#[tauri::command]
async fn connect_device(
    device_id: String,
    state: State<'_, AppState>,
) -> Result<AppSnapshot, String> {
    state.connect(&device_id).await
}

#[tauri::command]
async fn disconnect_device(state: State<'_, AppState>) -> Result<AppSnapshot, String> {
    state.disconnect().await
}

#[tauri::command]
async fn retry_device(state: State<'_, AppState>) -> Result<AppSnapshot, String> {
    state.retry().await
}

#[tauri::command]
async fn select_device(
    device_id: String,
    state: State<'_, AppState>,
) -> Result<AppSnapshot, String> {
    state.select_device(&device_id).await
}

#[tauri::command]
async fn save_mapping(
    device_id: String,
    mapping: MappingConfig,
    state: State<'_, AppState>,
) -> Result<AppSnapshot, String> {
    state.save_mapping(&device_id, mapping).await
}

#[tauri::command]
async fn restore_default_mappings(
    device_id: String,
    state: State<'_, AppState>,
) -> Result<AppSnapshot, String> {
    state.reset_mappings(&device_id).await
}

#[tauri::command]
async fn save_settings(
    update: SettingsUpdate,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppSnapshot, String> {
    let launch_at_startup = update.launch_at_startup;
    let snapshot = state.save_settings(update).await?;
    if let Err(error) = sync_autostart(&app, launch_at_startup) {
        state
            .logger
            .warn(format!("设置已保存，但开机自启动更新失败：{error}"));
    }
    Ok(snapshot)
}

fn sync_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    if manager.is_enabled().unwrap_or(!enabled) == enabled {
        return Ok(());
    }
    if enabled {
        manager.enable()
    } else {
        manager.disable()
    }
    .map_err(|error| error.to_string())
}

#[tauri::command]
async fn save_voice_settings(
    device_id: String,
    update: VoiceSettingsUpdate,
    state: State<'_, AppState>,
) -> Result<AppSnapshot, String> {
    state.save_voice_settings(&device_id, update).await
}

#[tauri::command]
async fn test_audio_output(state: State<'_, AppState>) -> Result<String, String> {
    state.test_audio_output().await
}

#[tauri::command]
async fn export_diagnostics(state: State<'_, AppState>) -> Result<String, String> {
    state.export_diagnostics().await
}

#[tauri::command]
async fn list_running_applications()
-> Result<Vec<remote_buddy_windows::apps::RunningApplication>, String> {
    tauri::async_runtime::spawn_blocking(remote_buddy_windows::apps::list_running_applications)
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn open_external_url(url: String) -> Result<(), String> {
    const ALLOWED_URLS: &[&str] = &[
        "https://github.com/jimmieyangcsjs/remote-buddy-rust",
        "https://github.com/jimmieyangcsjs/remote-buddy-rust/issues",
        "https://vb-audio.com/Cable/",
    ];
    if !ALLOWED_URLS.contains(&url.as_str()) {
        return Err("该链接不在允许列表中".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || {
        remote_buddy_windows::external::open_https_url(&url)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
fn enable_button_capture() -> Result<(), String> {
    let executable =
        std::env::current_exe().map_err(|error| format!("无法定位当前程序：{error}"))?;
    remote_buddy_windows::capture_profile::request_elevated_capture_setup(&executable)
}

#[tauri::command]
fn disable_button_capture() -> Result<(), String> {
    let executable =
        std::env::current_exe().map_err(|error| format!("无法定位当前程序：{error}"))?;
    remote_buddy_windows::capture_profile::request_elevated_capture_restore(&executable)
}

#[tauri::command]
fn restart_as_administrator(app: AppHandle) -> Result<(), String> {
    if remote_buddy_windows::elevation::is_elevated() {
        return Ok(());
    }
    remote_buddy_windows::elevation::relaunch_self_elevated().map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(350)).await;
        app.exit(0);
    });
    Ok(())
}

#[tauri::command]
fn show_main_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        schedule_webview_layout_repair(window);
    }
}

#[tauri::command]
fn toggle_main_window_maximize(app: AppHandle) -> Result<bool, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "主窗口不可用".to_string())?;
    let hwnd = window
        .hwnd()
        .map_err(|error| format!("无法获取主窗口句柄：{error}"))?;
    let maximized = unsafe { IsZoomed(hwnd).as_bool() };
    unsafe {
        let _ = ShowWindow(hwnd, if maximized { SW_RESTORE } else { SW_MAXIMIZE });
    }
    schedule_webview_layout_repair(window);
    Ok(!maximized)
}

fn schedule_webview_layout_repair(window: tauri::WebviewWindow<tauri::Wry>) {
    let generation = LAYOUT_REPAIR_GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    tauri::async_runtime::spawn(async move {
        for delay in [60_u64, 160, 420, 900] {
            tokio::time::sleep(Duration::from_millis(delay)).await;
            if LAYOUT_REPAIR_GENERATION.load(Ordering::Relaxed) != generation {
                return;
            }
            repair_webview_layout(&window);
        }
    });
}

fn repair_webview_layout(window: &tauri::WebviewWindow<tauri::Wry>) {
    if window.is_minimized().unwrap_or(false) {
        return;
    }
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    let mut client = RECT::default();
    if unsafe { GetClientRect(hwnd, &mut client) }.is_err() {
        return;
    }
    let width = (client.right - client.left).max(0) as u32;
    let height = (client.bottom - client.top).max(0) as u32;
    if width == 0 || height == 0 {
        return;
    }

    let webview: &tauri::Webview<tauri::Wry> = window.as_ref();
    let _ = webview.set_bounds(tauri::Rect {
        position: tauri::PhysicalPosition::new(0, 0).into(),
        size: tauri::PhysicalSize::new(width, height).into(),
    });
}

fn shutdown_system_resources(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>()
        && let Ok(mut listener) = state.etw_listener.lock()
    {
        listener.take();
    }
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .arg("--autostart")
                .build(),
        )
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app.clone());
        }))
        .setup(|app| {
            let state = AppState::new(app.handle().clone());
            if let Some(error) = STARTUP_ELEVATION_ERROR.get() {
                state.logger.warn(format!(
                    "未以管理员权限启动：{error}。高权限应用切换可能受限，可在设置中重新提权。"
                ));
            }
            app.manage(state);
            tray::build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::Resized(_)
            | tauri::WindowEvent::Moved(_)
            | tauri::WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(webview_window) = window.app_handle().get_webview_window(window.label())
                {
                    schedule_webview_layout_repair(webview_window);
                }
            }
            tauri::WindowEvent::CloseRequested { api, .. } => {
                let state = window.state::<AppState>();
                if state.minimize_to_tray.load(Ordering::Relaxed) {
                    api.prevent_close();
                    let _ = window.hide();
                    let _ = window.app_handle().emit("window-hidden-to-tray", ());
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_app_state,
            discover_devices,
            connect_device,
            disconnect_device,
            retry_device,
            select_device,
            save_mapping,
            restore_default_mappings,
            save_settings,
            save_voice_settings,
            test_audio_output,
            export_diagnostics,
            list_running_applications,
            open_external_url,
            enable_button_capture,
            disable_button_capture,
            restart_as_administrator,
            show_main_window,
            toggle_main_window_maximize,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Remote Buddy");

    app.run(|app, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            shutdown_system_resources(app);
        }
    });
}
