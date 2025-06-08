mod ipc;
mod iroh;
mod state;
mod todos;

use anyhow::{anyhow, Result};
use self::{iroh::Iroh, state::AppState};
use tauri::Manager;
#[cfg(debug_assertions)]
use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};

// setup an iroh node
async fn setup<R: tauri::Runtime>(handle: tauri::AppHandle<R>) -> Result<()> {
    // get the applicaiton data root, join with "iroh_data" to get the data root for the iroh node
    let data_root = handle
        .path()
        .app_data_dir()
        .map_err(|_| anyhow!("can't get application data directory"))?
        .join("iroh_data");

    let iroh = Iroh::new(data_root).await?;
    handle.manage(AppState::new(iroh));

    Ok(())
}

// Call this early, e.g., at the beginning of run() or setup()
#[cfg(debug_assertions)]
fn setup_logging() {
    // For Android, you might not be able to set ENV vars easily,
    // so a default can be provided.
    // Refined filter: INFO by default, DEBUG for app code and key iroh networking/sync.
    let default_filter = "info,tauri_todomvc_lib=debug,iroh_sync=debug,iroh_magicsock=debug,iroh_quinn_udp=debug,iroh=warn";
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter)),
        )
        .with_span_events(FmtSpan::CLOSE) // Less verbose span logging
        // On Android, logs to stderr will appear in logcat.
        // On desktop, they go to the console.
        .with_writer(std::io::stderr)
        .pretty()
        // ANSI codes might not render well in Logcat, so disable them.
        .with_ansi(false)
        .init();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(debug_assertions)]
    setup_logging(); // Initialize logging first
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            #[cfg(debug_assertions)] // only include this code on debug builds
            app.get_webview_window("main").unwrap().open_devtools();
            let handle = app.handle().clone();

            tauri::async_runtime::spawn(async move {
                println!("starting backend...");
                if let Err(err) = setup(handle).await {
                    eprintln!("failed: {:?}", err);
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::new_list,
            ipc::get_ticket,
            ipc::get_todos,
            ipc::new_todo,
            ipc::toggle_done,
            ipc::update_todo,
            ipc::delete,
            ipc::set_ticket,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
