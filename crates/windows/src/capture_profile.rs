use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::{CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_SET_VALUE, REG_DWORD, REG_OPTION_NON_VOLATILE, RRF_RT_REG_DWORD,
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegGetValueW, RegSetValueExW,
};
use windows::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::Win32::UI::WindowsAndMessaging::{
    MB_ICONERROR, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MessageBoxW, SW_SHOWNORMAL,
};
use windows::core::{HSTRING, PCWSTR};

const BTHPORT_PARAMETERS: &str = r"SYSTEM\CurrentControlSet\Services\BthPort\Parameters";
const LOG_SENSITIVE_DATA: &str = "EtwLogSensitiveData";
const MAX_ETW_BYTES: &str = "MaxEtwBytes";
const DROP_LARGE_EVENTS: &str = "EtwDropLargeEvents";
const ENABLE_ARGUMENT: &str = "--enable-button-capture";
const DISABLE_ARGUMENT: &str = "--disable-button-capture";
const BACKUP_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaptureProfileBackup {
    schema_version: u32,
    recorded_at_unix: u64,
    etw_log_sensitive_data: Option<u32>,
    max_etw_bytes: Option<u32>,
    etw_drop_large_events: Option<u32>,
}

impl CaptureProfileBackup {
    fn current() -> Self {
        Self {
            schema_version: BACKUP_SCHEMA_VERSION,
            recorded_at_unix: unix_now(),
            etw_log_sensitive_data: read_dword(LOG_SENSITIVE_DATA),
            max_etw_bytes: read_dword(MAX_ETW_BYTES),
            etw_drop_large_events: read_dword(DROP_LARGE_EVENTS),
        }
    }

