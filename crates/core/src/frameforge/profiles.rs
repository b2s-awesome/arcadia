//! Per-game performance profiles and the launch lifecycle.
//!
//! A profile belongs to one game executable (by file name, so a game update
//! or a moved install folder does not orphan it) and lists what to do while
//! that game runs: optimiser actions, a process priority, an Arcadia GPU
//! preset, and whether to monitor or benchmark.
//!
//! Nothing here writes to the system itself. A profile is applied by
//! handing requests to the optimiser engine (recorded, verified,
//! revertible) and presets to the preset layer, so every safety rule those
//! enforce holds for profiles too.
//!
//! Automatic activation is opt-in twice: the global setting, and the
//! profile's own switch. An automatic run never goes above the level the
//! user approved when saving the profile, nor above the optimiser level.
//!
//! Profile files are untrusted input. They can name optimiser actions by id
//! and nothing else: no commands, no paths, no process ids.

use super::benchmark::{BenchSession, BenchStore, CaptureContext};
use super::monitor::State;
use super::optimizer::{ApplyRequest, ChangeStatus, SessionStatus};
use super::presets::{Preset, PresetOutcome};
use super::processes::{self, Category, ProcessInfo};
use super::service::FrameForge;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

pub const PROFILE_SCHEMA: u32 = 1;
pub const MAX_PROFILE_BYTES: usize = 2 * 1024 * 1024;
const MAX_ACTIONS: usize = 64;
const MAX_EVENTS: usize = 60;
const FILE_KIND: &str = "arcadia-game-profile";

/// One optimiser action a profile applies, named by its registry id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileAction {
    pub id: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameProfile {
    pub schema: u32,
    #[serde(default)]
    pub id: String,
    pub name: String,
    /// Library game this profile was made for, when there is one.
    #[serde(default)]
    pub game_id: Option<String>,
    #[serde(default)]
    pub game_name: String,
    /// Executable file name. This is what identifies the game when it runs.
    pub exe: String,
    #[serde(default)]
    pub enabled: bool,
    /// Apply when the game starts (also needs the global setting).
    #[serde(default)]
    pub auto_apply: bool,
    /// Priority class for the game while it runs. Never Realtime.
    #[serde(default)]
    pub priority: Option<String>,
    /// Arcadia GPU preset to apply.
    #[serde(default)]
    pub preset_id: Option<String>,
    #[serde(default)]
    pub actions: Vec<ProfileAction>,
    /// Start Live Monitor when the game starts.
    #[serde(default)]
    pub monitor: bool,
    /// Record a benchmark for the whole session while monitoring.
    #[serde(default)]
    pub benchmark: bool,
    /// Highest optimiser level the user approved for this profile. 0: not approved yet.
    #[serde(default)]
    pub approved_level: u8,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub updated_at: u64,
}

fn check_text(what: &str, text: &str, max: usize) -> Result<(), String> {
    if text.chars().count() > max || text.chars().any(|c| c.is_control() && c != '\n') {
        return Err(format!("The profile's {what} is too long or holds control characters"));
    }
    Ok(())
}

impl GameProfile {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema == 0 || self.schema > PROFILE_SCHEMA {
            return Err(format!("This profile uses schema {} and this version of Arcadia reads up to {PROFILE_SCHEMA}", self.schema));
        }
        if self.name.trim().is_empty() {
            return Err("The profile needs a name".into());
        }
        check_text("name", &self.name, 120)?;
        check_text("game name", &self.game_name, 200)?;
        check_text("notes", &self.notes, 4000)?;
        if let Some(g) = &self.game_id {
            check_text("game id", g, 200)?;
        }
        super::nvidia::nip::check_executable(&self.exe)?;
        if self.exe.contains(['\\', '/', ':']) || !self.exe.to_ascii_lowercase().ends_with(".exe") {
            return Err("The game executable must be a file name ending in .exe, not a path".into());
        }
        if let Some(reason) = processes::protection_reason(u32::MAX, &self.exe, None, None, 0) {
            return Err(format!("{} cannot have a profile: {reason}", self.exe));
        }
        if let Some(p) = &self.priority {
            processes::check_priority(p).map_err(|e| e.to_string())?;
            if !processes::ALLOWED_PRIORITIES.contains(&p.as_str()) {
                return Err(format!("'{}' is not a priority class", p.chars().take(30).collect::<String>()));
            }
        }
        if let Some(p) = &self.preset_id {
            if p.is_empty() || p.len() > 64 || !p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return Err("The profile refers to an invalid preset".into());
            }
        }
        if self.approved_level > 3 {
            return Err("The profile's approved level is out of range".into());
        }
        if self.actions.len() > MAX_ACTIONS {
            return Err(format!("A profile can hold at most {MAX_ACTIONS} actions"));
        }
        let mut seen = HashSet::new();
        for a in &self.actions {
            if a.id.is_empty() || a.id.len() > 80 || !a.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_')) {
                return Err("The profile names an invalid optimiser action".into());
            }
            // Process actions carry a process id and GPU controls a device id: neither
            // means anything in a stored profile. Priority and presets have their own fields.
            if a.id.starts_with("process.") || a.id == "gpu.control" {
                return Err(format!("'{}' cannot be part of a profile", a.id));
            }
            if serde_json::to_string(&a.params).map(|s| s.len()).unwrap_or(usize::MAX) > 2048 {
                return Err(format!("The parameters of '{}' are too large", a.id));
            }
            if !seen.insert((a.id.clone(), a.params.to_string())) {
                return Err(format!("'{}' is listed twice", a.id));
            }
        }
        Ok(())
    }

    /// Is this running process the profile's game? The file name must match;
    /// when both sides know which library game they are, those must agree,
    /// so two games that share an executable name are not confused. Windows,
    /// security and anti-cheat processes never match.
    pub fn matches(&self, p: &ProcessInfo) -> bool {
        p.category != Category::Protected
            && p.name.eq_ignore_ascii_case(&self.exe)
            && match (&self.game_id, &p.game_id) {
                (Some(a), Some(b)) => a == b,
                _ => true,
            }
    }
}

/// A profile file: the profile plus the preset it uses, so it is self-contained.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileFile {
    pub kind: String,
    pub schema: u32,
    pub profile: GameProfile,
    #[serde(default)]
    pub preset: Option<Preset>,
}

impl ProfileFile {
    pub fn parse(bytes: &[u8]) -> Result<ProfileFile, String> {
        if bytes.len() > MAX_PROFILE_BYTES {
            return Err("The file is too large to be an Arcadia game profile".into());
        }
        let text = std::str::from_utf8(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes)).map_err(|_| "The file is not UTF-8 text".to_string())?;
        let file: ProfileFile = serde_json::from_str(text).map_err(|e| format!("The file is not an Arcadia game profile: {e}"))?;
        if file.kind != FILE_KIND {
            return Err("The file is not an Arcadia game profile".into());
        }
        file.profile.validate()?;
        if let Some(p) = &file.preset {
            p.validate()?;
        }
        Ok(file)
    }
}

