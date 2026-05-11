use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use crate::config::ResolvedConfig;
use crate::db::Database;
use crate::downloader::ProgressCallback;
use crate::model_workflow::WorkflowConfig;
use crate::video_workflow::scrape_and_save_video;

/// State of the batch manager (shared)
/// Public status snapshot
#[derive(Debug, Clone, Default)]
pub struct BatchStatus {
    pub queue_count: usize,
    pub active_count: usize,
    pub batch_total: usize,
    pub batch_current: usize,
    pub is_processing: bool,
}

#[derive(Debug)]
struct BatchState {
    queue: VecDeque<String>,
    active: HashSet<String>,
    batch_total: usize,
    batch_current: usize,
    is_processing: bool,
}

pub struct BatchManager {
    state: Arc<Mutex<BatchState>>,
    config: ResolvedConfig,
    db: Arc<Database>,
    notify: Arc<Notify>,
    // Callback to emit events to UI
    on_event: Option<ProgressCallback>,
    // Worker handle
    worker_handle: Option<JoinHandle<()>>,
    // Stop signal
    stop_tx: Option<tokio::sync::oneshot::Sender<()>>,
    // Tokio runtime handle
    rt_handle: tokio::runtime::Handle,
}

impl BatchManager {
    #[must_use]
    pub fn new(
        db: Arc<Database>,
        config: ResolvedConfig,
        on_event: Option<ProgressCallback>,
        rt_handle: tokio::runtime::Handle,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(BatchState {
                queue: VecDeque::new(),
                active: HashSet::new(),
                batch_total: 0,
                batch_current: 0,
                is_processing: false,
            })),
            config,
            db,
            notify: Arc::new(Notify::new()),
            on_event,
            worker_handle: None,
            stop_tx: None,
            rt_handle,
        }
    }

    #[must_use]
    pub fn status(&self) -> BatchStatus {
        let state = self.state.lock().unwrap_or_else(|e| {
            tracing::warn!("Batch state poisoned: {}", e);
            e.into_inner()
        });
        BatchStatus {
            queue_count: state.queue.len(),
            active_count: state.active.len(),
            batch_total: state.batch_total,
            batch_current: state.batch_current,
            is_processing: state.is_processing,
        }
    }

    pub fn enqueue(&self, url: String) {
        let mut state = self.state.lock().unwrap_or_else(|e| {
            tracing::warn!("Batch state poisoned: {}", e);
            e.into_inner()
        });
        // Simple dedupe
        if state.active.contains(&url) || state.queue.contains(&url) {
            return;
        }
        state.queue.push_back(url);
        state.batch_total += 1;

        let total = state.batch_total;
        let enqueued = state.queue.len();
        drop(state); // Drop lock before notifying

        self.notify.notify_one();

        // Emit event
        if let Some(cb) = &self.on_event {
            cb(serde_json::json!({
                "type": "batch_queued",
                "enqueued": enqueued,
                "total": total
            }));
        }
    }

    pub fn start(&mut self) {
        if self.worker_handle.is_some() {
            return;
        }

        let state = self.state.lock().unwrap_or_else(|e| {
            tracing::warn!("Batch state poisoned: {}", e);
            e.into_inner()
        });
        if state.queue.is_empty() {
            return; // Nothing to do
        }
        drop(state);

        let state = self.state.clone();
        let db = self.db.clone();
        let config = self.config.clone();
        let on_event = self.on_event.clone();
        let notify = self.notify.clone();

        let (tx, mut rx) = tokio::sync::oneshot::channel();
        self.stop_tx = Some(tx);

        let handle = self.rt_handle.spawn(async move {
            let mut did_work = false;
            loop {
                // Check if stopped
                if rx.try_recv().is_ok() {
                    break;
                }

                let next_url = {
                    let mut state = state.lock().unwrap_or_else(|e| {
                        tracing::warn!("Batch state poisoned (loop): {e}");
                        e.into_inner()
                    });
                    if state.queue.is_empty() {
                        if state.is_processing {
                            state.is_processing = false;
                        }
                        None
                    } else {
                        state.is_processing = true;
                        state.queue.pop_front()
                    }
                };

                if let Some(url) = next_url {
                    did_work = true;
                    // Pre-computation state
                    let (current, total) = {
                        let mut state = state.lock().unwrap_or_else(|e| {
                            tracing::warn!("Batch state poisoned (pre): {e}");
                            e.into_inner()
                        });
                        state.active.insert(url.clone());
                        (state.batch_current, state.batch_total)
                    };

                    // Emit "Starting" progress
                    if let Some(cb) = &on_event {
                        cb(serde_json::json!({
                            "type": "batch_progress_update",
                            "url": url,
                            "current": current + 1, // 1-based for UI "Processing 1 of 5"
                            "total": total
                        }));
                    }

                    // Process
                    let _ = Self::process_url(&db, &url, &config, on_event.as_ref()).await;

                    // Post-computation state
                    let (current, total) = {
                        let mut state = state.lock().unwrap_or_else(|e| {
                            tracing::warn!("Batch state poisoned (post): {e}");
                            e.into_inner()
                        });
                        state.active.remove(&url);
                        state.batch_current += 1;
                        (state.batch_current, state.batch_total)
                    };

                    // Emit "Finished" progress (or ready for next)
                    if let Some(cb) = &on_event {
                        cb(serde_json::json!({
                            "type": "batch_progress_update",
                            "url": url, // Still show completed URL briefly
                            "current": current,
                            "total": total
                        }));
                    }
                } else {
                    // Queue empty - Emit Batch Complete if we did work
                    if did_work {
                        if let Some(cb) = &on_event {
                            cb(serde_json::json!({
                                "type": "batch_complete"
                            }));
                        }
                        did_work = false;
                    }

                    // Wait for notification or stop
                    tokio::select! {
                        () = notify.notified() => {}
                        _ = &mut rx => { break; }
                    }
                }
            }
        });

        self.worker_handle = Some(handle);
    }

    pub fn stop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        self.worker_handle = None;
    }

    /// Process a single URL: Scrape -> Store -> Download.
    ///
    /// Returns the `page_id` if successful, None otherwise.
    #[allow(clippy::too_many_lines)]
    async fn process_url(
        db: &Arc<Database>,
        url: &str,
        config: &ResolvedConfig,
        on_event: Option<&ProgressCallback>,
    ) -> Option<i64> {
        let workflow_config = WorkflowConfig {
            models_dir: config.models_dir.clone(),
            flags_dir: config.flags_dir.clone(),
            covers_dir: config.covers_dir.clone(),
            thumbs_dir: config.thumbs_dir.clone(),
            previews_dir: config.previews_dir.clone(),
            scrapers_dir: config.scrapers_dir.clone(),
            download_delay_ms: config.download_delay_ms,
            ffmpeg_path: config.ffmpeg_path.clone(),
            ffprobe_path: config.ffprobe_path.clone(),
        };

        // Create a callback that bridges scrape_and_save_video's (stage, message)
        // to our (serde_json::Value) event system.
        let progress_cb = on_event.map(|cb| {
            let cb = Arc::new(cb.clone());
            Arc::new(move |stage: &str, msg: &str| {
                cb(serde_json::json!({
                    "type": "scrape_progress",
                    "stage": stage,
                    "message": msg
                }));
            }) as crate::scripting::WorkflowProgressCallback
        });

        match scrape_and_save_video(db, url, &workflow_config, progress_cb.as_ref()).await {
            Ok(result) => Some(result.page_id),
            Err(e) => {
                tracing::error!("Batch process failed for {url}: {e}");
                if let Some(cb) = on_event {
                    cb(serde_json::json!({
                        "type": "scrape_failed",
                        "url": url,
                        "error": format!("{e}")
                    }));
                }
                None
            }
        }
    }
}
