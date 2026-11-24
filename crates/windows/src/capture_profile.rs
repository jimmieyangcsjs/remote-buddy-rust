use std::path::Path;
use windows::Win32::Foundation::{CloseHandle, ERROR_SUCCESS};
use windows::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_SET_VALUE, REG_DWORD, REG_OPTION_NON_VOLATILE, RRF_RT_REG_DWORD,
    RegCloseKey, RegCreateKeyExW, RegGetValueW, RegSetValueExW,
};
use windows::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::Win32::UI::WindowsAndMessaging::{
    MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MessageBoxW, SW_SHOWNORMAL,
};
use windows::core::{HSTRING, PCWSTR};

const BTHPORT_PARAMETERS: &str = r"SYSTEM\CurrentControlSet\Services\BthPort\Parameters";
const LOG_SENSITIVE_DATA: &str = "EtwLogSensitiveData";
const MAX_ETW_BYTES: &str = "MaxEtwBytes";
const DROP_LARGE_EVENTS: &str = "EtwDropLargeEvents";
const ENABLE_ARGUMENT: &str = "--enable-button-capture";

pub fn capture_profile_ready() -> bool {
    read_dword(LOG_SENSITIVE_DATA) == Some(1)
        && read_dword(MAX_ETW_BYTES).is_some_and(|value| value >= 1024)
        && read_dword(DROP_LARGE_EVENTS) == Some(0)
}

pub fn enable_capture_profile() -> Result<(), String> {
    let subkey = wide(BTHPORT_PARAMETERS);
    let mut key = HKEY::default();
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(subkey.as_ptr()),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(format!(
            "无法打开 BthPort 注册表配置（错误码 {}）。请确认已在 UAC 提示中选择“是”。",
            status.0
        ));
    }

    let result = (|| {
        write_dword(key, LOG_SENSITIVE_DATA, 1)?;
        write_dword(key, MAX_ETW_BYTES, 1024)?;
        write_dword(key, DROP_LARGE_EVENTS, 0)
    })();
    unsafe {
        let _ = RegCloseKey(key);
    }
    result
}

pub fn is_capture_setup_invocation() -> bool {
    std::env::args().any(|argument| argument == ENABLE_ARGUMENT)
}

pub fn request_elevated_capture_setup(executable: &Path) -> Result<(), String> {
    let file = HSTRING::from(executable.as_os_str());
    let parameters = HSTRING::from(ENABLE_ARGUMENT);
    let operation = HSTRING::from("runas");
    let mut execute = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: PCWSTR(operation.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe {
        ShellExecuteExW(&mut execute).map_err(|error| {
            format!("无法启动管理员配置进程：{error}。如果取消了 UAC，请重新点击按钮。")
        })?;
        if !execute.hProcess.is_invalid() && !execute.hProcess.0.is_null() {
            let _ = CloseHandle(execute.hProcess);
        }
    }
    Ok(())
}

pub fn run_capture_setup_and_report() -> bool {
    if !is_capture_setup_invocation() {
        return false;
    }
    let (title, message, error) = match enable_capture_profile() {
        Ok(()) => (
            "Remote Buddy 按键增强".to_string(),
            "Windows 蓝牙按键载荷已启用。\n\n请在 Windows 设置中关闭并重新打开蓝牙，或重启电脑，然后重新打开 Remote Buddy。\n\n该设置仅让本机蓝牙驱动向本机 ETW 会话提供完整报告，不联网，也不上传数据。".to_string(),
            false,
        ),
        Err(error) => ("Remote Buddy 按键增强失败".to_string(), error, true),
    };
    let title = wide(&title);
    let message = wide(&message);
    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(message.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK
                | if error {
                    MB_ICONERROR
                } else {
                    MB_ICONINFORMATION
                },
        );
    }
    true
}

fn read_dword(name: &str) -> Option<u32> {
    let subkey = wide(BTHPORT_PARAMETERS);
    let value_name = wide(name);
    let mut value = 0_u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(value_name.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        )
    };
    (status == ERROR_SUCCESS && size == std::mem::size_of::<u32>() as u32).then_some(value)
}

fn write_dword(key: HKEY, name: &str, value: u32) -> Result<(), String> {
    let value_name = wide(name);
    let status = unsafe {
        RegSetValueExW(
            key,
            PCWSTR(value_name.as_ptr()),
            None,
            REG_DWORD,
            Some(&value.to_le_bytes()),
        )
    };
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(format!(
            "写入 {name} 失败（错误码 {}）。请确认当前进程具有管理员权限。",
            status.0
        ))
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maintenance_flag_is_opt_in() {
        assert!(!is_capture_setup_invocation());
    }

    #[test]
    fn wide_string_is_nul_terminated() {
        let value = wide(LOG_SENSITIVE_DATA);
        assert_eq!(value.last(), Some(&0));
        assert!(value.len() > LOG_SENSITIVE_DATA.len());
    }
}
