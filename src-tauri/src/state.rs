use anyhow::Result;
use futures_lite::StreamExt;
use iroh_docs::{rpc::client::docs::LiveEvent, ContentStatus};
use tauri::Emitter as _;
use tokio::sync::Mutex;

use crate::{todos::Todos, Iroh};

pub struct AppState {
    pub todos: Mutex<Option<(Todos, tokio::task::JoinHandle<()>)>>,
    iroh: Iroh,
}
impl AppState {
    pub fn new(iroh: Iroh) -> Self {
        AppState {
            todos: Mutex::new(None),
            iroh,
        }
    }

    pub fn iroh(&self) -> &Iroh {
        &self.iroh
    }

    pub async fn init_todos<R: tauri::Runtime>(
        &self,
        app_handle: tauri::AppHandle<R>,
        todos: Todos,
    ) -> Result<()> {
        let mut events = todos.doc_subscribe().await?;
        let id = todos.doc.id();
        let events_handle = tokio::spawn(async move {
            tracing::info!("Starting live event processing loop for doc: {}", id);
            loop {
                match events.next().await {
                    Some(Ok(event)) => {
                        tracing::debug!("Received live event: {:?}", event);
                        match event {
                            LiveEvent::InsertRemote { entry, content_status, .. } => {
                                tracing::info!(author = %entry.author(), key = ?String::from_utf8_lossy(entry.key()), status = ?content_status, "Event: InsertRemote");
                                if content_status == ContentStatus::Complete {
                                    tracing::debug!("InsertRemote: Content is complete, emitting update-all.");
                                    app_handle.emit("update-all", ()).ok();
                                }
                            }
                            LiveEvent::InsertLocal { entry } => {
                                tracing::info!(author = %entry.author(), key = ?String::from_utf8_lossy(entry.key()), "Event: InsertLocal");
                                app_handle.emit("update-all", ()).ok();
                            }
                            LiveEvent::ContentReady { hash } => {
                                tracing::info!(hash = %hash, "Event: ContentReady");
                                app_handle.emit("update-all", ()).ok();
                            }
                            LiveEvent::NeighborUp(id) => {
                                tracing::info!(peer = %id, "Event: NeighborUp");
                            }
                            LiveEvent::NeighborDown(id) => {
                                tracing::info!(peer = %id, "Event: NeighborDown");
                            }
                            LiveEvent::SyncFinished(sync_event) => {
                                tracing::info!(peer = %sync_event.peer, origin = ?sync_event.origin, result = ?sync_event.result, "Event: SyncFinished");
                            }
                            other => {
                                tracing::debug!("Unhandled live event: {:?}", other);
                            }
                        }
                    }
                    Some(Err(e)) => {
                        tracing::error!("Error in live event stream: {:?}", e);
                        break; // Exit loop on error
                    }
                    None => {
                        tracing::info!("Live event stream ended.");
                        break; // Exit loop as stream is finished
                    }
                }
            }
        });

        let mut t = self.todos.lock().await;
        if let Some((_t, handle)) = t.take() {
            handle.abort();
        }
        *t = Some((todos, events_handle));

        Ok(())
    }
}
