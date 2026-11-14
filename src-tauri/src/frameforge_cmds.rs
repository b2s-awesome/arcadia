//! Tauri commands for FrameForge (the Performance section).
//!
//! The commands are thin: every decision lives in `arcadia_core::frameforge`.
//! Anything that scans hardware, talks to a driver or lists processes runs on
//! the blocking pool so the UI thread never waits on it.

use crate::{err, AppState, Res};
use arcadia_core::frameforge::{self as ff, FrameForge};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

/// FrameForge is created on first use, not at start-up, so opening Arcadia
/// does not load any GPU driver library until the user needs it.
pub(crate) fn forge(app: &AppHandle) -> Arc<FrameForge> {
    let st = app.state::<AppState>();
    st.ff
        .get_or_init(|| {
            let f = Arc::new(FrameForge::new());
            f.set_games(&st.games.lock());
            f
        })
        .clone()
}

/// Run blocking FrameForge work off the UI thread.
async fn blocking<T: Send + 'static>(
    app: &AppHandle,
    f: impl FnOnce(Arc<FrameForge>) -> T + Send + 'static,
) -> Res<T> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || f(forge(&app)))
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn ff_overview(app: AppHandle, rescan: bool) -> Res<ff::service::Overview> {
    blocking(&app, move |f| f.overview(rescan)).await
}

#[tauri::command]
pub async fn ff_telemetry(app: AppHandle) -> Res<ff::telemetry::Sample> {
    blocking(&app, |f| f.telemetry()).await
}

#[tauri::command]
pub async fn ff_processes(app: AppHandle) -> Res<Vec<ff::processes::ProcessInfo>> {
    blocking(&app, |f| f.processes()).await
}

/// The running process most likely to be a library game, if any.
#[tauri::command]
pub async fn ff_running_game(app: AppHandle) -> Res<Option<ff::processes::ProcessInfo>> {
    blocking(&app, |f| {
        ff::processes::pick_running_game(&f.processes()).cloned()
    })
    .await
}

#[tauri::command]
pub async fn ff_presentmon(app: AppHandle) -> Res<ff::presentmon::Availability> {
    blocking(&app, |f| f.presentmon()).await
}

#[tauri::command]
pub async fn ff_config_get(app: AppHandle) -> Res<ff::config::Config> {
    blocking(&app, |f| f.config()).await
}

#[tauri::command]
pub async fn ff_config_set(app: AppHandle, config: ff::config::Config) -> Res<ff::config::Config> {
    blocking(&app, move |f| f.set_config(config).map(|_| f.config()))
        .await?
        .map_err(err)
}

#[tauri::command]
pub async fn ff_monitor_start(app: AppHandle, pid: u32, name: String) -> Res<()> {
    blocking(&app, move |f| f.monitor_start(pid, name))
        .await?
        .map_err(err)
}

#[tauri::command]
pub async fn ff_monitor_stop(app: AppHandle) -> Res<()> {
    blocking(&app, |f| f.monitor_stop()).await
}

#[tauri::command]
pub async fn ff_monitor_snapshot(app: AppHandle) -> Res<ff::monitor::Snapshot> {
    blocking(&app, |f| f.monitor_snapshot()).await
}

// ── diagnostics and optimiser ───────────────────────────────────────────

#[tauri::command]
pub async fn ff_diagnose(app: AppHandle) -> Res<ff::service::Diagnosis> {
    blocking(&app, |f| f.diagnose()).await
}

#[tauri::command]
pub async fn ff_opt_list(app: AppHandle) -> Res<Vec<ff::optimizer::ActionView>> {
    blocking(&app, |f| f.optimizer_list()).await
}

/// Apply the selected actions as one session. `level` is the level the user
/// confirmed in the dialog. The stored session is returned even when an
/// action failed, so the UI can show exactly what happened.
#[tauri::command]
pub async fn ff_opt_apply(
    app: AppHandle,
    requests: Vec<ff::optimizer::ApplyRequest>,
    level: u8,
    label: String,
) -> Res<ff::optimizer::Session> {
    blocking(&app, move |f| f.optimizer_apply(&requests, level, &label))
        .await?
        .map_err(err)
}