/// Profiles on disk: `FrameForge\profiles\<id>.json`.
pub struct ProfileStore {
    dir: PathBuf,
}

impl ProfileStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn path(&self, id: &str) -> Option<PathBuf> {
        (!id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')).then(|| self.dir.join(format!("{id}.json")))
    }

    pub fn list(&self) -> Vec<GameProfile> {
        let mut out: Vec<GameProfile> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
            .filter_map(|e| std::fs::read(e.path()).ok())
            .filter_map(|b| serde_json::from_slice::<GameProfile>(&b).ok())
            .filter(|p| !p.id.is_empty() && p.validate().is_ok())
            .collect();
        out.sort_by(|a, b| a.game_name.to_lowercase().cmp(&b.game_name.to_lowercase()).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())).then_with(|| a.id.cmp(&b.id)));
        out
    }

    pub fn get(&self, id: &str) -> anyhow::Result<GameProfile> {
        let path = self.path(id).ok_or_else(|| anyhow::anyhow!("That profile does not exist"))?;
        let bytes = std::fs::read(path).map_err(|_| anyhow::anyhow!("That profile does not exist"))?;
        let p: GameProfile = serde_json::from_slice(&bytes)?;
        p.validate().map_err(|e| anyhow::anyhow!(e))?;
        Ok(p)
    }

    /// Create (empty id) or update a profile.
    pub fn save(&self, mut p: GameProfile) -> anyhow::Result<GameProfile> {
        p.name = p.name.trim().to_string();
        p.exe = p.exe.trim().to_string();
        p.notes = p.notes.trim().to_string();
        p.schema = PROFILE_SCHEMA;
        p.validate().map_err(|e| anyhow::anyhow!(e))?;
        let now = super::hardware::now_secs();
        if p.id.is_empty() {
            p.id = uuid::Uuid::new_v4().simple().to_string();
            p.created_at = now;
        } else if let Ok(old) = self.get(&p.id) {
            p.created_at = old.created_at;
        }
        p.updated_at = now;
        let path = self.path(&p.id).ok_or_else(|| anyhow::anyhow!("Invalid profile id"))?;
        super::write_atomic(&path, &serde_json::to_vec_pretty(&p)?)?;
        Ok(p)
    }

    pub fn delete(&self, id: &str) -> anyhow::Result<()> {
        let path = self.path(id).ok_or_else(|| anyhow::anyhow!("That profile does not exist"))?;
        std::fs::remove_file(path).map_err(|_| anyhow::anyhow!("That profile does not exist"))
    }

    /// A copy under a new id. The copy does not activate by itself until the user says so.
    pub fn duplicate(&self, id: &str) -> anyhow::Result<GameProfile> {
        let mut p = self.get(id)?;
        p.id = String::new();
        p.name = format!("{} (copy)", p.name).chars().take(120).collect();
        p.auto_apply = false;
        self.save(p)
    }
}

/// What applying a profile did.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRun {
    pub profile_id: String,
    pub profile_name: String,
    /// The game process the run is tied to, when the game was running.
    pub pid: Option<u32>,
    pub applied: Vec<String>,
    /// What was left out, each with its reason.
    pub skipped: Vec<String>,
    pub session_id: Option<String>,
    pub preset: Option<PresetOutcome>,
    pub monitoring: bool,
}

#[derive(Debug, Clone)]
pub(super) struct ActiveRun {
    profile_id: String,
    profile_name: String,
    exe: String,
    pid: Option<u32>,
    session_id: Option<String>,
    preset_session_id: Option<String>,
    preset_backup_id: Option<String>,
    started_monitor: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleEvent {
    /// Unix seconds.
    pub at: u64,
    /// "applied", "exited", "monitor", "skipped" or "error".
    pub kind: String,
    pub profile: Option<String>,
    pub process: String,
    pub pid: Option<u32>,
    pub text: String,
}

#[derive(Default)]
pub(super) struct RunState {
    active: Vec<ActiveRun>,
    events: VecDeque<LifecycleEvent>,
    /// `(profile id, pid)` pairs the watcher already handled, so a profile the
    /// user reverted by hand is not re-applied to the same running game.
    auto_seen: HashSet<(String, u32)>,
    /// Processes the watcher already tried to monitor.
    monitor_seen: HashSet<u32>,
    /// Monitoring the watcher started by itself (no profile involved).
    auto_monitor_pid: Option<u32>,
}

/// A profile with its live state, for the list.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileView {
    #[serde(flatten)]
    pub profile: GameProfile,
    /// The game is running now (its process id).
    pub running_pid: Option<u32>,
    /// Arcadia applied this profile and has not reverted it.
    pub applied: bool,
    pub preset_name: Option<String>,
    /// The preset the profile points at no longer exists.
    pub preset_missing: bool,
    pub benchmarks: usize,
}

/// A game a profile can be made for.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameChoice {
    pub game_id: Option<String>,
    pub name: String,
    /// Known from the library, or from the game's running process.
    pub exe: Option<String>,
    pub running_pid: Option<u32>,
}

impl FrameForge {
    pub fn profiles(&self) -> ProfileStore {
        ProfileStore::new(super::profiles_dir())
    }

    pub fn benchmarks(&self) -> BenchStore {
        BenchStore::new(super::benchmarks_dir())
    }

    fn event(&self, kind: &str, profile: Option<&str>, process: &str, pid: Option<u32>, text: impl Into<String>) -> LifecycleEvent {
        let e = LifecycleEvent { at: super::hardware::now_secs(), kind: kind.into(), profile: profile.map(String::from), process: process.into(), pid, text: text.into() };
        let mut runs = self.runs.lock();
        runs.events.push_front(e.clone());
        runs.events.truncate(MAX_EVENTS);
        e
    }

    /// Recent launch-lifecycle activity, newest first.
    pub fn lifecycle_events(&self) -> Vec<LifecycleEvent> {
        self.runs.lock().events.iter().cloned().collect()
    }

    pub fn profile_views(&self) -> Vec<ProfileView> {
        let processes = self.processes();
        let presets = self.presets().list();
        let benches = self.benchmarks().list();
        let active: Vec<String> = self.runs.lock().active.iter().map(|r| r.profile_id.clone()).collect();
        self.profiles()
            .list()
            .into_iter()
            .map(|profile| {
                let preset = profile.preset_id.as_ref().and_then(|id| presets.iter().find(|p| &p.id == id));
                ProfileView {
                    running_pid: processes.iter().find(|p| profile.matches(p)).map(|p| p.pid),
                    applied: active.contains(&profile.id),
                    preset_name: preset.map(|p| p.name.clone()),
                    preset_missing: profile.preset_id.is_some() && preset.is_none(),
                    benchmarks: benches.iter().filter(|b| b.exe.eq_ignore_ascii_case(&profile.exe)).count(),
                    profile,
                }
            })
            .collect()
    }

