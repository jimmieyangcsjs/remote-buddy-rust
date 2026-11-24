use remote_buddy_core::config::AppConfig;
use serde_json::Value;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

const REDACTED: &str = "[redacted]";

pub fn export(
    directory: &Path,
    system: &Value,
    config: &AppConfig,
    logs: &str,
    hid_tap: &Value,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let path = directory.join(format!(
        "remote-buddy-{}.zip",
        chrono::Local::now().format("%Y%m%d-%H%M%S%.3f")
    ));
    let archive =
        build_archive(system, config, logs, hid_tap).map_err(|error| error.to_string())?;
    std::fs::write(&path, archive).map_err(|error| error.to_string())?;
    Ok(path)
}

pub fn build_archive(
    system: &Value,
    config: &AppConfig,
    logs: &str,
    hid_tap: &Value,
) -> zip::result::ZipResult<Vec<u8>> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    write_json(&mut writer, "system.json", system, options)?;
    write_json(
        &mut writer,
        "config.redacted.json",
        &redacted_config(config),
        options,
    )?;
    writer.start_file("logs/app.log", options)?;
    writer.write_all(logs.as_bytes())?;
    write_json(&mut writer, "hid-tap-status.json", hid_tap, options)?;
    Ok(writer.finish()?.into_inner())
}

fn write_json(
    writer: &mut zip::ZipWriter<Cursor<Vec<u8>>>,
    name: &str,
    value: &Value,
    options: zip::write::SimpleFileOptions,
) -> zip::result::ZipResult<()> {
    writer.start_file(name, options)?;
    serde_json::to_writer_pretty(&mut *writer, value).map_err(std::io::Error::other)?;
    Ok(())
}

pub fn redacted_config(config: &AppConfig) -> Value {
    let mut value = serde_json::to_value(config).expect("AppConfig must serialize");
    redact_value(&mut value);
    value
}

fn redact_value(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if matches!(
                    key.as_str(),
                    "id" | "name"
                        | "containerId"
                        | "bluetoothAddress"
                        | "formattedAddress"
                        | "selectedDeviceId"
                ) {
                    *value = Value::String(REDACTED.to_string());
                } else {
                    redact_value(value);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                redact_value(value);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
fn entry_text(archive: &[u8], name: &str) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(archive.to_vec())).expect("zip");
    let mut file = zip.by_name(name).expect("entry");
    let mut text = String::new();
    std::io::Read::read_to_string(&mut file, &mut text).expect("utf-8 entry");
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use remote_buddy_core::config::{DeviceConfig, InputToolConfig, MappingConfig};
    use serde_json::json;

    fn fixture_config() -> AppConfig {
        AppConfig {
            selected_device_id: Some("11:22:33:44:55:66".to_string()),
            devices: vec![DeviceConfig {
                id: "11:22:33:44:55:66".to_string(),
                name: "Private Living Room Remote".to_string(),
                model_type: "GoogleG9N9N".to_string(),
                container_id: Some("37f310c9-5665-5283-acaa-87b21ce3347b".to_string()),
                bluetooth_address: 18838586676582,
                formatted_address: "11:22:33:44:55:66".to_string(),
                voice_mode: Default::default(),
                gain: 3.621714,
                mappings: vec![MappingConfig::none("Ok")],
                extensions: Default::default(),
            }],
            input_tool: InputToolConfig::default(),
            ..AppConfig::default()
        }
    }

    #[test]
    fn archive_redacts_device_identity() {
        let archive = build_archive(
            &json!({"appVersion": "0.1.0", "configPath": "C:\\Users\\test\\config.json"}),
            &fixture_config(),
            "INFO application started\n",
            &json!({"state": "degraded", "detail": "waiting"}),
        )
        .expect("archive");

        let text = entry_text(&archive, "config.redacted.json");
        assert!(!text.contains("Private Living Room Remote"));
        assert!(!text.contains("11:22:33:44:55:66"));
        assert!(!text.contains("18838586676582"));
        assert!(!text.contains("37f310c9-5665-5283-acaa-87b21ce3347b"));
        assert!(text.contains("GoogleG9N9N"));
        assert!(text.contains("Ok"));
    }

    #[test]
    fn archive_contains_the_documented_files() {
        let archive = build_archive(
            &json!({"appVersion": "0.1.0"}),
            &fixture_config(),
            "INFO test log\n",
            &json!({"state": "ready"}),
        )
        .expect("archive");

        let mut zip = zip::ZipArchive::new(Cursor::new(archive)).expect("zip");
        let mut names = (0..zip.len())
            .map(|index| zip.by_index(index).unwrap().name().to_string())
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(
            names,
            vec![
                "config.redacted.json",
                "hid-tap-status.json",
                "logs/app.log",
                "system.json",
            ]
        );
    }
}
