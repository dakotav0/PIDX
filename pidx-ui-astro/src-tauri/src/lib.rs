mod commands;

pub use commands::AppState;

/// Tauri app entry point — called from main.rs.
///
/// This is the Tauri equivalent of `main()` in the CLI. `generate_handler!` turns
/// the `#[tauri::command]` functions into an IPC handler table. The frontend calls
/// these by name: `invoke('list_users')`, `invoke('confirm_observation', { ... })`.
///
/// `manage(AppState::new())` registers the shared cache as injectable state —
/// Tauri's DI system injects it into any command that has `State<'_, AppState>`.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialise tracing — same setup as the CLI, stderr only
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::list_users,
            commands::get_profile,
            commands::get_show,
            commands::get_status,
            commands::confirm_observation,
            commands::reject_observation,
            commands::confirm_all,
            commands::reject_all,
            commands::clear,
            commands::ingest_packet,
            commands::resolve_delta,
            commands::annotate,
            commands::decay,
        ])
        .run(tauri::generate_context!())
        .expect("error while running pidx UI");
}