    /// Library games (and running games the library matched) to make a profile for.
    pub fn profile_games(&self) -> Vec<GameChoice> {
        let processes = self.processes();
        let mut out: Vec<GameChoice> = self
            .game_index()
            .games()
            .into_iter()
            .map(|(id, name, exe)| {
                let running = processes.iter().find(|p| p.category == Category::Game && p.game_id.as_deref() == Some(id.as_str()));
                GameChoice { exe: exe.or_else(|| running.map(|p| p.name.clone())), running_pid: running.map(|p| p.pid), game_id: Some(id), name }
            })
            .collect();
        out.sort_by(|a, b| b.running_pid.is_some().cmp(&a.running_pid.is_some()).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        out
    }

    /// Save a profile the user edited. `approved_level` is the level they
    /// confirmed for it; automatic runs never go above it.
    pub fn profile_save(&self, mut profile: GameProfile, approved_level: u8) -> anyhow::Result<GameProfile> {
        profile.approved_level = approved_level.min(3);
        if let Some(id) = &profile.preset_id {
            self.presets().get(id).map_err(|_| anyhow::anyhow!("The preset this profile uses no longer exists"))?;
        }
        self.profiles().save(profile)
    }

    pub fn profile_delete(&self, id: &str) -> anyhow::Result<()> {
        if self.runs.lock().active.iter().any(|r| r.profile_id == id) {
            anyhow::bail!("This profile is applied right now. Revert it first, then delete it.");
        }
        self.profiles().delete(id)
    }

    pub fn profile_export(&self, id: &str, path: &Path) -> anyhow::Result<()> {
        if !path.is_absolute() || path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("json")) != Some(true) {
            anyhow::bail!("Choose a full path ending in .json");
        }
        let profile = self.profiles().get(id)?;
        let preset = profile.preset_id.as_ref().and_then(|p| self.presets().get(p).ok());
        let file = ProfileFile { kind: FILE_KIND.into(), schema: PROFILE_SCHEMA, profile, preset };
        super::write_atomic(path, &serde_json::to_vec_pretty(&file)?)
    }

    /// Import a profile file as a new profile. It arrives switched off and
    /// unapproved: nothing in it runs until the user reviews and saves it.
    pub fn profile_import(&self, bytes: &[u8]) -> anyhow::Result<GameProfile> {
        let file = ProfileFile::parse(bytes).map_err(|e| anyhow::anyhow!(e))?;
        let mut profile = file.profile;
        profile.id = String::new();
        profile.enabled = false;
        profile.auto_apply = false;
        profile.approved_level = 0;
        profile.preset_id = match file.preset {
            Some(p) => Some(self.presets().add(p)?.id),
            None => None,
        };
        self.profiles().save(profile)
    }

    /// Apply a profile now. `approved_level` is the level the user confirmed.
    pub fn profile_apply(&self, id: &str, approved_level: u8) -> anyhow::Result<ProfileRun> {
        let profile = self.profiles().get(id)?;
        if self.runs.lock().active.iter().any(|r| r.profile_id == id) {
            anyhow::bail!("This profile is already applied. Revert it before applying it again.");
        }
        let processes = self.processes();
        let target = processes.iter().find(|p| profile.matches(p)).cloned();
        let run = self.apply_profile(&profile, target.as_ref(), approved_level.clamp(1, 3))?;
        if profile.monitor {
            if let Some(p) = &target {
                self.start_profile_monitor(&profile, p);
            }
        }
        Ok(ProfileRun { monitoring: self.runs.lock().active.iter().any(|r| r.profile_id == id && r.started_monitor), ..run })
    }

    fn apply_profile(&self, profile: &GameProfile, target: Option<&ProcessInfo>, level: u8) -> anyhow::Result<ProfileRun> {
        let mut skipped = Vec::new();
        let mut requests = Vec::new();
        let views = self.optimizer_list();
        for a in &profile.actions {
            let wanted = if a.params.is_null() { Value::Null } else { a.params.clone() };
            match views.iter().find(|v| v.meta.id == a.id && v.params == wanted) {
                None => skipped.push(format!("{}: not available on this PC", a.id)),
                Some(v) if v.state == "available" && v.meta.level > level => skipped.push(format!("{}: needs the {} level", v.label.clone().unwrap_or_else(|| v.meta.name.clone()), ["", "Safe", "Advanced", "Expert"][v.meta.level.min(3) as usize])),
                Some(v) if v.state == "available" => requests.push(ApplyRequest { id: a.id.clone(), params: a.params.clone() }),
                Some(v) if v.state == "notApplicable" => skipped.push(format!("{}: {}", v.meta.name, v.reason.clone().unwrap_or_else(|| "not applicable".into()))),
                // Already set, or applied by an earlier session: nothing to do.
                Some(_) => {}
            }
        }
        if let Some(class) = &profile.priority {
            match target {
                None => skipped.push("Priority: the game is not running, so there is no process to change".into()),
                Some(_) if level < 2 => skipped.push("Priority: needs the Advanced level".into()),
                Some(p) => requests.push(ApplyRequest { id: "process.priority".into(), params: json!({ "pid": p.pid, "class": class }) }),
            }
        }
        let mut applied = Vec::new();
        let mut session_id = None;
        if !requests.is_empty() {
            let session = self.optimizer_apply(&requests, level, &format!("Game profile: {}", profile.name))?;
            if session.status != SessionStatus::Applied {
                anyhow::bail!("{}", session.error.unwrap_or_else(|| "The profile could not be applied. Anything already changed was put back.".into()));
            }
            applied.extend(session.changes.iter().filter(|c| c.status == ChangeStatus::Applied).map(|c| c.name.clone()));
            session_id = Some(session.id);
        }
        let mut preset_out = None;
        if let Some(pid) = &profile.preset_id {
            match self.presets().get(pid) {
                Err(_) => skipped.push("GPU preset: it no longer exists".into()),
                Ok(preset) => {
                    let check = self.preset_check(&preset);
                    if !check.usable {
                        if let Some(p) = check.problems.first() {
                            skipped.push(format!("GPU preset \"{}\": {p}", preset.name));
                        }
                    } else if check.level > level {
                        skipped.push(format!("GPU preset \"{}\": needs the {} level", preset.name, ["", "Safe", "Advanced", "Expert"][check.level.min(3) as usize]));
                    } else {
                        match self.preset_apply(&preset, level) {
                            Ok(out) => {
                                applied.push(format!("GPU preset \"{}\" ({} setting(s))", preset.name, out.applied));
                                preset_out = Some(out);
                            }
                            Err(e) => skipped.push(format!("GPU preset \"{}\": {e}", preset.name)),
                        }
                    }
                }
            }
        }
        if session_id.is_some() || preset_out.is_some() {
            self.runs.lock().active.push(ActiveRun {
                profile_id: profile.id.clone(),
                profile_name: profile.name.clone(),
                exe: profile.exe.clone(),
                pid: target.map(|p| p.pid),
                session_id: session_id.clone(),
                preset_session_id: preset_out.as_ref().and_then(|o| o.session_id.clone()),
                preset_backup_id: preset_out.as_ref().and_then(|o| o.backup_id.clone()),
                started_monitor: false,
            });
        }
        Ok(ProfileRun { profile_id: profile.id.clone(), profile_name: profile.name.clone(), pid: target.map(|p| p.pid), applied, skipped, session_id, preset: preset_out, monitoring: false })
    }