#[tauri::command]
pub async fn ff_opt_revert_change(
    app: AppHandle,
    session_id: String,
    change_id: u32,
) -> Res<ff::optimizer::Session> {
    blocking(&app, move |f| {
        f.optimizer_revert_change(&session_id, change_id)
    })
    .await?
    .map_err(err)
}

#[tauri::command]
pub async fn ff_opt_revert_session(
    app: AppHandle,
    session_id: String,
) -> Res<ff::optimizer::Session> {
    blocking(&app, move |f| f.optimizer_revert_session(&session_id))
        .await?
        .map_err(err)
}

#[tauri::command]
pub async fn ff_opt_history(app: AppHandle) -> Res<Vec<ff::optimizer::Session>> {
    blocking(&app, |f| f.optimizer_history()).await
}

/// Open one of a fixed list of Windows Settings pages.
#[tauri::command]
pub async fn ff_open_settings(app: AppHandle, uri: String) -> Res<()> {
    blocking(&app, move |f| f.open_settings_page(&uri))
        .await?
        .map_err(err)
}

// ── NVIDIA driver profiles ──────────────────────────────────────────────

use ff::nvidia::{self as nv, nip};
use std::path::PathBuf;

/// Largest `.nip` file read from disk (same limit the parser enforces).
const NIP_LIMIT: u64 = nip::MAX_FILE_BYTES as u64;

#[tauri::command]
pub async fn ff_nv_status(app: AppHandle) -> Res<nv::NvStatus> {
    blocking(&app, |f| f.nvidia().status()).await
}

#[tauri::command]
pub async fn ff_nv_profiles(app: AppHandle, refresh: bool) -> Res<Vec<nv::ProfileSummary>> {
    blocking(&app, move |f| f.nvidia().list(refresh)).await?.map_err(err)
}

#[tauri::command]
pub async fn ff_nv_profile(app: AppHandle, name: String) -> Res<nv::ProfileDetail> {
    blocking(&app, move |f| f.nvidia().detail(&name)).await?.map_err(err)
}

/// What the changes would do. Nothing is written.
#[tauri::command]
pub async fn ff_nv_plan(app: AppHandle, profile: String, changes: Vec<nv::Change>) -> Res<Vec<nv::store::DiffLine>> {
    blocking(&app, move |f| f.nvidia().plan(&profile, &changes)).await?.map_err(err)
}

/// Snapshot, apply through DRS, save, read back, record.
#[tauri::command]
pub async fn ff_nv_apply(app: AppHandle, profile: String, changes: Vec<nv::Change>) -> Res<nv::ApplyOutcome> {
    blocking(&app, move |f| f.nvidia().apply(&profile, &changes, None)).await?.map_err(err)
}

#[tauri::command]
pub async fn ff_nv_create_profile(app: AppHandle, name: String, exe: Option<String>) -> Res<nv::ApplyOutcome> {
    blocking(&app, move |f| f.nvidia().create_profile(&name, exe.as_deref())).await?.map_err(err)
}

#[tauri::command]
pub async fn ff_nv_delete_profile(app: AppHandle, name: String) -> Res<nv::ApplyOutcome> {
    blocking(&app, move |f| f.nvidia().delete_profile(&name)).await?.map_err(err)
}

/// Export profiles to a `.nip` file. An empty list exports every profile the user changed.
#[tauri::command]
pub async fn ff_nv_export(app: AppHandle, path: String, names: Vec<String>) -> Res<usize> {
    blocking(&app, move |f| {
        let names = if names.is_empty() { f.nvidia().customised_names()? } else { names };
        f.nvidia().export_to(&PathBuf::from(path), &names)
    })
    .await?
    .map_err(err)
}

/// Read a `.nip` file and show it next to the live driver state. Nothing is written.
#[tauri::command]
pub async fn ff_nv_import_preview(app: AppHandle, path: String) -> Res<nv::ImportPreview> {
    blocking(&app, move |f| {
        let bytes = FrameForge::read_user_file(&PathBuf::from(path), NIP_LIMIT)?;
        f.nvidia().import_preview(&bytes)
    })
    .await?
    .map_err(err)
}

