use super::migrate::{ConfigError, parse_and_migrate, save_atomic};
use super::model::AppConfig;
use std::path::{Path, PathBuf};

pub fn default_config_path() -> PathBuf {
    let root = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    root.join("RemoteBuddy").join("config.json")
}

pub fn load_config(path: &Path) -> Result<AppConfig, ConfigError> {
    match std::fs::read_to_string(path) {
        Ok(raw) => parse_and_migrate(&raw),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(AppConfig::default()),
        Err(error) => Err(ConfigError::Io(error)),
    }
}

pub fn save_config_with_backup(path: &Path, config: &AppConfig) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let backup = path.with_file_name("config.backup.json");
    if path.is_file() && !backup.exists() {
        std::fs::copy(path, &backup)?;
    }

    save_atomic(path, config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn missing_config_returns_defaults() {
        let path = std::env::temp_dir().join("remote-buddy-missing-config.json");
        let _ = std::fs::remove_file(&path);
        assert_eq!(load_config(&path).unwrap(), AppConfig::default());
    }

    #[test]
    fn first_save_keeps_a_backup() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("remote-buddy-config-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, r#"{"Version":1,"Devices":[]}"#).unwrap();
        save_config_with_backup(&path, &AppConfig::default()).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("config.backup.json")).unwrap(),
            r#"{"Version":1,"Devices":[]}"#
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
