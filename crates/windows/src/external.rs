use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, PCWSTR};

pub fn open_https_url(url: &str) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("仅允许打开 HTTPS 链接".to_string());
    }
    let operation = HSTRING::from("open");
    let target = HSTRING::from(url);
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(operation.as_ptr()),
            PCWSTR(target.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize <= 32 {
        Err(format!("系统无法打开链接（错误码 {}）", result.0 as isize))
    } else {
        Ok(())
    }
}
