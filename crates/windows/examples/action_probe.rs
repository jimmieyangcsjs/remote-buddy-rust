use remote_buddy_core::config::AppConfig;

fn main() {
    let button = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Netflix".to_string());
    let config_path = std::env::var("APPDATA")
        .map(std::path::PathBuf::from)
        .unwrap()
        .join("RemoteBuddy")
        .join("config.json");
    let raw = std::fs::read_to_string(&config_path).expect("read config");
    let config: AppConfig = serde_json::from_str(&raw).expect("parse config");
    let device = config.devices.first().expect("device");
    let mapping = device
        .mappings
        .iter()
        .find(|mapping| mapping.button == button)
        .expect("mapping");
    match remote_buddy_windows::input::execute_action(&mapping.click) {
        Ok(()) => println!("action ok: {button}"),
        Err(error) => {
            eprintln!("action failed: {error}");
            std::process::exit(1);
        }
    }
}
