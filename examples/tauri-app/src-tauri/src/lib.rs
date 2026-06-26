use steamworks::Client;

// Learn more about Tauri commands at https://v2.tauri.app/develop/calling-rust/#commands
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn activate_overlay(client: tauri::State<'_, Client>) {
    client.friends().activate_game_overlay("SteamOverlay");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let client = Client::init_app(480).unwrap();

    tauri::Builder::default()
        .manage(client)
        .invoke_handler(tauri::generate_handler![greet, activate_overlay])
        .plugin(tauri_plugin_steam_overlay::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