/// Re-evaluate a file the user edited in the import dialog.
#[tauri::command]
pub async fn ff_nv_preview(app: AppHandle, file: nip::NipFile) -> Res<nv::ImportPreview> {
    blocking(&app, move |f| {
        let checked = nip::parse(&nip::write(&file)).map_err(|e| anyhow::anyhow!(e))?;
        Ok::<_, anyhow::Error>(f.nvidia().preview(checked))
    })
    .await?
    .map_err(err)
}

#[tauri::command]
pub async fn ff_nv_import(app: AppHandle, file: nip::NipFile, only: Vec<String>, mode: nv::ImportMode, label: String) -> Res<nv::ImportOutcome> {
    blocking(&app, move |f| f.nvidia().import(&file, &only, mode, &label)).await?.map_err(err)
}

/// Save an edited (not applied) profile file.
#[tauri::command]
pub async fn ff_nv_save_file(app: AppHandle, path: String, file: nip::NipFile) -> Res<()> {
    blocking(&app, move |f| f.nvidia().save_file(&PathBuf::from(path), &file)).await?.map_err(err)
}

#[tauri::command]
pub async fn ff_nv_compare(app: AppHandle, a: String, b: String) -> Res<nip::NipDiff> {
    blocking(&app, move |f| {
        let fa = FrameForge::read_user_file(&PathBuf::from(a), NIP_LIMIT)?;
        let fb = FrameForge::read_user_file(&PathBuf::from(b), NIP_LIMIT)?;
        f.nvidia().compare_files(&fa, &fb)
    })
    .await?
    .map_err(err)
}

#[tauri::command]
pub async fn ff_nv_backups(app: AppHandle) -> Res<Vec<nv::store::BackupEntry>> {
    blocking(&app, |f| f.nvidia().backups()).await
}

#[tauri::command]
pub async fn ff_nv_backup_now(app: AppHandle, names: Vec<String>, label: String) -> Res<nv::store::BackupEntry> {
    blocking(&app, move |f| {
        let names = if names.is_empty() { f.nvidia().customised_names()? } else { names };
        f.nvidia().backup_now(&names, &label)
    })
    .await?
    .map_err(err)
}

#[tauri::command]
pub async fn ff_nv_backup_contents(app: AppHandle, id: String) -> Res<nv::ImportPreview> {
    blocking(&app, move |f| f.nvidia().backup_contents(&id)).await?.map_err(err)
}

#[tauri::command]
pub async fn ff_nv_restore_backup(app: AppHandle, id: String) -> Res<nv::ImportOutcome> {
    blocking(&app, move |f| f.nvidia().restore_backup(&id)).await?.map_err(err)
}

#[tauri::command]
pub async fn ff_nv_delete_backup(app: AppHandle, id: String) -> Res<()> {
    blocking(&app, move |f| f.nvidia().delete_backup(&id)).await?.map_err(err)
}

/// Write NVIDIA's own dump of the whole profile database (not a `.nip` file).
#[tauri::command]
pub async fn ff_nv_save_database(app: AppHandle, path: String) -> Res<()> {
    blocking(&app, move |f| f.nvidia().save_driver_database(&PathBuf::from(path))).await?.map_err(err)
}

/// Fallback: hand a `.nip` file to the user's own NVIDIA Profile Inspector.
#[tauri::command]
pub async fn ff_nv_npi_import(app: AppHandle, path: String) -> Res<nv::npi::NpiRun> {
    blocking(&app, move |f| {
        let exe = f.config().npi_exe.ok_or("Choose your NVIDIA Profile Inspector executable first")?;
        nv::npi::import_with(&PathBuf::from(exe), &[PathBuf::from(path)])
    })
    .await?
    .map_err(err)
}

/// Fallback: ask Profile Inspector to export all customised profiles next to its executable.
#[tauri::command]
pub async fn ff_nv_npi_export(app: AppHandle) -> Res<nv::npi::NpiRun> {
    blocking(&app, move |f| {
        let exe = f.config().npi_exe.ok_or("Choose your NVIDIA Profile Inspector executable first")?;
        nv::npi::export_customized_with(&PathBuf::from(exe))
    })
    .await?
    .map_err(err)
}

