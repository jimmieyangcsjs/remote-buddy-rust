use crate::state::AppState;
use remote_buddy_core::device::DeviceState;
use std::path::Path;
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

const WAITING_FOR_INPUT: &str = "最近按键：等待遥控器输入";

pub struct TrayState {
    tray: TrayIcon,
    connection: MenuItem<tauri::Wry>,
    last_action: MenuItem<tauri::Wry>,
    reconnect: MenuItem<tauri::Wry>,
    disconnect: MenuItem<tauri::Wry>,
    snapshot: Mutex<TraySnapshot>,
}

#[derive(Default)]
struct TraySnapshot {
    connection: Option<DeviceState>,
    last_action: Option<String>,
}

impl TrayState {
    pub fn update_connection(&self, state: DeviceState) {
        let label = connection_label(state);
        let _ = self.connection.set_text(format!("状态：{label}"));
        let _ = self
            .reconnect
            .set_enabled(!matches!(state, DeviceState::Connecting));
        let _ = self.disconnect.set_enabled(matches!(
            state,
            DeviceState::Ready | DeviceState::Degraded | DeviceState::Connecting
        ));

        if let Ok(mut snapshot) = self.snapshot.lock() {
            snapshot.connection = Some(state);
        }
        self.refresh_tooltip();
    }

    pub fn record_action(&self, button: &str, gesture: &str, action: &str) {
        let text = format!(
            "{} · {} → {}",
            button_label(button),
            gesture,
            truncate(action, 72)
        );
        let _ = self.last_action.set_text(format!("最近按键：{text}"));
        if let Ok(mut snapshot) = self.snapshot.lock() {
            snapshot.last_action = Some(text);
        }
        self.refresh_tooltip();
    }

    pub fn show_notice(&self, text: &str) {
        let text = truncate(text, 96);
        let _ = self.last_action.set_text(&text);
        if let Ok(mut snapshot) = self.snapshot.lock() {
            snapshot.last_action = Some(text.clone());
        }
        self.refresh_tooltip();
    }

    pub fn reset_last_action(&self) {
        let _ = self.last_action.set_text(WAITING_FOR_INPUT);
        if let Ok(mut snapshot) = self.snapshot.lock() {
            snapshot.last_action = None;
        }
        self.refresh_tooltip();
    }

    fn refresh_tooltip(&self) {
        let snapshot = self.snapshot.lock().ok();
        let connection = snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.connection)
            .map(connection_label)
            .unwrap_or("未连接");
        let last_action = snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.last_action.as_deref())
            .unwrap_or("等待输入");
        let _ = self
            .tray
            .set_tooltip(Some(format!("Remote Buddy · {connection} · {last_action}")));
    }
}

