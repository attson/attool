use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::models::{Phase, ReleaseInfo, Snapshot};

pub struct UpdaterState {
    pub inner: Mutex<Snapshot>,
    pub stage_dir: PathBuf,
    pub cancel: Arc<AtomicBool>,
    pub cached_at: Mutex<Option<Instant>>,
    pub cached_release: Mutex<Option<ReleaseInfo>>,
    pub staged_asset: Mutex<Option<StagedAsset>>,
}

pub struct StagedAsset {
    #[allow(dead_code)]
    pub version: String,
    pub path: PathBuf,
}

impl UpdaterState {
    pub fn new(stage_dir: PathBuf) -> Self {
        Self {
            inner: Mutex::new(Snapshot::default()),
            stage_dir,
            cancel: Arc::new(AtomicBool::new(false)),
            cached_at: Mutex::new(None),
            cached_release: Mutex::new(None),
            staged_asset: Mutex::new(None),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.inner.lock().expect("updater state poisoned").clone()
    }

    pub fn set_phase(&self, phase: Phase) {
        if let Ok(mut snap) = self.inner.lock() {
            snap.phase = phase;
        }
    }

    pub fn set_error(&self, msg: String) {
        if let Ok(mut snap) = self.inner.lock() {
            snap.error = Some(msg.clone());
            snap.phase = Phase::Error { message: msg };
        }
    }

    pub fn clear_error(&self) {
        if let Ok(mut snap) = self.inner.lock() {
            snap.error = None;
        }
    }

    pub fn mark_checked_at(&self, ts_ms: i64) {
        if let Ok(mut snap) = self.inner.lock() {
            snap.last_check_at = Some(ts_ms);
        }
    }
}