/// Check and store the Profile Inspector path; `None` clears it.
#[tauri::command]
pub async fn ff_nv_set_npi(app: AppHandle, path: Option<String>) -> Res<ff::config::Config> {
    blocking(&app, move |f| {
        let mut c = f.config();
        c.npi_exe = match path {
            Some(p) => Some(nv::npi::validate_exe(&PathBuf::from(p)).map_err(|e| anyhow::anyhow!(e))?.to_string_lossy().into_owned()),
            None => None,
        };
        f.set_config(c)?;
        Ok::<_, anyhow::Error>(f.config())
    })
    .await?
    .map_err(err)
}

/// Load setting names from the Reference.xml next to the user's Profile Inspector.
#[tauri::command]
pub async fn ff_nv_load_reference(app: AppHandle) -> Res<usize> {
    blocking(&app, |f| f.nvidia_load_reference()).await?.map_err(err)
}

// ── GPU controls and Arcadia presets ────────────────────────────────────

use ff::presets::{Preset, PresetCheck, PresetOutcome, MAX_PRESET_BYTES};

#[tauri::command]
pub async fn ff_gpu_controls(app: AppHandle) -> Res<Vec<ff::service::GpuControlsView>> {
    blocking(&app, |f| f.gpu_controls()).await
}

#[tauri::command]
pub async fn ff_presets(app: AppHandle) -> Res<Vec<Preset>> {
    blocking(&app, |f| f.presets().list()).await
}

/// Capture the current driver settings of one GPU and save them as a preset.
#[tauri::command]
pub async fn ff_preset_capture(
    app: AppHandle,
    gpu: String,
    name: String,
    game_exe: Option<String>,
    notes: String,
) -> Res<Preset> {
    blocking(&app, move |f| {
        let preset = f.preset_capture(&gpu, &name, game_exe.as_deref(), &notes)?;
        f.presets().add(preset)
    })
    .await?
    .map_err(err)
}

#[tauri::command]
pub async fn ff_preset_check(app: AppHandle, id: String) -> Res<PresetCheck> {
    blocking(&app, move |f| f.presets().get(&id).map(|p| f.preset_check(&p)))
        .await?
        .map_err(err)
}

#[tauri::command]
pub async fn ff_preset_apply(app: AppHandle, id: String, level: u8) -> Res<PresetOutcome> {
    blocking(&app, move |f| f.preset_apply(&f.presets().get(&id)?, level))
        .await?
        .map_err(err)
}

#[tauri::command]
pub async fn ff_preset_import(app: AppHandle, path: String) -> Res<Preset> {
    blocking(&app, move |f| {
        let bytes = FrameForge::read_user_file(&PathBuf::from(path), MAX_PRESET_BYTES as u64)?;
        f.preset_import(&bytes)
    })
    .await?
    .map_err(err)
}

#[tauri::command]
pub async fn ff_preset_export(app: AppHandle, id: String, path: String) -> Res<()> {
    blocking(&app, move |f| f.preset_export(&id, &PathBuf::from(path)))
        .await?
        .map_err(err)
}

#[tauri::command]
pub async fn ff_preset_delete(app: AppHandle, id: String) -> Res<()> {
    blocking(&app, move |f| f.presets().delete(&id)).await?.map_err(err)
}

// ── Game profiles, launch lifecycle, benchmarks ─────────────────────────

use ff::benchmark::{BenchSession, Comparison};
use ff::profiles::{GameChoice, GameProfile, LifecycleEvent, ProfileRun, ProfileView, MAX_PROFILE_BYTES};

/// How often the launch watcher looks for games starting and exiting.
const WATCH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3);

/// Watch for games starting and exiting, for users who opted in to
/// automatic profiles or monitoring. Until they do (and until something
/// else has created FrameForge), each pass only reads the small config
/// file: no process listing, no driver library.
pub(crate) fn start_watcher(app: AppHandle) {
    let spawned = std::thread::Builder::new().name("frameforge-watcher".into()).spawn(move || loop {
        std::thread::sleep(WATCH_INTERVAL);
        let created = app.state::<AppState>().ff.get().is_some();
        if !created {
            let cfg = ff::config::Config::load();
            if !cfg.auto_profiles && !cfg.auto_monitor {
                continue;
            }
        }
        let events = forge(&app).lifecycle_tick();
        if !events.is_empty() {
            let _ = tauri::Emitter::emit(&app, "ff-lifecycle", &events);
        }
    });
    if let Err(e) = spawned {
        eprintln!("FrameForge launch watcher could not start: {e}");
    }
}