pub fn build(app: &tauri::App) -> tauri::Result<()> {
    let connection = MenuItem::with_id(app, "status", "状态：未连接", false, None::<&str>)?;
    let last_action =
        MenuItem::with_id(app, "last-action", WAITING_FOR_INPUT, false, None::<&str>)?;
    let clear_action = MenuItem::with_id(app, "clear-action", "清除最近按键", true, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let voice = MenuItem::with_id(app, "voice", "打开语音页", true, None::<&str>)?;
    let mapping = MenuItem::with_id(app, "mapping", "打开按键映射", true, None::<&str>)?;
    let diagnostics = MenuItem::with_id(app, "diagnostics", "打开诊断页", true, None::<&str>)?;
    let export = MenuItem::with_id(app, "export", "导出诊断信息", true, None::<&str>)?;
    let enhance = MenuItem::with_id(app, "enhance", "启用/刷新按键增强", true, None::<&str>)?;
    let reconnect = MenuItem::with_id(app, "reconnect", "重新连接遥控器", true, None::<&str>)?;
    let disconnect = MenuItem::with_id(app, "disconnect", "断开连接", false, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出 Remote Buddy", true, None::<&str>)?;
    let separator_1 = PredefinedMenuItem::separator(app)?;
    let separator_2 = PredefinedMenuItem::separator(app)?;
    let separator_3 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &connection,
            &last_action,
            &clear_action,
            &separator_1,
            &show,
            &voice,
            &mapping,
            &diagnostics,
            &separator_2,
            &reconnect,
            &disconnect,
            &export,
            &enhance,
            &separator_3,
            &quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id("remote-buddy-tray")
        .tooltip("Remote Buddy · 等待连接")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => crate::show_main_window(app.clone()),
            "clear-action" => reset_last_action(app),
            "voice" => show_page(app, "voice"),
            "mapping" => show_page(app, "mapping"),
            "diagnostics" => show_page(app, "diagnostics"),
            "reconnect" => request_reconnect(app),
            "disconnect" => request_disconnect(app),
            "export" => export_diagnostics(app),
            "enhance" => enable_button_capture(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                crate::show_main_window(tray.app_handle().clone());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    let tray = builder.build(app)?;
    let connection_state = app.state::<AppState>().supervisor.state().state;
    app.manage(TrayState {
        tray,
        connection,
        last_action,
        reconnect,
        disconnect,
        snapshot: Mutex::new(TraySnapshot {
            connection: Some(connection_state),
            last_action: None,
        }),
    });
    if let Some(tray) = app.try_state::<TrayState>() {
        tray.update_connection(connection_state);
    }
    Ok(())
}

pub fn update_connection(app: &AppHandle, state: DeviceState) {
    if let Some(tray) = app.try_state::<TrayState>() {
        tray.update_connection(state);
    }
}

pub fn record_action(app: &AppHandle, button: &str, gesture: &str, action: &str) {
    if let Some(tray) = app.try_state::<TrayState>() {
        tray.record_action(button, gesture, action);
    }
}

pub fn reset_last_action(app: &AppHandle) {
    if let Some(tray) = app.try_state::<TrayState>() {
        tray.reset_last_action();
    }
}

fn show_page(app: &AppHandle, page: &str) {
    crate::show_main_window(app.clone());
    let _ = app.emit("navigate-page", page);
}

fn request_reconnect(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if let Err(error) = state.retry().await {
            state.logger.warn(format!("托盘重连失败：{error}"));
            if let Some(tray) = app.try_state::<TrayState>() {
                tray.show_notice("重连失败，请查看诊断日志");
            }
        }
    });
}

fn request_disconnect(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if let Err(error) = state.disconnect().await {
            state.logger.warn(format!("托盘断开连接失败：{error}"));
            if let Some(tray) = app.try_state::<TrayState>() {
                tray.show_notice("断开连接失败，请查看诊断日志");
            }
        }
    });
}

fn export_diagnostics(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        match state.export_diagnostics().await {
            Ok(path) => {
                let name = Path::new(&path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(&path);
                if let Some(tray) = app.try_state::<TrayState>() {
                    tray.show_notice(&format!("诊断已导出：{name}"));
                }
                state.logger.info(format!("托盘已导出诊断信息：{path}"));
            }
            Err(error) => {
                state.logger.warn(format!("托盘导出诊断失败：{error}"));
                if let Some(tray) = app.try_state::<TrayState>() {
                    tray.show_notice("诊断导出失败，请查看日志");
                }
            }
        }
    });
}

fn enable_button_capture(app: &AppHandle) {
    let state = app.state::<AppState>();
    let result = std::env::current_exe()
        .map_err(|error| format!("无法定位当前程序：{error}"))
        .and_then(|executable| {
            remote_buddy_windows::capture_profile::request_elevated_capture_setup(&executable)
        });
    match result {
        Ok(()) => state.logger.info("托盘已请求启用按键增强"),
        Err(error) => {
            state.logger.warn(format!("托盘启用按键增强失败：{error}"));
            if let Some(tray) = app.try_state::<TrayState>() {
                tray.show_notice("按键增强启动失败，请查看日志");
            }
        }
    }
}

fn connection_label(state: DeviceState) -> &'static str {
    match state {
        DeviceState::Disconnected => "未连接",
        DeviceState::Connecting => "正在连接",
        DeviceState::Ready => "已连接",
        DeviceState::Degraded => "连接不稳定",
        DeviceState::ManualDisconnected => "已手动断开",
    }
}

fn button_label(button: &str) -> &str {
    match button {
        "Up" => "向上",
        "Down" => "向下",
        "Left" => "向左",
        "Right" => "向右",
        "Ok" => "确定",
        "Back" => "返回",
        "Home" => "主页",
        "Menu" => "菜单",
        "VolumeUp" => "音量 +",
        "VolumeDown" => "音量 -",
        "Mute" => "静音",
        "PlayPause" => "播放/暂停",
        "Power" => "电源",
        "YouTube" => "YouTube",
        "Netflix" => "Netflix",
        "LiveTv" => "直播电视",
        "Input" => "输入源",
        "Voice" => "语音键",
        "Custom" => "自定义键",
        _ => button,
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}
