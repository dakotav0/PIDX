mod commands;

pub use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
            commands::ingest_packet_content,
            commands::resolve_delta,
            commands::resolve_review,
            commands::annotate,
            commands::decay,
        ])
        .run(tauri::generate_context!())
        .expect("error while running pidx UI");
}
