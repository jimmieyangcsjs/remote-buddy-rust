#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(all(windows, not(target_pointer_width = "64")))]
compile_error!("Remote Buddy only supports 64-bit Windows.");

fn main() {
    if remote_buddy_windows::capture_profile::run_capture_setup_and_report() {
        return;
    }
    remote_buddy_windows::elevation::wait_for_previous_instance();
    remote_buddy_desktop::maybe_self_elevate();
    remote_buddy_desktop::run();
}
