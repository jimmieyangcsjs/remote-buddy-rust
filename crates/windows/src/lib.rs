pub mod apps;
pub mod audio;
pub mod ble;
pub mod capture_profile;
pub mod elevation;
pub mod external;
pub mod input;

pub fn platform_name() -> &'static str {
    "windows"
}
