use serde::Serialize;
use std::collections::VecDeque;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

const LOG_LIMIT: usize = 500;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub time: String,
    pub level: String,
    pub message: String,
}

#[derive(Clone)]
pub struct AppLogger {
    app: AppHandle,
    entries: Arc<Mutex<VecDeque<LogEntry>>>,
    path: Option<PathBuf>,
}

impl AppLogger {
    pub fn new(app: AppHandle) -> Self {
        let path = std::env::var_os("APPDATA").map(|root| {
            PathBuf::from(root)
                .join("RemoteBuddy")
                .join("logs")
                .join("rust-app.log")
        });
        if let Some(path) = &path
            && let Some(parent) = path.parent()
        {
            let _ = std::fs::create_dir_all(parent);
        }
        Self {
            app,
            entries: Arc::new(Mutex::new(VecDeque::with_capacity(LOG_LIMIT))),
            path,
        }
    }

    pub fn info(&self, message: impl Into<String>) {
        self.push("INFO", message.into());
    }

    pub fn warn(&self, message: impl Into<String>) {
        self.push("WARN", message.into());
    }

    pub fn error(&self, message: impl Into<String>) {
        self.push("ERROR", message.into());
    }

    pub fn entries(&self) -> Vec<LogEntry> {
        self.entries
            .lock()
            .map(|entries| entries.iter().cloned().collect())
            .unwrap_or_default()
    }

    #[allow(dead_code)]
    pub fn log_path(&self) -> Option<PathBuf> {
        self.path.clone()
    }

    fn push(&self, level: &str, message: String) {
        let entry = LogEntry {
            time: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
            level: level.to_string(),
            message,
        };

        if let Ok(mut entries) = self.entries.lock() {
            if entries.len() == LOG_LIMIT {
                entries.pop_front();
            }
            entries.push_back(entry.clone());
        }

        if let Some(path) = &self.path
            && let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
        {
            let _ = writeln!(
                file,
                "[{}] [{:<5}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                entry.level,
                entry.message
            );
        }

        let _ = self.app.emit("log-appended", entry);
    }
}
