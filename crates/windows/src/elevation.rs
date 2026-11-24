use std::time::Duration;
use thiserror::Error;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_SYNCHRONIZE, WaitForSingleObject,
};
use windows::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, PCWSTR};

const ELEVATED_ARGUMENT: &str = "--elevated";
const WAIT_FOR_PID_ARGUMENT: &str = "--wait-for-pid";
const NO_ELEVATE_ARGUMENT: &str = "--no-elevate";
const ERROR_CANCELLED_HRESULT: i32 = 0x8007_04C7u32 as i32;

#[derive(Debug, Error)]
pub enum ElevationError {
    #[error("无法定位当前程序：{0}")]
    CurrentExecutable(#[from] std::io::Error),
    #[error("已取消管理员权限请求")]
    Cancelled,
    #[error("无法请求管理员权限：{0}")]
    Launch(String),
}

pub fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut returned = 0_u32;
        let result = GetTokenInformation(
            token,
            TokenElevation,
            Some((&mut elevation as *mut TOKEN_ELEVATION).cast()),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        );
        let _ = CloseHandle(token);
        result.is_ok() && elevation.TokenIsElevated != 0
    }
}

pub fn is_elevated_invocation() -> bool {
    std::env::args().any(|argument| argument == ELEVATED_ARGUMENT)
}

pub fn elevation_disabled_by_argument() -> bool {
    std::env::args().any(|argument| argument == NO_ELEVATE_ARGUMENT)
}

pub fn relaunch_self_elevated() -> Result<(), ElevationError> {
    let executable = std::env::current_exe()?;
    let parameters = format!(
        "{ELEVATED_ARGUMENT} {WAIT_FOR_PID_ARGUMENT} {}",
        std::process::id()
    );
    let operation = HSTRING::from("runas");
    let file = HSTRING::from(executable.as_os_str());
    let parameters = HSTRING::from(parameters);
    let mut execute = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: PCWSTR(operation.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    match unsafe { ShellExecuteExW(&mut execute) } {
        Ok(()) => {
            if !execute.hProcess.is_invalid() {
                let _ = unsafe { CloseHandle(execute.hProcess) };
            }
            Ok(())
        }
        Err(error) if error.code().0 == ERROR_CANCELLED_HRESULT => Err(ElevationError::Cancelled),
        Err(error) => Err(ElevationError::Launch(error.to_string())),
    }
}

pub fn wait_for_previous_instance() {
    let Some(pid) = argument_value(WAIT_FOR_PID_ARGUMENT).and_then(|value| value.parse().ok())
    else {
        return;
    };
    if pid == std::process::id() {
        return;
    }
    unsafe {
        match OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            Ok(handle) => {
                let _ = WaitForSingleObject(handle, 5_000);
                let _ = CloseHandle(handle);
            }
            Err(_) => std::thread::sleep(Duration::from_millis(400)),
        }
    }
}

fn argument_value(name: &str) -> Option<String> {
    find_argument_value(std::env::args().skip(1), name)
}

fn find_argument_value(mut arguments: impl Iterator<Item = String>, name: &str) -> Option<String> {
    while let Some(argument) = arguments.next() {
        if argument == name {
            return arguments.next();
        }
        if let Some(value) = argument.strip_prefix(&format!("{name}=")) {
            return Some(value.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_separate_and_equals_argument_values() {
        let separate = vec![
            "--elevated".to_string(),
            "--wait-for-pid".to_string(),
            "42".to_string(),
        ];
        assert_eq!(
            find_argument_value(separate.into_iter(), WAIT_FOR_PID_ARGUMENT),
            Some("42".to_string())
        );
        let equals = vec!["--wait-for-pid=7".to_string()];
        assert_eq!(
            find_argument_value(equals.into_iter(), WAIT_FOR_PID_ARGUMENT),
            Some("7".to_string())
        );
    }

    #[test]
    fn ignores_unrelated_arguments() {
        let arguments = vec!["--autostart".to_string()];
        assert_eq!(
            find_argument_value(arguments.into_iter(), WAIT_FOR_PID_ARGUMENT),
            None
        );
    }
}
