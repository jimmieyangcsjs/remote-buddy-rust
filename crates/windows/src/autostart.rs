use crate::elevation;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Output};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const TASK_NAME: &str = "Remote Buddy";
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "Remote Buddy";
const LEGACY_RUN_VALUES: &[&str] = &["remote-buddy-desktop"];

pub fn configure(
    executable: &Path,
    enabled: bool,
    run_as_admin: bool,
    silent: bool,
) -> Result<(), String> {
    if !enabled {
        let task_error = remove_admin_task().err();
        remove_user_run();
        return task_error.map_or(Ok(()), Err);
    }

    if run_as_admin {
        if !elevation::is_elevated() {
            return Err(
                "创建管理员开机启动需要管理员权限，请先以管理员身份重新启动 Remote Buddy。"
                    .to_string(),
            );
        }
        create_admin_task(executable, silent)?;
        remove_user_run();
        Ok(())
    } else {
        remove_admin_task()?;
        create_user_run(executable, silent)
    }
}

fn create_admin_task(executable: &Path, silent: bool) -> Result<(), String> {
    let user = current_user()?;
    let script = task_script(executable, &user, silent);
    let arguments = vec![
        "-NoProfile".to_string(),
        "-NonInteractive".to_string(),
        "-ExecutionPolicy".to_string(),
        "Bypass".to_string(),
        "-Command".to_string(),
        script,
    ];
    let output = run("powershell.exe", &arguments)?;
    command_result("创建管理员开机启动任务", output)
}

fn task_script(executable: &Path, user: &str, silent: bool) -> String {
    let user = powershell_quote(user);
    let executable = powershell_quote(&executable.display().to_string());
    let arguments = powershell_quote(&startup_arguments(silent));
    format!(
        "$ErrorActionPreference = 'Stop'\n\
$action = New-ScheduledTaskAction -Execute {executable} -Argument {arguments}\n\
$trigger = New-ScheduledTaskTrigger -AtLogOn -User {user}\n\
$principal = New-ScheduledTaskPrincipal -UserId {user} -LogonType Interactive -RunLevel Highest\n\
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable -ExecutionTimeLimit ([TimeSpan]::Zero) -MultipleInstances IgnoreNew\n\
Register-ScheduledTask -TaskName '{TASK_NAME}' -Description 'Start Remote Buddy with administrator privileges after sign-in.' -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Force | Out-Null"
    )
}

fn powershell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn remove_admin_task() -> Result<(), String> {
    if !admin_task_exists()? {
        return Ok(());
    }
    let arguments = vec![
        "/Delete".to_string(),
        "/TN".to_string(),
        TASK_NAME.to_string(),
        "/F".to_string(),
    ];
    let output = run("schtasks.exe", &arguments)?;
    command_result("删除管理员开机启动任务", output)
}

fn admin_task_exists() -> Result<bool, String> {
    let arguments = vec![
        "/Query".to_string(),
        "/TN".to_string(),
        TASK_NAME.to_string(),
    ];
    Ok(run("schtasks.exe", &arguments)?.status.success())
}

fn create_user_run(executable: &Path, silent: bool) -> Result<(), String> {
    remove_user_run();
    let action = startup_command(executable, silent);
    let arguments = vec![
        "add".to_string(),
        RUN_KEY.to_string(),
        "/v".to_string(),
        RUN_VALUE.to_string(),
        "/t".to_string(),
        "REG_SZ".to_string(),
        "/d".to_string(),
        action,
        "/f".to_string(),
    ];
    let output = run("reg.exe", &arguments)?;
    command_result("写入当前用户开机启动项", output)
}

fn remove_user_run() {
    for value in std::iter::once(RUN_VALUE).chain(LEGACY_RUN_VALUES.iter().copied()) {
        let arguments = vec![
            "delete".to_string(),
            RUN_KEY.to_string(),
            "/v".to_string(),
            value.to_string(),
            "/f".to_string(),
        ];
        let _ = run("reg.exe", &arguments);
    }
}

fn startup_command(executable: &Path, silent: bool) -> String {
    if silent {
        format!("\"{}\" --autostart --silent", executable.display())
    } else {
        format!("\"{}\" --autostart", executable.display())
    }
}

fn startup_arguments(silent: bool) -> String {
    if silent {
        "--autostart --silent".to_string()
    } else {
        "--autostart".to_string()
    }
}

fn current_user() -> Result<String, String> {
    let user = std::env::var("USERNAME")
        .map_err(|_| "无法读取当前 Windows 用户名，不能创建开机启动任务".to_string())?;
    let domain = std::env::var("USERDOMAIN").unwrap_or_default();
    if domain.is_empty() {
        Ok(user)
    } else {
        Ok(format!("{domain}\\{user}"))
    }
}

fn run(program: &str, arguments: &[String]) -> Result<Output, String> {
    Command::new(program)
        .args(arguments)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|error| format!("无法运行 {program}：{error}"))
}

fn command_result(action: &str, output: Output) -> Result<(), String> {
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if !stderr.is_empty() { stderr } else { stdout };
    if detail.is_empty() {
        Err(format!("{action}失败（退出码 {}）", output.status))
    } else {
        Err(format!("{action}失败：{detail}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_executable_path_with_spaces() {
        let executable = Path::new(r"C:\Program Files\Remote Buddy\remote-buddy-desktop.exe");
        assert_eq!(
            startup_command(executable, false),
            r#""C:\Program Files\Remote Buddy\remote-buddy-desktop.exe" --autostart"#
        );
    }

    #[test]
    fn adds_silent_argument_only_when_requested() {
        let executable = Path::new(r"C:\Remote Buddy\remote-buddy-desktop.exe");
        assert_eq!(
            startup_command(executable, true),
            r#""C:\Remote Buddy\remote-buddy-desktop.exe" --autostart --silent"#
        );
    }

    #[test]
    fn task_script_runs_elevated_and_allows_battery_power() {
        let executable = Path::new(r"C:\Program Files\Remote Buddy\remote-buddy-desktop.exe");
        let script = task_script(executable, r"STUDIO\player", true);
        assert!(script.contains("-RunLevel Highest"));
        assert!(script.contains("-AllowStartIfOnBatteries"));
        assert!(script.contains("-DontStopIfGoingOnBatteries"));
        assert!(script.contains("-Argument '--autostart --silent'"));
        assert!(
            script.contains("-Execute 'C:\\Program Files\\Remote Buddy\\remote-buddy-desktop.exe'")
        );
    }

    #[test]
    fn task_script_quotes_apostrophes_in_paths_and_users() {
        let executable = Path::new(r"C:\Tools\Player's remote.exe");
        let script = task_script(executable, r"STUDIO\O'Brien", false);
        assert!(script.contains("-UserId 'STUDIO\\O''Brien'"));
        assert!(script.contains("-Execute 'C:\\Tools\\Player''s remote.exe'"));
    }
}