    /// Start Live Monitor for a profile's game unless something is already being monitored.
    fn start_profile_monitor(&self, profile: &GameProfile, p: &ProcessInfo) -> Vec<LifecycleEvent> {
        let mut events = Vec::new();
        if self.monitor_snapshot().state == State::Running {
            return events;
        }
        match self.monitor_start(p.pid, p.name.clone()) {
            Ok(()) => {
                {
                    let mut state = self.runs.lock();
                    match state.active.iter_mut().find(|r| r.profile_id == profile.id && r.pid == Some(p.pid)) {
                        Some(r) => r.started_monitor = true,
                        None => state.auto_monitor_pid = Some(p.pid),
                    }
                }
                events.push(self.event("monitor", Some(&profile.name), &p.name, Some(p.pid), "Live Monitor started"));
                if profile.benchmark {
                    if let Err(e) = self.bench_start(&profile.name) {
                        events.push(self.event("error", Some(&profile.name), &p.name, Some(p.pid), format!("The benchmark recording could not start: {e}")));
                    }
                }
            }
            Err(e) => events.push(self.event("error", Some(&profile.name), &p.name, Some(p.pid), format!("Live Monitor could not start: {e}"))),
        }
        events
    }

    /// Undo everything a profile applied, driver settings included.
    pub fn profile_revert(&self, id: &str) -> anyhow::Result<Vec<String>> {
        let runs: Vec<ActiveRun> = {
            let mut state = self.runs.lock();
            let (mine, rest) = std::mem::take(&mut state.active).into_iter().partition(|r| r.profile_id == id);
            state.active = rest;
            mine
        };
        if runs.is_empty() {
            anyhow::bail!("Arcadia has nothing applied for this profile right now");
        }
        let mut notes = Vec::new();
        for run in runs {
            notes.extend(self.end_run(&run, true, true));
        }
        Ok(notes)
    }

    /// Close one run. Temporary process changes always go back. System
    /// changes go back when `restore_system`. Driver settings from a preset
    /// are intentional and persistent: they only go back when `restore_driver`
    /// (the user chose Revert).
    fn end_run(&self, run: &ActiveRun, restore_system: bool, restore_driver: bool) -> Vec<String> {
        let mut notes = Vec::new();
        if run.started_monitor && self.monitor_snapshot().pid == run.pid {
            self.monitor_stop();
            notes.push("Live Monitor stopped".to_string());
        }
        self.settle_exited_processes();
        if let Some(sid) = &run.session_id {
            let still_applied = self.engine().history().into_iter().find(|s| &s.id == sid).map(|s| s.changes.into_iter().filter(|c| c.status == ChangeStatus::Applied && c.reversible).collect::<Vec<_>>()).unwrap_or_default();
            for c in still_applied {
                if !(c.temporary || restore_system) {
                    continue;
                }
                match self.optimizer_revert_change(sid, c.id) {
                    Ok(_) => notes.push(format!("Restored: {}", c.name)),
                    Err(e) => notes.push(format!("Could not restore \"{}\": {e}", c.name)),
                }
            }
        }
        if restore_driver {
            if let Some(sid) = &run.preset_session_id {
                match self.optimizer_revert_session(sid) {
                    Ok(_) => notes.push("Restored the GPU controls the preset changed".to_string()),
                    Err(e) => notes.push(format!("Could not restore the GPU controls: {e}")),
                }
            }
            if let Some(bid) = &run.preset_backup_id {
                match self.nvidia().restore_backup(bid) {
                    Ok(_) => notes.push("Restored the NVIDIA driver profile from its snapshot".to_string()),
                    Err(e) => notes.push(format!("Could not restore the NVIDIA driver profile: {e}")),
                }
            }
        }
        notes
    }

    /// One pass of the launch watcher, with the system's process list.
    pub fn lifecycle_tick(&self) -> Vec<LifecycleEvent> {
        let cfg = self.config();
        let idle = {
            let s = self.runs.lock();
            s.active.is_empty() && s.auto_monitor_pid.is_none()
        };
        if idle && !cfg.auto_profiles && !cfg.auto_monitor {
            return Vec::new(); // Not opted in and nothing to clean up: do not even list processes.
        }
        self.lifecycle_tick_with(&self.processes())
    }

    /// One pass of the launch watcher: close runs whose game exited, then
    /// (when the user opted in) apply profiles to games that just started.
    pub fn lifecycle_tick_with(&self, processes: &[ProcessInfo]) -> Vec<LifecycleEvent> {
        let cfg = self.config();
        let mut events = Vec::new();
        let alive = |pid: u32| processes.iter().any(|p| p.pid == pid);

        // 1. Games that exited.
        let ended: Vec<ActiveRun> = {
            let mut s = self.runs.lock();
            let (gone, rest) = std::mem::take(&mut s.active).into_iter().partition(|r| r.pid.map(|p| !alive(p)).unwrap_or(false));
            s.active = rest;
            s.auto_seen.retain(|(_, pid)| alive(*pid));
            s.monitor_seen.retain(|pid| alive(*pid));
            gone
        };
        for run in ended {
            let notes = self.end_run(&run, cfg.restore_on_exit, false);
            let text = if notes.is_empty() { "The game exited".to_string() } else { format!("The game exited. {}", notes.join(". ")) };
            events.push(self.event("exited", Some(&run.profile_name), &run.exe, run.pid, text));
        }
        let orphan_monitor = self.runs.lock().auto_monitor_pid.filter(|pid| !alive(*pid));
        if let Some(pid) = orphan_monitor {
            self.runs.lock().auto_monitor_pid = None;
            if self.monitor_snapshot().pid == Some(pid) {
                let name = self.monitor_snapshot().process;
                self.monitor_stop();
                events.push(self.event("exited", None, &name, Some(pid), "The game exited. Live Monitor stopped"));
            }
        }

        // 2. Games that started, for profiles the user switched on.
        let profiles = if cfg.auto_profiles { self.profiles().list() } else { Vec::new() };
        for profile in profiles.iter().filter(|p| p.enabled && p.auto_apply) {
            let Some(p) = processes.iter().find(|p| profile.matches(p)) else { continue };
            {
                let mut s = self.runs.lock();
                if s.active.iter().any(|r| r.profile_id == profile.id) || !s.auto_seen.insert((profile.id.clone(), p.pid)) {
                    continue;
                }
            }
            let level = profile.approved_level.min(cfg.optimizer_level.clamp(1, 3));
            if level == 0 {
                events.push(self.event("skipped", Some(&profile.name), &p.name, Some(p.pid), "Not applied: this profile has not been reviewed yet. Open it and save it to approve its changes."));
                continue;
            }
            match self.apply_profile(profile, Some(p), level) {
                Ok(run) => {
                    let mut text = if run.applied.is_empty() { "Nothing needed changing".to_string() } else { format!("Applied: {}", run.applied.join(", ")) };
                    if !run.skipped.is_empty() {
                        text.push_str(&format!(". Left out: {}", run.skipped.join("; ")));
                    }
                    events.push(self.event("applied", Some(&profile.name), &p.name, Some(p.pid), text));
                }
                Err(e) => events.push(self.event("error", Some(&profile.name), &p.name, Some(p.pid), format!("The profile could not be applied: {e}"))),
            }
            if profile.monitor {
                self.runs.lock().monitor_seen.insert(p.pid);
                events.extend(self.start_profile_monitor(profile, p));
            }
        }

        // 3. Monitoring for any library game, when the user asked for that.
        if cfg.auto_monitor && self.monitor_snapshot().state != State::Running {
            if let Some(p) = processes::pick_running_game(processes) {
                if self.runs.lock().monitor_seen.insert(p.pid) {
                    match self.monitor_start(p.pid, p.name.clone()) {
                        Ok(()) => {
                            self.runs.lock().auto_monitor_pid = Some(p.pid);
                            events.push(self.event("monitor", None, &p.name, Some(p.pid), "Live Monitor started"));
                        }
                        Err(e) => events.push(self.event("error", None, &p.name, Some(p.pid), format!("Live Monitor could not start: {e}"))),
                    }
                }
            }
        }
        events
    }

