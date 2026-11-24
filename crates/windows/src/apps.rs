use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use thiserror::Error;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentThreadId, OpenProcess, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GA_ROOT, GW_OWNER, GWL_EXSTYLE, GetAncestor,
    GetForegroundWindow, GetWindow, GetWindowLongPtrW, GetWindowTextW, GetWindowThreadProcessId,
    HWND_NOTOPMOST, HWND_TOP, HWND_TOPMOST, IsIconic, IsWindowVisible, PostMessageW, SC_RESTORE,
    SW_RESTORE, SW_SHOW, SW_SHOWNORMAL, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
    SetForegroundWindow, SetWindowPos, ShowWindow, ShowWindowAsync, SwitchToThisWindow,
    WM_SYSCOMMAND, WS_EX_TOPMOST,
};
use windows::core::{BOOL, PCWSTR, PWSTR};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningApplication {
    pub name: String,
    pub path: String,
    pub window_title: String,
    pub is_foreground: bool,
}

#[derive(Debug, Error)]
pub enum AppActivationError {
    #[error("application path is empty")]
    EmptyPath,
    #[error("application was not found: {0}")]
    NotFound(String),
    #[error("Windows could not start the application (ShellExecute code {0})")]
    Launch(isize),
    #[error(
        "Windows could not bring the application window to the foreground; if the target app runs as administrator, restart Remote Buddy as administrator"
    )]
    Focus,
    #[error("Windows API failed: {0}")]
    Windows(#[from] windows::core::Error),
}

struct ListContext {
    apps: Vec<RunningApplication>,
    own_pid: u32,
    foreground: HWND,
}

struct FindContext {
    path: String,
    name: String,
    found: Option<HWND>,
    title_match: Option<HWND>,
}

