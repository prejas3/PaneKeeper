pub mod windows;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    windows::enable_dpi_awareness();
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running WindowsKeeper");
}