    // ── benchmarks ──────────────────────────────────────────────────────

    /// Start recording the monitored game for a benchmark.
    pub fn bench_start(&self, label: &str) -> anyhow::Result<()> {
        let label = label.trim();
        if label.chars().count() > 120 || label.chars().any(|c| c.is_control()) {
            anyhow::bail!("The name can be at most 120 characters");
        }
        let snap = self.monitor_snapshot();
        let pid = match (snap.state, snap.pid) {
            (State::Running, Some(pid)) => pid,
            _ => anyhow::bail!("Start Live Monitor on the game first: a benchmark records the frames it measures"),
        };
        if snap.recording {
            anyhow::bail!("A benchmark is already being recorded");
        }
        let inv = self.inventory(false);
        let process = self.processes().into_iter().find(|p| p.pid == pid);
        let display = inv.displays.iter().find(|d| d.primary).or(inv.displays.first());
        let gpu = inv.primary_gpu();
        let profile = self.runs.lock().active.iter().find(|r| r.pid == Some(pid)).map(|r| r.profile_name.clone());
        let ctx = CaptureContext {
            label: label.to_string(),
            game_id: process.as_ref().and_then(|p| p.game_id.clone()),
            game_name: process.as_ref().and_then(|p| p.game_name.clone()).unwrap_or_else(|| snap.process.clone()),
            exe: snap.process.clone(),
            pid,
            source: match snap.source {
                Some(super::presentmon::SourceKind::Service) => "PresentMon service",
                Some(super::presentmon::SourceKind::Cli) => "PresentMon console application",
                Some(super::presentmon::SourceKind::Replay) => "Recorded frames",
                None => "unknown",
            }
            .to_string(),
            width: display.map(|d| d.width),
            height: display.map(|d| d.height),
            refresh_hz: display.and_then(|d| d.refresh_hz),
            cpu: inv.cpu.model.clone(),
            gpu: gpu.map(|g| g.name.clone()),
            gpu_id: gpu.map(|g| g.id.clone()),
            gpu_driver: gpu.and_then(|g| g.driver_version_vendor.clone().or(g.driver_version.clone())),
            os: format!("{} {}", inv.system.os_name, inv.system.os_version).trim().to_string(),
            active_optimizations: self.engine().active_changes().into_iter().map(|(_, c)| c.name).collect(),
            profile,
        };
        let started = self.with_monitor(|m| m.start_recording()).is_some();
        if !started {
            anyhow::bail!("Live Monitor stopped before the recording could start");
        }
        *self.bench.lock() = Some(ctx);
        Ok(())
    }

    /// Stop recording, summarise what was captured and add it to the history.
    pub fn bench_stop(&self) -> anyhow::Result<BenchSession> {
        let rec = self.with_monitor(|m| m.stop_recording()).flatten();
        let ctx = self.bench.lock().take();
        let (Some(rec), Some(ctx)) = (rec, ctx) else { anyhow::bail!("No benchmark is being recorded") };
        let session = super::benchmark::summarise(&rec, &ctx)?;
        self.benchmarks().save(session)
    }