pub fn list_running_applications() -> Result<Vec<RunningApplication>, AppActivationError> {
    let mut context = ListContext {
        apps: Vec::new(),
        own_pid: std::process::id(),
        foreground: unsafe { GetForegroundWindow() },
    };
    unsafe {
        EnumWindows(
            Some(enum_application_windows),
            LPARAM((&mut context as *mut ListContext) as isize),
        )?
    };
    context.apps.sort_by(|left, right| {
        right
            .is_foreground
            .cmp(&left.is_foreground)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    Ok(context.apps)
}

pub fn focus_or_launch(path: &str) -> Result<(), AppActivationError> {
    focus_or_launch_named(path, "")
}

pub fn focus_or_launch_named(path: &str, name: &str) -> Result<(), AppActivationError> {
    let path = path.trim().trim_matches('"');
    if path.is_empty() {
        return Err(AppActivationError::EmptyPath);
    }

    if let Some(window) = find_application_window(path, name)? {
        return bring_to_foreground(window);
    }

    let path_buf = PathBuf::from(path);
    if !path_buf.exists() {
        return Err(AppActivationError::NotFound(path.to_string()));
    }
    launch_application(&path_buf)
}

unsafe extern "system" fn enum_application_windows(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let context = unsafe { &mut *(lparam.0 as *mut ListContext) };
    if !unsafe { IsWindowVisible(hwnd) }.as_bool() || window_has_owner(hwnd) {
        return BOOL(1);
    }

    let Some(pid) = window_process_id(hwnd) else {
        return BOOL(1);
    };
    if pid == context.own_pid {
        return BOOL(1);
    }
    let Some(path) = process_path(pid) else {
        return BOOL(1);
    };
    let Some(title) = window_title(hwnd) else {
        return BOOL(1);
    };
    if title == "Program Manager" {
        return BOOL(1);
    }
    let key = normalized_path_key(&path);
    if context
        .apps
        .iter()
        .any(|app| normalized_path_key(&app.path) == key)
    {
        return BOOL(1);
    }
    context.apps.push(RunningApplication {
        name: application_name(&path),
        path,
        window_title: title,
        is_foreground: hwnd == context.foreground,
    });
    BOOL(1)
}

unsafe extern "system" fn find_application_window_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let context = unsafe { &mut *(lparam.0 as *mut FindContext) };
    if context.found.is_some()
        || !unsafe { IsWindowVisible(hwnd) }.as_bool()
        || window_has_owner(hwnd)
    {
        return BOOL(1);
    }
    let Some(pid) = window_process_id(hwnd) else {
        return BOOL(1);
    };
    let Some(title) = window_title(hwnd) else {
        return BOOL(1);
    };
    if process_path(pid).is_some_and(|process_path| {
        normalized_path_key(&process_path) == normalized_path_key(&context.path)
    }) {
        context.found.get_or_insert(hwnd);
    } else if context.title_match.is_none()
        && title_matches_application(&title, &context.path, &context.name)
    {
        context.title_match = Some(hwnd);
    }
    BOOL(1)
}

fn find_application_window(path: &str, name: &str) -> Result<Option<HWND>, AppActivationError> {
    let mut context = FindContext {
        path: path.to_string(),
        name: name.to_string(),
        found: None,
        title_match: None,
    };
    unsafe {
        EnumWindows(
            Some(find_application_window_callback),
            LPARAM((&mut context as *mut FindContext) as isize),
        )?
    };
    Ok(context.found.or(context.title_match))
}

fn window_has_owner(hwnd: HWND) -> bool {
    unsafe { GetWindow(hwnd, GW_OWNER) }
        .ok()
        .is_some_and(|owner| !owner.0.is_null())
}

fn window_process_id(hwnd: HWND) -> Option<u32> {
    let mut pid = 0_u32;
    let thread = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    (thread != 0 && pid != 0).then_some(pid)
}

fn window_title(hwnd: HWND) -> Option<String> {
    let mut buffer = [0_u16; 512];
    let length = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    (length > 0).then(|| String::from_utf16_lossy(&buffer[..length as usize]))
}

fn process_path(pid: u32) -> Option<String> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut buffer = vec![0_u16; 32_768];
    let mut length = buffer.len() as u32;
    let result = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    };
    let _ = unsafe { CloseHandle(process) };
    result.ok()?;
    Some(String::from_utf16_lossy(&buffer[..length as usize]))
}

fn bring_to_foreground(hwnd: HWND) -> Result<(), AppActivationError> {
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindowAsync(hwnd, SW_RESTORE);
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = PostMessageW(
                Some(hwnd),
                WM_SYSCOMMAND,
                windows::Win32::Foundation::WPARAM(SC_RESTORE as usize),
                LPARAM(0),
            );
            for _ in 0..10 {
                if !IsIconic(hwnd).as_bool() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
        }

        let _ = ShowWindowAsync(hwnd, SW_SHOW);
        if window_is_foreground(hwnd) {
            let _ = SetFocus(Some(hwnd));
            return Ok(());
        }

        let was_topmost = is_topmost(hwnd);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOP),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );
        let _ = BringWindowToTop(hwnd);

        let foreground = GetForegroundWindow();
        let foreground_thread = GetWindowThreadProcessId(foreground, None);
        let current_thread = GetCurrentThreadId();
        let target_thread = GetWindowThreadProcessId(hwnd, None);
        let attachments =
            ThreadAttachments::attach(current_thread, &[foreground_thread, target_thread]);

        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(Some(hwnd));
        let attached_success = wait_for_foreground(hwnd, 180);
        drop(attachments);
        if attached_success {
            return Ok(());
        }

        request_foreground_with_alt_unlock(hwnd);
        if wait_for_foreground(hwnd, 220) {
            return Ok(());
        }

        if !was_topmost
            && SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
            )
            .is_ok()
        {
            let _ = BringWindowToTop(hwnd);
            let _ = SetForegroundWindow(hwnd);
            let _ = SetFocus(Some(hwnd));
            let activated = wait_for_foreground(hwnd, 220);
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_NOTOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
            if activated {
                return Ok(());
            }
        }

        SwitchToThisWindow(hwnd, true);
        if wait_for_foreground(hwnd, 200) {
            Ok(())
        } else {
            Err(AppActivationError::Focus)
        }
    }
}