    fn values(&self) -> [(&'static str, Option<u32>); 3] {
        [
            (LOG_SENSITIVE_DATA, self.etw_log_sensitive_data),
            (MAX_ETW_BYTES, self.max_etw_bytes),
            (DROP_LARGE_EVENTS, self.etw_drop_large_events),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RestoreAction {
    Write(&'static str, u32),
    Delete(&'static str),
}

#[derive(Debug, Clone, Default)]
pub struct CaptureRestoreReport {
    pub used_backup: bool,
    pub restored_values: usize,
    pub removed_values: usize,
    pub left_untouched: Vec<&'static str>,
}

pub fn capture_profile_ready() -> bool {
    read_dword(LOG_SENSITIVE_DATA) == Some(1)
        && read_dword(MAX_ETW_BYTES).is_some_and(|value| value >= 1024)
        && read_dword(DROP_LARGE_EVENTS) == Some(0)
}

pub fn capture_profile_backup_available() -> bool {
    backup_path().is_file()
}

pub fn enable_capture_profile() -> Result<(), String> {
    ensure_original_backup()?;
    let key = open_parameters_key().map_err(|status| {
        format!(
            "无法打开 BthPort 注册表配置（错误码 {}）。请确认已在 UAC 提示中选择“是”。",
            status.0
        )
    })?;
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

pub fn restore_capture_profile() -> Result<CaptureRestoreReport, String> {
    let backup = load_backup();
    let current = CaptureProfileBackup::current();
    let mut report = CaptureRestoreReport {
        used_backup: backup.is_some(),
        ..CaptureRestoreReport::default()
    };

    let actions = match backup.as_ref() {
        Some(saved) => saved
            .values()
            .into_iter()
            .map(|(name, value)| match value {
                Some(value) => RestoreAction::Write(name, value),
                None => RestoreAction::Delete(name),
            })
            .collect::<Vec<_>>(),
        None => legacy_restore_actions(&current, &mut report.left_untouched),
    };

    let key = open_parameters_key().map_err(|status| {
        format!(
            "无法打开 BthPort 注册表配置进行恢复（错误码 {}）。请确认已在 UAC 提示中选择“是”。",
            status.0
        )
    })?;
    let result = (|| {
        for action in actions {
            match action {
                RestoreAction::Write(name, value) => {
                    write_dword(key, name, value)?;
                    report.restored_values += 1;
                }
                RestoreAction::Delete(name) => {
                    delete_value(key, name)?;
                    report.removed_values += 1;
                }
            }
        }
        Ok::<_, String>(())
    })();
    unsafe {
        let _ = RegCloseKey(key);
    }
    result?;
    archive_backup();
    Ok(report)
}

pub fn is_capture_setup_invocation() -> bool {
    std::env::args().any(|argument| argument == ENABLE_ARGUMENT)
}

pub fn is_capture_restore_invocation() -> bool {
    std::env::args().any(|argument| argument == DISABLE_ARGUMENT)
}

pub fn request_elevated_capture_setup(executable: &Path) -> Result<(), String> {
    request_elevated_capture(executable, ENABLE_ARGUMENT)
}

pub fn request_elevated_capture_restore(executable: &Path) -> Result<(), String> {
    request_elevated_capture(executable, DISABLE_ARGUMENT)
}

pub fn run_capture_setup_and_report() -> bool {
    let enable = is_capture_setup_invocation();
    let restore = is_capture_restore_invocation();
    if !enable && !restore {
        return false;
    }

    let (title, message, icon) = if restore {
        match restore_capture_profile() {
            Ok(report) => (
                "Remote Buddy 蓝牙兼容恢复".to_string(),
                restore_message(&report),
                MB_ICONINFORMATION,
            ),
            Err(error) => (
                "Remote Buddy 蓝牙兼容恢复失败".to_string(),
                error,
                MB_ICONERROR,
            ),
        }
    } else {
        match enable_capture_profile() {
            Ok(()) => (
                "Remote Buddy 按键增强".to_string(),
                "Windows 蓝牙按键载荷已启用，原始注册表值已备份。\n\n\
                 请在 Windows 设置中关闭并重新打开蓝牙，或重启电脑，然后重新打开 Remote Buddy。\n\n\
                 注意：此设置作用于整块蓝牙适配器，也会让同机蓝牙鼠标/键盘报告进入全局 ETW 诊断路径。\
                 部分 Intel 蓝牙驱动对此较敏感；若鼠标出现掉线或驱动错误，请立即在诊断页点击“恢复蓝牙默认设置”。"
                    .to_string(),
                MB_ICONWARNING,
            ),
            Err(error) => (
                "Remote Buddy 按键增强失败".to_string(),
                error,
                MB_ICONERROR,
            ),
        }
    };
    let title = wide(&title);
    let message = wide(&message);
    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(message.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | icon,
        );
    }
    true
}

fn restore_message(report: &CaptureRestoreReport) -> String {
    let source = if report.used_backup {
        "已按启用前的备份恢复 BthPort 参数。"
    } else {
        "未找到旧版备份，因此只移除了与本程序写入值完全一致的 BthPort 参数，没有覆盖其他软件或手动配置。"
    };
    let untouched = if report.left_untouched.is_empty() {
        String::new()
    } else {
        format!(
            "\n\n以下值不是本程序写入，已保持原样：{}。如仍影响蓝牙，请先导出注册表后手动检查。",
            report.left_untouched.join(", ")
        )
    };
    format!(
        "{source}\n\n恢复 {} 项、删除 {} 项。请完全退出 Remote Buddy，然后关闭再打开 Windows 蓝牙，或重启电脑。{untouched}",
        report.restored_values, report.removed_values
    )
}

fn ensure_original_backup() -> Result<(), String> {
    let path = backup_path();
    if path.is_file() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("无法创建蓝牙配置备份目录：{error}"))?;
    }
    let backup = CaptureProfileBackup::current();
    let bytes = serde_json::to_vec_pretty(&backup)
        .map_err(|error| format!("无法序列化蓝牙配置备份：{error}"))?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, bytes).map_err(|error| format!("无法写入蓝牙配置备份：{error}"))?;
    std::fs::rename(&temporary, &path).map_err(|error| format!("无法保存蓝牙配置备份：{error}"))?;
    Ok(())
}

fn load_backup() -> Option<CaptureProfileBackup> {
    let raw = std::fs::read_to_string(backup_path()).ok()?;
    let backup: CaptureProfileBackup = serde_json::from_str(&raw).ok()?;
    (backup.schema_version == BACKUP_SCHEMA_VERSION).then_some(backup)
}

fn archive_backup() {
    let path = backup_path();
    if !path.is_file() {
        return;
    }
    let archived = path.with_file_name(format!("capture-profile.restored-{}.json", unix_now()));
    let _ = std::fs::rename(path, archived);
}

fn legacy_restore_actions(
    current: &CaptureProfileBackup,
    left_untouched: &mut Vec<&'static str>,
) -> Vec<RestoreAction> {
    let mut actions = Vec::new();
    for (name, current, enabled) in [
        (LOG_SENSITIVE_DATA, current.etw_log_sensitive_data, Some(1)),
        (MAX_ETW_BYTES, current.max_etw_bytes, Some(1024)),
        (DROP_LARGE_EVENTS, current.etw_drop_large_events, Some(0)),
    ] {
        if current == enabled {
            actions.push(RestoreAction::Delete(name));
        } else if current.is_some() {
            left_untouched.push(name);
        }
    }
    actions
}

fn request_elevated_capture(executable: &Path, argument: &str) -> Result<(), String> {
    let file = HSTRING::from(executable.as_os_str());
    let parameters = HSTRING::from(argument);
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

fn open_parameters_key() -> Result<HKEY, windows::Win32::Foundation::WIN32_ERROR> {
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
    if status == ERROR_SUCCESS {
        Ok(key)
    } else {
        Err(status)
    }
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

fn delete_value(key: HKEY, name: &str) -> Result<(), String> {
    let value_name = wide(name);
    let status = unsafe { RegDeleteValueW(key, PCWSTR(value_name.as_ptr())) };
    if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        Err(format!(
            "删除 {name} 失败（错误码 {}）。请确认当前进程具有管理员权限。",
            status.0
        ))
    }
}

fn backup_path() -> PathBuf {
    let root = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    root.join("RemoteBuddy").join("capture-profile.backup.json")
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn backup(values: [Option<u32>; 3]) -> CaptureProfileBackup {
        CaptureProfileBackup {
            schema_version: BACKUP_SCHEMA_VERSION,
            recorded_at_unix: 0,
            etw_log_sensitive_data: values[0],
            max_etw_bytes: values[1],
            etw_drop_large_events: values[2],
        }
    }

    #[test]
    fn maintenance_flags_are_opt_in() {
        assert!(!is_capture_setup_invocation());
        assert!(!is_capture_restore_invocation());
    }

    #[test]
    fn backup_round_trips_original_missing_values() {
        let original = backup([None, Some(2048), None]);
        let json = serde_json::to_string(&original).unwrap();
        let decoded: CaptureProfileBackup = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn legacy_restore_only_deletes_exact_app_values() {
        let current = backup([Some(1), Some(2048), Some(0)]);
        let mut untouched = Vec::new();
        let actions = legacy_restore_actions(&current, &mut untouched);
        assert_eq!(
            actions,
            vec![
                RestoreAction::Delete(LOG_SENSITIVE_DATA),
                RestoreAction::Delete(DROP_LARGE_EVENTS)
            ]
        );
        assert_eq!(untouched, vec![MAX_ETW_BYTES]);
    }

    #[test]
    fn legacy_restore_is_noop_when_values_were_not_written_by_app() {
        let current = backup([Some(0), Some(64), Some(1)]);
        let mut untouched = Vec::new();
        let actions = legacy_restore_actions(&current, &mut untouched);
        assert!(actions.is_empty());
        assert_eq!(
            untouched,
            vec![LOG_SENSITIVE_DATA, MAX_ETW_BYTES, DROP_LARGE_EVENTS]
        );
    }
}