#[tauri::command]
pub async fn ff_profiles(app: AppHandle) -> Res<Vec<ProfileView>> {
    blocking(&app, |f| f.profile_views()).await
}

#[tauri::command]
pub async fn ff_profile_games(app: AppHandle) -> Res<Vec<GameChoice>> {
    blocking(&app, |f| f.profile_games()).await
}

/// Save a profile. `level` is the optimiser level the user confirmed for it.
#[tauri::command]
pub async fn ff_profile_save(app: AppHandle, profile: GameProfile, level: u8) -> Res<GameProfile> {
    blocking(&app, move |f| f.profile_save(profile, level)).await?.map_err(err)
}

#[tauri::command]
pub async fn ff_profile_delete(app: AppHandle, id: String) -> Res<()> {
    blocking(&app, move |f| f.profile_delete(&id)).await?.map_err(err)
}

#[tauri::command]
pub async fn ff_profile_duplicate(app: AppHandle, id: String) -> Res<GameProfile> {
    blocking(&app, move |f| f.profiles().duplicate(&id)).await?.map_err(err)
}

#[tauri::command]
pub async fn ff_profile_apply(app: AppHandle, id: String, level: u8) -> Res<ProfileRun> {
    blocking(&app, move |f| f.profile_apply(&id, level)).await?.map_err(err)
}

#[tauri::command]
pub async fn ff_profile_revert(app: AppHandle, id: String) -> Res<Vec<String>> {
    blocking(&app, move |f| f.profile_revert(&id)).await?.map_err(err)
}

#[tauri::command]
pub async fn ff_profile_export(app: AppHandle, id: String, path: String) -> Res<()> {
    blocking(&app, move |f| f.profile_export(&id, &PathBuf::from(path))).await?.map_err(err)
}

#[tauri::command]
pub async fn ff_profile_import(app: AppHandle, path: String) -> Res<GameProfile> {
    blocking(&app, move |f| {
        let bytes = FrameForge::read_user_file(&PathBuf::from(path), MAX_PROFILE_BYTES as u64)?;
        f.profile_import(&bytes)
    })
    .await?
    .map_err(err)
}

#[tauri::command]
pub async fn ff_profile_events(app: AppHandle) -> Res<Vec<LifecycleEvent>> {
    blocking(&app, |f| f.lifecycle_events()).await
}

#[tauri::command]
pub async fn ff_bench_start(app: AppHandle, label: String) -> Res<()> {
    blocking(&app, move |f| f.bench_start(&label)).await?.map_err(err)
}

#[tauri::command]
pub async fn ff_bench_stop(app: AppHandle) -> Res<BenchSession> {
    blocking(&app, |f| f.bench_stop()).await?.map_err(err)
}

#[tauri::command]
pub async fn ff_bench_list(app: AppHandle) -> Res<Vec<BenchSession>> {
    blocking(&app, |f| f.benchmarks().list()).await
}

#[tauri::command]
pub async fn ff_bench_rename(app: AppHandle, id: String, label: String) -> Res<BenchSession> {
    blocking(&app, move |f| f.benchmarks().rename(&id, &label)).await?.map_err(err)
}

#[tauri::command]
pub async fn ff_bench_delete(app: AppHandle, id: String) -> Res<()> {
    blocking(&app, move |f| f.benchmarks().delete(&id)).await?.map_err(err)
}

fn bench_sessions(f: &FrameForge, ids: &[String]) -> anyhow::Result<Vec<BenchSession>> {
    ids.iter().map(|id| f.benchmarks().get(id)).collect()
}

/// Compare sessions against the first id given.
#[tauri::command]
pub async fn ff_bench_compare(app: AppHandle, ids: Vec<String>) -> Res<Comparison> {
    blocking(&app, move |f| ff::benchmark::compare(&bench_sessions(&f, &ids)?)).await?.map_err(err)
}

/// Write a CSV or JSON report (by file extension) for the chosen sessions.
#[tauri::command]
pub async fn ff_bench_export(app: AppHandle, ids: Vec<String>, path: String) -> Res<()> {
    blocking(&app, move |f| ff::benchmark::export(&bench_sessions(&f, &ids)?, &PathBuf::from(path))).await?.map_err(err)
}