    /// Save a recording that is still open when monitoring ends. A recording
    /// with no frames is dropped: there is nothing to keep.
    pub(super) fn bench_finish_quietly(&self) {
        if self.bench.lock().is_some() {
            let _ = self.bench_stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frameforge::gpu::mock::{MockBase, MockVendor};
    use crate::frameforge::gpu::{ControlKind, GpuControl, GpuDevice, GpuRegistry, Risk, Vendor};
    use crate::frameforge::optimizer::ops::{MockOps, MockProcs};
    use crate::frameforge::presentmon::{Frame, ReplaySource};
    use crate::frameforge::presets::{PresetSetting, PresetValue, PRESET_SCHEMA};
    use std::sync::Arc;

    const GAME_PID: u32 = 4242;

    struct World {
        ff: FrameForge,
        ops: Arc<MockOps>,
        procs: Arc<MockProcs>,
        amd: Arc<MockVendor>,
        dir: tempfile::TempDir,
        _home: (std::sync::MutexGuard<'static, ()>, tempfile::TempDir),
    }

    fn proc(pid: u32, name: &str, game: Option<&str>) -> ProcessInfo {
        ProcessInfo {
            pid,
            name: name.into(),
            exe: Some(format!("D:\\Games\\{name}")),
            cpu_pct: 10.0,
            memory_bytes: 1 << 30,
            gpu_pct: Some(90.0),
            owner: Some("you".into()),
            category: if game.is_some() { Category::Game } else { Category::App },
            game_id: game.map(String::from),
            game_name: game.map(|_| "Test Game".into()),
            relevance: String::new(),
            closable: game.is_none(),
            protected_reason: None,
        }
    }

    fn world() -> World {
        let home = crate::minecraft::test_home();
        let dir = tempfile::tempdir().unwrap();
        let mut amd = MockVendor::available(Vendor::Amd);
        amd.controls = vec![GpuControl { id: "adlx.antilag".into(), vendor: Vendor::Amd, name: "Anti-Lag".into(), description: String::new(), category: "3D".into(), kind: ControlKind::Toggle, value: Some(0.0), default: None, supported: true, unsupported_reason: None, risk: Risk::Low, level: 2, requires_restart: false }];
        let amd = Arc::new(amd);
        let gpu = GpuDevice { id: "amd0".into(), vendor: Vendor::Amd, name: "Radeon".into(), ..Default::default() };
        let registry = GpuRegistry::new(Arc::new(MockBase::new(vec![gpu])), vec![amd.clone()]);
        let ops = Arc::new(MockOps::typical());
        let procs = Arc::new(MockProcs::with(&[GAME_PID]));
        let ff = FrameForge::with_parts(registry, ops.clone(), procs.clone());
        ff.set_process_list(Some(vec![proc(GAME_PID, "game.exe", Some("steam:1")), proc(7, "notepad.exe", None)]));
        let mut cfg = ff.config();
        cfg.optimizer_level = 2;
        ff.set_config(cfg).unwrap();
        World { ff, ops, procs, amd, dir, _home: home }
    }

    fn profile() -> GameProfile {
        GameProfile {
            schema: PROFILE_SCHEMA,
            id: String::new(),
            name: "Competitive".into(),
            game_id: Some("steam:1".into()),
            game_name: "Test Game".into(),
            exe: "Game.exe".into(),
            enabled: true,
            auto_apply: true,
            priority: Some("High".into()),
            preset_id: None,
            actions: vec![ProfileAction { id: "power.plan.high_performance".into(), params: Value::Null }],
            monitor: false,
            benchmark: false,
            approved_level: 0,
            notes: String::new(),
            created_at: 0,
            updated_at: 0,
        }
    }

    fn active_plan(w: &World) -> String {
        w.ops.power_plans().into_iter().find(|p| p.active).map(|p| p.name).unwrap_or_default()
    }
    use crate::frameforge::optimizer::ops::SystemOps;
    use crate::frameforge::processes::ProcessOps;

    #[test]
    fn create_edit_duplicate_delete() {
        let w = world();
        let p = w.ff.profile_save(profile(), 2).unwrap();
        assert!(!p.id.is_empty() && p.approved_level == 2 && p.created_at > 0);
        let mut edit = p.clone();
        edit.name = "  Ranked  ".into();
        edit.priority = Some("AboveNormal".into());
        let edit = w.ff.profile_save(edit, 2).unwrap();
        assert_eq!((edit.id.as_str(), edit.name.as_str(), edit.created_at), (p.id.as_str(), "Ranked", p.created_at));
        let copy = w.ff.profiles().duplicate(&p.id).unwrap();
        assert_ne!(copy.id, p.id);
        assert_eq!(copy.name, "Ranked (copy)");
        assert!(!copy.auto_apply, "a copy does not activate by itself");
        let views = w.ff.profile_views();
        assert_eq!(views.len(), 2);
        assert!(views.iter().all(|v| v.running_pid == Some(GAME_PID) && !v.applied));
        w.ff.profile_delete(&copy.id).unwrap();
        assert_eq!(w.ff.profiles().list().len(), 1);
        assert!(w.ff.profiles().get("../x").is_err());
        // A preset that does not exist cannot be attached.
        let mut bad = profile();
        bad.preset_id = Some("missing".into());
        assert!(w.ff.profile_save(bad, 2).is_err());
    }

    #[test]
    fn validation_rejects_dangerous_or_malformed_profiles() {
        let ok = profile();
        assert!(ok.validate().is_ok());
        let with = |f: &dyn Fn(&mut GameProfile)| {
            let mut p = profile();
            f(&mut p);
            p.validate()
        };
        assert!(with(&|p| p.priority = Some("Realtime".into())).unwrap_err().contains("Realtime"));
        assert!(with(&|p| p.priority = Some("Ludicrous".into())).is_err());
        assert!(with(&|p| p.exe = "C:\\Games\\game.exe".into()).is_err());
        assert!(with(&|p| p.exe = "..\\game.exe".into()).is_err());
        assert!(with(&|p| p.exe = "game.bat".into()).is_err());
        for protected in ["csrss.exe", "MsMpEng.exe", "vgc.exe", "EasyAntiCheat.exe", "explorer.exe"] {
            assert!(with(&|p| p.exe = protected.into()).is_err(), "{protected} must never get a profile");
        }
        assert!(with(&|p| p.actions.push(ProfileAction { id: "process.end".into(), params: json!({ "pid": 4 }) })).is_err());
        assert!(with(&|p| p.actions.push(ProfileAction { id: "gpu.control".into(), params: Value::Null })).is_err());
        assert!(with(&|p| p.actions.push(ProfileAction { id: "cmd /c del".into(), params: Value::Null })).is_err());
        assert!(with(&|p| p.actions.push(p.actions[0].clone())).unwrap_err().contains("twice"));
        assert!(with(&|p| p.name = " ".into()).is_err());
        assert!(with(&|p| p.schema = 9).is_err());
        assert!(with(&|p| p.approved_level = 4).is_err());
        assert!(with(&|p| p.notes = "x".repeat(4001)).is_err());
    }

    #[test]
    fn matching_uses_the_executable_and_never_unrelated_processes() {
        let p = profile();
        assert!(p.matches(&proc(1, "GAME.EXE", Some("steam:1"))), "case-insensitive, any folder");
        assert!(p.matches(&proc(1, "game.exe", None)), "a moved install the library has not matched yet");
        assert!(!p.matches(&proc(1, "game.exe", Some("epic:other"))), "same file name, different library game");
        assert!(!p.matches(&proc(1, "game2.exe", Some("steam:1"))));
        assert!(!p.matches(&proc(1, "notepad.exe", None)));
        let mut protected = proc(1, "game.exe", None);
        protected.category = Category::Protected;
        assert!(!p.matches(&protected));
    }

    #[test]
    fn apply_and_revert_by_hand() {
        let w = world();
        let before = active_plan(&w);
        let p = w.ff.profile_save(profile(), 2).unwrap();
        // Approved for Safe only: the Advanced priority change is left out and said so.
        let run = w.ff.profile_apply(&p.id, 1).unwrap();
        assert_eq!(run.pid, Some(GAME_PID));
        assert_eq!(run.applied.len(), 1);
        assert!(run.skipped.iter().any(|s| s.contains("Priority") && s.contains("Advanced")), "{:?}", run.skipped);
        assert_eq!(w.procs.priority(GAME_PID).unwrap(), "Normal");
        assert_ne!(active_plan(&w), before);
        assert!(w.ff.profile_views()[0].applied);
        assert!(w.ff.profile_apply(&p.id, 2).unwrap_err().to_string().contains("already applied"));
        assert!(w.ff.profile_delete(&p.id).unwrap_err().to_string().contains("Revert it first"));
        let notes = w.ff.profile_revert(&p.id).unwrap();
        assert!(notes.iter().any(|n| n.starts_with("Restored:")), "{notes:?}");
        assert_eq!(active_plan(&w), before);
        assert!(w.ff.profile_revert(&p.id).is_err());

        // With Advanced approved the priority goes through the optimiser too.
        let run = w.ff.profile_apply(&p.id, 2).unwrap();
        assert_eq!(run.applied.len(), 2);
        assert_eq!(w.procs.priority(GAME_PID).unwrap(), "High");
        w.ff.profile_revert(&p.id).unwrap();
        assert_eq!(w.procs.priority(GAME_PID).unwrap(), "Normal");
        assert_eq!(active_plan(&w), before);
    }

    #[test]
    fn automatic_activation_is_opt_in_and_restores_on_exit() {
        let w = world();
        let before = active_plan(&w);
        let p = w.ff.profile_save(profile(), 2).unwrap();
        let list = vec![proc(GAME_PID, "game.exe", Some("steam:1")), proc(7, "notepad.exe", None)];

        // Not opted in: nothing happens, however many times the watcher runs.
        assert!(w.ff.lifecycle_tick_with(&list).is_empty());
        assert!(w.ff.lifecycle_tick().is_empty());
        assert_eq!(active_plan(&w), before);

        let mut cfg = w.ff.config();
        cfg.auto_profiles = true;
        cfg.restore_on_exit = true;
        w.ff.set_config(cfg).unwrap();
        let events = w.ff.lifecycle_tick_with(&list);
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!((events[0].kind.as_str(), events[0].pid), ("applied", Some(GAME_PID)));
        assert_eq!(w.procs.priority(GAME_PID).unwrap(), "High");
        assert_eq!(w.procs.priority(7).ok(), None, "the unrelated process was not touched");
        assert_ne!(active_plan(&w), before);
        // Applied once, not on every pass.
        assert!(w.ff.lifecycle_tick_with(&list).is_empty());

        // The game exits: the system change is restored and the run is closed.
        w.procs.running.lock().remove(&GAME_PID);
        w.ff.set_process_list(Some(vec![proc(7, "notepad.exe", None)]));
        let events = w.ff.lifecycle_tick();
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(events[0].kind, "exited");
        assert!(events[0].text.contains("Restored"), "{}", events[0].text);
        assert_eq!(active_plan(&w), before);
        assert!(!w.ff.profile_views()[0].applied);
        assert!(w.ff.engine().active_changes().is_empty(), "nothing is left applied");
        assert!(w.ff.lifecycle_events().len() >= 2);
        let _ = p;
    }

    #[test]
    fn automatic_runs_respect_approval_and_the_exit_setting() {
        let w = world();
        let before = active_plan(&w);
        let mut cfg = w.ff.config();
        cfg.auto_profiles = true;
        cfg.restore_on_exit = false;
        w.ff.set_config(cfg).unwrap();
        let list = vec![proc(GAME_PID, "game.exe", Some("steam:1"))];

        // An imported or never-reviewed profile (level 0) is not applied.
        let p = w.ff.profile_save(profile(), 0).unwrap();
        let events = w.ff.lifecycle_tick_with(&list);
        assert_eq!(events[0].kind, "skipped");
        assert_eq!(active_plan(&w), before);

        // Approved for Safe only: the plan changes, the priority does not.
        w.ff.set_process_list(Some(Vec::new()));
        w.ff.lifecycle_tick();
        w.ff.set_process_list(Some(list.clone()));
        w.ff.profile_save(p.clone(), 1).unwrap();
        let events = w.ff.lifecycle_tick();
        assert_eq!(events[0].kind, "applied");
        assert!(events[0].text.contains("Left out"), "{}", events[0].text);
        assert_eq!(w.procs.priority(GAME_PID).unwrap(), "Normal");
        let changed = active_plan(&w);
        assert_ne!(changed, before);

        // restore_on_exit is off: the system change stays (and is still revertible from History).
        w.ff.set_process_list(Some(Vec::new()));
        let events = w.ff.lifecycle_tick();
        assert_eq!(events[0].kind, "exited");
        assert_eq!(active_plan(&w), changed);
        assert_eq!(w.ff.engine().active_changes().len(), 1);

        // A switched-off profile, or one without auto-apply, is ignored.
        let mut off = w.ff.profiles().get(&p.id).unwrap();
        off.auto_apply = false;
        w.ff.profile_save(off, 2).unwrap();
        assert!(w.ff.lifecycle_tick_with(&list).is_empty());
    }

    #[test]
    fn preset_is_applied_with_the_profile_and_kept_on_exit() {
        let w = world();
        let preset = w.ff.presets().add(Preset { schema: PRESET_SCHEMA, id: String::new(), name: "Low latency".into(), vendor: Vendor::Amd, gpu_model: None, driver_version: None, min_driver: None, game_exe: None, settings: vec![PresetSetting { id: "adlx.antilag".into(), value: PresetValue::Number(1.0), name: String::new() }], created_at: 0, created_with: String::new(), notes: String::new() }).unwrap();
        let mut p = profile();
        p.actions.clear();
        p.priority = None;
        p.preset_id = Some(preset.id.clone());
        let p = w.ff.profile_save(p, 2).unwrap();
        let mut cfg = w.ff.config();
        cfg.auto_profiles = true;
        w.ff.set_config(cfg).unwrap();
        let events = w.ff.lifecycle_tick();
        assert!(events[0].text.contains("GPU preset"), "{events:?}");
        assert_eq!(w.amd.applied.lock().last().unwrap(), &("adlx.antilag".to_string(), 1.0));
        assert_eq!(w.ff.profile_views()[0].preset_name.as_deref(), Some("Low latency"));

        // Exit: driver settings are intentional and stay.
        w.ff.set_process_list(Some(Vec::new()));
        w.ff.lifecycle_tick();
        assert_eq!(w.amd.applied.lock().len(), 1);

        // Revert by hand undoes the driver change as well.
        w.ff.set_process_list(Some(vec![proc(GAME_PID, "game.exe", Some("steam:1"))]));
        // (The user turned the setting off again in the meantime.)
        w.ff.gpus().set_control(&w.ff.inventory(false).gpus[0], "adlx.antilag", 0.0).unwrap();
        w.amd.applied.lock().clear();
        w.ff.profile_apply(&p.id, 2).unwrap();
        w.ff.profile_revert(&p.id).unwrap();
        assert_eq!(w.amd.applied.lock().last().unwrap(), &("adlx.antilag".to_string(), 0.0));

        // The preset is deleted: the profile says so instead of failing.
        w.ff.presets().delete(&preset.id).unwrap();
        assert!(w.ff.profile_views()[0].preset_missing);
        let run = w.ff.profile_apply(&p.id, 2).unwrap();
        assert!(run.skipped.iter().any(|s| s.contains("no longer exists")));
    }

    #[test]
    fn export_import_round_trip_arrives_switched_off() {
        let w = world();
        let preset = w.ff.presets().add(Preset { schema: PRESET_SCHEMA, id: String::new(), name: "P".into(), vendor: Vendor::Amd, gpu_model: None, driver_version: None, min_driver: None, game_exe: None, settings: vec![PresetSetting { id: "adlx.antilag".into(), value: PresetValue::Number(1.0), name: String::new() }], created_at: 0, created_with: String::new(), notes: String::new() }).unwrap();
        let mut p = profile();
        p.preset_id = Some(preset.id.clone());
        let p = w.ff.profile_save(p, 2).unwrap();
        let file = w.dir.path().join("profile.json");
        w.ff.profile_export(&p.id, &file).unwrap();
        assert!(w.ff.profile_export(&p.id, Path::new("x.json")).is_err());
        let bytes = std::fs::read(&file).unwrap();
        let imported = w.ff.profile_import(&bytes).unwrap();
        assert_ne!(imported.id, p.id);
        assert!(!imported.enabled && !imported.auto_apply && imported.approved_level == 0);
        assert_eq!((imported.exe.as_str(), imported.priority.as_deref(), imported.actions.len()), ("Game.exe", Some("High"), 1));
        // The preset travelled with it and was stored as a new one.
        assert_ne!(imported.preset_id, p.preset_id);
        assert_eq!(w.ff.presets().list().len(), 2);

        // Tampered files are refused whole.
        let edit = |f: &dyn Fn(&mut Value)| {
            let mut v: Value = serde_json::from_slice(&bytes).unwrap();
            f(&mut v);
            w.ff.profile_import(&serde_json::to_vec(&v).unwrap())
        };
        assert!(edit(&|v| v["kind"] = "something-else".into()).is_err());
        assert!(edit(&|v| v["profile"]["priority"] = "Realtime".into()).is_err());
        assert!(edit(&|v| v["profile"]["exe"] = "lsass.exe".into()).is_err());
        assert!(edit(&|v| v["profile"]["actions"][0]["id"] = "process.end".into()).is_err());
        assert!(edit(&|v| v["preset"]["settings"][0]["id"] = "drs.0x00000001".into()).is_err());
        assert!(edit(&|v| v["profile"]["command"] = "calc.exe".into()).is_ok(), "unknown fields are ignored, never run");
        assert!(w.ff.profile_import(b"MZ\x90\x00").is_err());
        assert!(w.ff.profile_import(&vec![b' '; MAX_PROFILE_BYTES + 1]).is_err());
    }

    #[test]
    fn benchmark_capture_through_the_monitor() {
        let w = world();
        assert!(w.ff.bench_start("x").unwrap_err().to_string().contains("Live Monitor"));
        assert!(w.ff.bench_stop().is_err());
        let batches: Vec<Vec<Frame>> = (0..40).map(|_| (0..100).map(|_| Frame { frame_time_ms: 10.0, displayed_time_ms: Some(10.0), gpu_busy_ms: Some(9.0), ..Default::default() }).collect()).collect();
        w.ff.set_source_factory(Some(Arc::new(move |_| Ok(Box::new(ReplaySource::new(batches.clone())) as Box<dyn crate::frameforge::presentmon::FrameSource>))));
        w.ff.monitor_start(GAME_PID, "game.exe".into()).unwrap();
        w.ff.bench_start("  Stock settings ").unwrap();
        assert!(w.ff.bench_start("again").is_err(), "one recording at a time");
        for _ in 0..500 {
            if w.ff.monitor_snapshot().state == State::Ended {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let s = w.ff.bench_stop().unwrap();
        assert_eq!((s.label.as_str(), s.exe.as_str(), s.pid, s.game_name.as_str()), ("Stock settings", "game.exe", GAME_PID, "Test Game"));
        assert_eq!(s.game_id.as_deref(), Some("steam:1"));
        assert!(s.presented.frames > 0 && s.presented.frames <= 4000);
        assert!((s.headline().avg_fps.unwrap() - 100.0).abs() < 1e-6);
        assert_eq!(s.gpu.as_deref(), Some("Radeon"));
        assert_eq!(s.source, "Recorded frames");
        assert_eq!(w.ff.benchmarks().list().len(), 1);
        assert!(w.ff.bench_stop().is_err());
        assert_eq!(w.ff.profile_views().len(), 0);

        // Stopping the monitor while recording keeps what was captured.
        w.ff.monitor_start(GAME_PID, "game.exe".into()).unwrap();
        w.ff.bench_start("Second").unwrap();
        for _ in 0..200 {
            if w.ff.monitor_snapshot().recorded_frames > 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        w.ff.monitor_stop();
        let all = w.ff.benchmarks().list();
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|b| b.label == "Second"));
    }

    #[test]
    fn profile_monitoring_starts_and_stops_with_the_game() {
        let w = world();
        let batches: Vec<Vec<Frame>> = (0..2000).map(|_| (0..20).map(|_| Frame { frame_time_ms: 10.0, displayed_time_ms: Some(10.0), ..Default::default() }).collect()).collect();
        w.ff.set_source_factory(Some(Arc::new(move |_| Ok(Box::new(ReplaySource::new(batches.clone())) as Box<dyn crate::frameforge::presentmon::FrameSource>))));
        let mut p = profile();
        p.monitor = true;
        p.benchmark = true;
        w.ff.profile_save(p, 2).unwrap();
        let mut cfg = w.ff.config();
        cfg.auto_profiles = true;
        w.ff.set_config(cfg).unwrap();
        let events = w.ff.lifecycle_tick();
        assert!(events.iter().any(|e| e.kind == "applied") && events.iter().any(|e| e.kind == "monitor"), "{events:?}");
        let snap = w.ff.monitor_snapshot();
        assert_eq!((snap.state, snap.pid, snap.recording), (State::Running, Some(GAME_PID), true));
        for _ in 0..200 {
            if w.ff.monitor_snapshot().recorded_frames > 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // The game exits: monitoring stops and the benchmark is saved under the profile's name.
        w.procs.running.lock().remove(&GAME_PID);
        w.ff.set_process_list(Some(Vec::new()));
        let events = w.ff.lifecycle_tick();
        assert!(events[0].text.contains("Live Monitor stopped"), "{events:?}");
        assert_eq!(w.ff.monitor_snapshot().state, State::Idle);
        let saved = w.ff.benchmarks().list();
        assert_eq!(saved.len(), 1);
        assert_eq!((saved[0].label.as_str(), saved[0].profile.as_deref()), ("Competitive", Some("Competitive")));

        // Without a source, the failure is reported instead of hidden.
        w.ff.set_source_factory(Some(Arc::new(|_| anyhow::bail!("PresentMon is not installed"))));
        w.ff.set_process_list(Some(vec![proc(GAME_PID + 1, "game.exe", Some("steam:1"))]));
        w.procs.running.lock().insert(GAME_PID + 1, "Normal".into());
        let events = w.ff.lifecycle_tick();
        assert!(events.iter().any(|e| e.kind == "error" && e.text.contains("PresentMon is not installed")), "{events:?}");
    }
}