fn request_foreground_with_alt_unlock(hwnd: HWND) {
    let _ = crate::input::send_input::unlock_foreground();
    unsafe {
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(Some(hwnd));
    }
}

fn wait_for_foreground(hwnd: HWND, timeout_ms: u64) -> bool {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        if window_is_foreground(hwnd) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn window_is_foreground(hwnd: HWND) -> bool {
    let foreground = unsafe { GetForegroundWindow() };
    if foreground.0.is_null() || hwnd.0.is_null() {
        return false;
    }
    let target_root = unsafe { GetAncestor(hwnd, GA_ROOT) };
    let foreground_root = unsafe { GetAncestor(foreground, GA_ROOT) };
    target_root == foreground_root || hwnd == foreground
}

fn is_topmost(hwnd: HWND) -> bool {
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
    style & WS_EX_TOPMOST.0 != 0
}

struct ThreadAttachments {
    current: u32,
    attached: Vec<u32>,
}

impl ThreadAttachments {
    unsafe fn attach(current: u32, threads: &[u32]) -> Self {
        let mut attached = Vec::new();
        for thread in threads {
            if *thread == 0 || *thread == current || attached.contains(thread) {
                continue;
            }
            if unsafe { AttachThreadInput(current, *thread, true) }.as_bool() {
                attached.push(*thread);
            }
        }
        Self { current, attached }
    }
}

impl Drop for ThreadAttachments {
    fn drop(&mut self) {
        for thread in self.attached.drain(..) {
            let _ = unsafe { AttachThreadInput(self.current, thread, false) };
        }
    }
}

fn launch_application(path: &Path) -> Result<(), AppActivationError> {
    let operation = wide("open");
    let file = wide(path.as_os_str());
    let code = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(operation.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
        .0 as isize
    };
    if code <= 32 {
        return Err(AppActivationError::Launch(code));
    }
    Ok(())
}

fn wide(value: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    value
        .as_ref()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn application_name(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "应用".to_string())
}

fn normalized_path_key(path: &str) -> String {
    let path = path.trim().trim_matches('"').replace('/', "\\");
    path.strip_prefix(r"\\?\")
        .unwrap_or(&path)
        .to_ascii_lowercase()
}

fn title_matches_application(title: &str, path: &str, name: &str) -> bool {
    let title = title.to_lowercase();
    application_match_tokens(path, name)
        .into_iter()
        .any(|token| title.contains(&token))
}

fn application_match_tokens(path: &str, name: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for value in [
        Path::new(path)
            .file_stem()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_default(),
        name.trim().to_string(),
    ] {
        for token in value.split(['-', '_', '.', ' ', '\\', '/']) {
            let token = token.trim().to_lowercase();
            if token.len() >= 4 && !tokens.contains(&token) {
                tokens.push(token);
            }
        }
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_executable_paths_case_insensitively() {
        assert_eq!(
            normalized_path_key(r"\\?\C:\Apps\Weixin.EXE"),
            normalized_path_key("c:/apps/weixin.exe")
        );
    }

    #[test]
    fn derives_a_friendly_application_name_from_the_executable() {
        assert_eq!(application_name(r"C:\Apps\Weixin.exe"), "Weixin");
        assert_eq!(application_name(""), "应用");
    }

    #[test]
    fn matches_a_protected_application_by_its_window_title() {
        assert!(title_matches_application(
            "Argus",
            r"C:\Program Files\Argus\argus-admin.exe",
            "argus-admin"
        ));
        assert!(!title_matches_application(
            "Notepad",
            r"C:\Program Files\Argus\argus-admin.exe",
            "argus-admin"
        ));
    }
}
