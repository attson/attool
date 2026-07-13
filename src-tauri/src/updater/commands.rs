use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, State};

use super::check::{build_release_info, fetch_latest_release, newer_than_current};
use super::download::{download_to_stage, fetch_text};
use super::keys::is_enabled;
use super::models::{Phase, Snapshot};
use super::state::{StagedAsset, UpdaterState};
use super::verify::{lookup_expected_sha256, verify_checksums_signature};

const CACHE_TTL: Duration = Duration::from_secs(600);

#[tauri::command]
pub fn updater_get_state(state: State<'_, UpdaterState>) -> Snapshot {
    state.snapshot()
}

#[tauri::command]
pub async fn updater_check(
    app: AppHandle,
    state: State<'_, UpdaterState>,
) -> Result<Snapshot, String> {
    if !is_enabled() {
        state.set_phase(Phase::UpToDate);
        emit_state(&app, &state);
        return Ok(state.snapshot());
    }

    state.clear_error();
    state.set_phase(Phase::Checking);
    emit_state(&app, &state);

    // 缓存 10 min 内直接返回上次结果
    let fresh_release = {
        let cached_at = state.cached_at.lock().ok().and_then(|g| *g);
        let expired = cached_at
            .map(|ts| ts.elapsed() > CACHE_TTL)
            .unwrap_or(true);
        if !expired {
            state.cached_release.lock().ok().and_then(|g| g.clone())
        } else {
            None
        }
    };

    let info = if let Some(cached) = fresh_release {
        cached
    } else {
        let release = fetch_latest_release().await.map_err(|error| {
            state.set_error(error.clone());
            emit_state(&app, &state);
            error
        })?;
        let info = build_release_info(&release).map_err(|error| {
            state.set_error(error.clone());
            emit_state(&app, &state);
            error
        })?;
        if let Ok(mut slot) = state.cached_release.lock() {
            *slot = Some(info.clone());
        }
        if let Ok(mut slot) = state.cached_at.lock() {
            *slot = Some(Instant::now());
        }
        info
    };

    state.mark_checked_at(chrono::Utc::now().timestamp_millis());

    let current = env!("CARGO_PKG_VERSION");
    if newer_than_current(&info.version, current) {
        state.set_phase(Phase::Available { info });
    } else {
        state.set_phase(Phase::UpToDate);
    }
    emit_state(&app, &state);
    Ok(state.snapshot())
}

#[tauri::command]
pub async fn updater_download(
    app: AppHandle,
    state: State<'_, UpdaterState>,
) -> Result<Snapshot, String> {
    if !is_enabled() {
        return Err("当前构建未启用更新校验".to_string());
    }
    let info = match &state.snapshot().phase {
        Phase::Available { info } => info.clone(),
        Phase::Ready { .. } => return Ok(state.snapshot()),
        _ => return Err("当前不在 available 状态".to_string()),
    };

    state.cancel.store(false, Ordering::SeqCst);
    state.set_phase(Phase::Downloading {
        pct: 0,
        downloaded: 0,
        total: info.asset_size,
    });
    emit_state(&app, &state);

    // fetch SHA256SUMS + sig
    let release = fetch_latest_release().await.map_err(|e| set_err(&app, &state, e))?;
    let base_url = release
        .assets
        .iter()
        .find(|a| a.name == "SHA256SUMS")
        .ok_or_else(|| set_err(&app, &state, "release 缺 SHA256SUMS".to_string()))?
        .browser_download_url
        .clone();
    let sig_url = release
        .assets
        .iter()
        .find(|a| a.name == "SHA256SUMS.sig")
        .ok_or_else(|| set_err(&app, &state, "release 缺 SHA256SUMS.sig".to_string()))?
        .browser_download_url
        .clone();

    let sums = fetch_text(&base_url).await.map_err(|e| set_err(&app, &state, e))?;
    let sig = fetch_text(&sig_url).await.map_err(|e| set_err(&app, &state, e))?;

    verify_checksums_signature(sums.as_bytes(), &sig)
        .map_err(|e| set_err(&app, &state, e))?;

    let expected = lookup_expected_sha256(&sums, &info.asset_name)
        .ok_or_else(|| set_err(&app, &state, format!("SHA256SUMS 中未找到 {}", info.asset_name)))?;

    let staged = download_to_stage(
        &app,
        &state.stage_dir,
        &info.asset_name,
        &info.asset_url,
        &expected,
        state.cancel.clone(),
    )
    .await
    .map_err(|e| set_err(&app, &state, e))?;

    state.set_phase(Phase::Verifying);
    emit_state(&app, &state);

    if let Ok(mut slot) = state.staged_asset.lock() {
        *slot = Some(StagedAsset {
            version: info.version.clone(),
            path: staged.staged_path.clone(),
        });
    }

    state.set_phase(Phase::Ready {
        version: info.version.clone(),
    });
    emit_state(&app, &state);
    Ok(state.snapshot())
}

#[tauri::command]
pub async fn updater_apply(
    app: AppHandle,
    state: State<'_, UpdaterState>,
) -> Result<(), String> {
    let staged_path = {
        let guard = state.staged_asset.lock().ok().and_then(|g| g.as_ref().map(|s| s.path.clone()));
        guard.ok_or("尚未下载完成，无法安装")?
    };

    state.set_phase(Phase::Applying);
    emit_state(&app, &state);

    super::apply::apply(&app, &staged_path).map_err(|e| set_err(&app, &state, e))?;
    Ok(())
}

#[tauri::command]
pub fn updater_cancel(app: AppHandle, state: State<'_, UpdaterState>) {
    state.cancel.store(true, Ordering::SeqCst);
    // Cancel 后回退到 available（若曾拿到 info）或 idle
    let cached = state.cached_release.lock().ok().and_then(|g| g.clone());
    if let Some(info) = cached {
        state.set_phase(Phase::Available { info });
    } else {
        state.set_phase(Phase::Idle);
    }
    emit_state(&app, &state);
}

fn emit_state(app: &AppHandle, state: &UpdaterState) {
    let snap = state.snapshot();
    let _ = app.emit("updater://state", snap);
}

fn set_err(app: &AppHandle, state: &UpdaterState, msg: String) -> String {
    state.set_error(msg.clone());
    emit_state(app, state);
    msg
}
