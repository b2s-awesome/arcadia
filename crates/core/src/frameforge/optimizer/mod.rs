//! Reversible optimisation framework.
//!
//! Every change FrameForge can make to the system is an [`Action`] in one
//! registry. An action knows how to read the current value, what it would
//! set, how to apply it and how to put the old value back. The [`Engine`]
//! runs actions as a session with this fixed sequence per action:
//!
//! 1. check the level, applicability and that it is not already applied;
//! 2. read the current value and compute the proposed one;
//! 3. write the pending change record to disk (so a crash leaves a trace);
//! 4. apply;
//! 5. read back and verify;
//! 6. record the result.
//!
//! If any action in a session fails, the ones already applied in that
//! session are rolled back in reverse order, and the session is stored as
//! failed with the error and the state of every change. History lives in
//! `FrameForge\history\`, one JSON file per session, and is never mixed
//! with disposable monitoring data.

pub mod actions;
pub mod ops;

use crate::frameforge::gpu::{GpuRegistry, Risk};
use crate::frameforge::hardware::Inventory;
use crate::frameforge::processes::{ProcessInfo, ProcessOps};
use ops::SystemOps;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;

pub const HISTORY_SCHEMA: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Power,
    Windows,
    Startup,
    Process,
    Graphics,
    Driver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Restart {
    #[default]
    None,
    /// The game must be restarted to pick the change up.
    Game,
    SignOut,
    Windows,
}

/// Static description of an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionMeta {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: Category,
    pub risk: Risk,
    /// 1 = safe, 2 = advanced, 3 = expert.
    pub level: u8,
    pub restart: Restart,
    pub reversible: bool,
    /// Meant to last only while a game runs (priority, closed apps).
    pub temporary: bool,
    /// What it costs, in plain words.
    pub trade_off: String,
    /// The interface used and why the change is defensible.
    pub source: String,
    /// Hardware it applies to, in plain words.
    pub hardware: String,
    /// Windows versions it applies to, in plain words.
    pub os: String,
}

/// How an action shows up in the optimiser list.
pub enum Instances {
    /// One entry, no parameters.
    Single,
    /// One entry per item (e.g. per startup app): `(params, label)`.
    Many(Vec<(Value, String)>),
    /// Not listed; other pages invoke it with parameters (processes, GPU controls, game profiles).
    OnDemand,
}

/// Everything an action may look at or use.
pub struct Ctx<'a> {
    pub ops: &'a dyn SystemOps,
    pub procs: &'a dyn ProcessOps,
    pub gpus: &'a GpuRegistry,
    pub inv: &'a Inventory,
    pub processes: &'a [ProcessInfo],
}

pub trait Action: Send + Sync {
    fn meta(&self) -> ActionMeta;
    fn instances(&self, _ctx: &Ctx) -> Instances {
        Instances::Single
    }
    /// Identifies one target of the action, for duplicate prevention.
    fn key(&self, _params: &Value) -> String {
        self.meta().id
    }
    /// Level for these parameters (vendor controls carry their own).
    fn level(&self, _ctx: &Ctx, _params: &Value) -> u8 {
        self.meta().level
    }
    /// Risk for these parameters.
    fn risk(&self, _ctx: &Ctx, _params: &Value) -> Risk {
        self.meta().risk
    }
    /// `Err(reason)` when the action cannot be used here.
    fn applicable(&self, ctx: &Ctx, params: &Value) -> Result<(), String>;
    fn read(&self, ctx: &Ctx, params: &Value) -> anyhow::Result<Value>;
    fn proposed(&self, ctx: &Ctx, params: &Value) -> anyhow::Result<Value>;
    fn apply(&self, ctx: &Ctx, params: &Value, before: &Value, proposed: &Value) -> anyhow::Result<()>;
    fn rollback(&self, ctx: &Ctx, params: &Value, before: &Value) -> anyhow::Result<()>;
    /// Did the read-back value match what was intended?
    fn verify(&self, after: &Value, proposed: &Value) -> bool {
        after == proposed
    }
    /// Did a rollback put the old value back?
    fn verify_restored(&self, now: &Value, before: &Value) -> bool {
        now == before
    }
    /// A value as the user should read it.
    fn describe(&self, value: &Value) -> String {
        match value {
            Value::Null => "not set".into(),
            Value::String(s) => s.clone(),
            other => other.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeStatus {
    /// Recorded, apply not finished (seen only after a crash).
    Pending,
    Applied,
    /// The value already matched; nothing was written.
    Skipped,
    Failed,
    /// Undone automatically because a later action in the session failed.
    RolledBack,
    /// Undone by the user.
    Reverted,
    RevertFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub id: u32,
    pub action_id: String,
    pub key: String,
    pub name: String,
    pub category: Category,
    pub risk: Risk,
    pub level: u8,
    pub restart: Restart,
    pub reversible: bool,
    pub temporary: bool,
    pub params: Value,
    pub before: Value,
    pub proposed: Value,
    pub after: Option<Value>,
    pub before_text: String,
    pub proposed_text: String,
    pub after_text: Option<String>,
    pub status: ChangeStatus,
    pub error: Option<String>,
    pub applied_at: u64,
    pub reverted_at: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionStatus {
    InProgress,
    Applied,
    /// An action failed; everything applied before it was rolled back.
    Failed,
    /// An action failed and at least one rollback also failed. Needs attention.
    FailedDirty,
    PartlyReverted,
    Reverted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub schema: u32,
    pub id: String,
    pub label: String,
    pub started_at: u64,
    pub level: u8,
    pub status: SessionStatus,
    pub error: Option<String>,
    pub changes: Vec<Change>,
}

impl Session {
    fn settle(&mut self) {
        let applied = self.changes.iter().filter(|c| c.status == ChangeStatus::Applied).count();
        let undone = self.changes.iter().filter(|c| matches!(c.status, ChangeStatus::Reverted)).count();
        if matches!(self.status, SessionStatus::Applied | SessionStatus::PartlyReverted | SessionStatus::Reverted) {
            self.status = if undone == 0 {
                SessionStatus::Applied
            } else if applied == 0 {
                SessionStatus::Reverted
            } else {
                SessionStatus::PartlyReverted
            };
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApplyRequest {
    pub id: String,
    #[serde(default)]
    pub params: Value,
}

/// One row of the optimiser list.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionView {
    #[serde(flatten)]
    pub meta: ActionMeta,
    pub key: String,
    pub params: Value,
    /// Instance label (e.g. the startup app's name), when the action has instances.
    pub label: Option<String>,
    pub applicable: bool,
    pub reason: Option<String>,
    pub current: Option<String>,
    pub proposed: Option<String>,
    /// "available", "applied" (by Arcadia, can be reverted), "alreadySet" or "notApplicable".
    pub state: String,
    /// Session and change that applied it, when `state` is "applied".
    pub session_id: Option<String>,
    pub change_id: Option<u32>,
}

fn now() -> u64 {
    crate::frameforge::hardware::now_secs()
}

/// Sessions on disk.
pub struct HistoryStore {
    dir: PathBuf,
}

impl HistoryStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{}.json", crate::frameforge::safe_file_stem(id)))
    }

    pub fn save(&self, s: &Session) -> anyhow::Result<()> {
        crate::frameforge::write_atomic(&self.path(&s.id), &serde_json::to_vec_pretty(s)?)
    }

    /// Every stored session, newest first. Unreadable files are skipped, not fatal.
    pub fn load_all(&self) -> Vec<Session> {
        let mut out: Vec<Session> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
            .filter_map(|e| std::fs::read(e.path()).ok())
            .filter_map(|b| serde_json::from_slice::<Session>(&b).ok())
            .collect();
        out.sort_by(|a, b| b.started_at.cmp(&a.started_at).then_with(|| b.id.cmp(&a.id)));
        out
    }

    pub fn load(&self, id: &str) -> Option<Session> {
        serde_json::from_slice(&std::fs::read(self.path(id)).ok()?).ok()
    }
}

pub struct Engine {
    actions: Mutex<Vec<Arc<dyn Action>>>,
    store: HistoryStore,
    /// One session at a time.
    lock: Mutex<()>,
}

impl Engine {
    pub fn new(history_dir: PathBuf) -> Self {
        Self { actions: Mutex::new(actions::standard()), store: HistoryStore::new(history_dir), lock: Mutex::new(()) }
    }

    /// Add an action from another module (e.g. NVIDIA profile changes).
    pub fn register(&self, action: Arc<dyn Action>) {
        let mut a = self.actions.lock();
        let id = action.meta().id;
        a.retain(|x| x.meta().id != id);
        a.push(action);
    }

    fn action(&self, id: &str) -> Option<Arc<dyn Action>> {
        self.actions.lock().iter().find(|a| a.meta().id == id).cloned()
    }

    pub fn history(&self) -> Vec<Session> {
        self.store.load_all()
    }

    /// Changes Arcadia applied that are still in effect: `(session id, change)`.
    pub fn active_changes(&self) -> Vec<(String, Change)> {
        self.store
            .load_all()
            .into_iter()
            .flat_map(|s| {
                let id = s.id.clone();
                s.changes.into_iter().filter(|c| c.status == ChangeStatus::Applied && c.reversible).map(move |c| (id.clone(), c))
            })
            .collect()
    }

    /// The optimiser list: every listed action with its current and proposed value.
    pub fn list(&self, ctx: &Ctx) -> Vec<ActionView> {
        let active = self.active_changes();
        let mut out = Vec::new();
        for a in self.actions.lock().iter() {
            let meta = a.meta();
            let targets: Vec<(Value, Option<String>)> = match a.instances(ctx) {
                Instances::Single => vec![(Value::Null, None)],
                Instances::Many(items) => items.into_iter().map(|(p, l)| (p, Some(l))).collect(),
                Instances::OnDemand => continue,
            };
            for (params, label) in targets {
                let key = a.key(&params);
                let mut v = ActionView {
                    meta: ActionMeta { level: a.level(ctx, &params), ..meta.clone() },
                    key: key.clone(),
                    params: params.clone(),
                    label,
                    applicable: true,
                    reason: None,
                    current: None,
                    proposed: None,
                    state: "available".into(),
                    session_id: None,
                    change_id: None,
                };
                if let Some((sid, c)) = active.iter().find(|(_, c)| c.key == key) {
                    v.state = "applied".into();
                    v.session_id = Some(sid.clone());
                    v.change_id = Some(c.id);
                    v.current = a.read(ctx, &params).ok().map(|x| a.describe(&x));
                    v.proposed = Some(c.proposed_text.clone());
                    out.push(v);
                    continue;
                }
                if let Err(reason) = a.applicable(ctx, &params) {
                    v.applicable = false;
                    v.reason = Some(reason);
                    v.state = "notApplicable".into();
                    v.current = a.read(ctx, &params).ok().map(|x| a.describe(&x));
                    out.push(v);
                    continue;
                }
                match (a.read(ctx, &params), a.proposed(ctx, &params)) {
                    (Ok(cur), Ok(prop)) => {
                        if a.verify(&cur, &prop) {
                            v.state = "alreadySet".into();
                        }
                        v.current = Some(a.describe(&cur));
                        v.proposed = Some(a.describe(&prop));
                    }
                    (Err(e), _) | (_, Err(e)) => {
                        v.applicable = false;
                        v.reason = Some(format!("The current value could not be read: {e}"));
                        v.state = "notApplicable".into();
                    }
                }
                out.push(v);
            }
        }
        // Applied changes whose target is no longer listed (e.g. a start-up
        // entry that was removed) stay visible so they can be reverted.
        for (sid, c) in &active {
            if out.iter().any(|v| v.key == c.key) {
                continue;
            }
            let Some(a) = self.action(&c.action_id) else { continue };
            if matches!(a.instances(ctx), Instances::OnDemand) {
                continue;
            }
            out.push(ActionView {
                meta: a.meta(),
                key: c.key.clone(),
                params: c.params.clone(),
                label: c.params.get("name").and_then(|n| n.as_str()).map(String::from),
                applicable: true,
                reason: None,
                current: a.read(ctx, &c.params).ok().map(|x| a.describe(&x)),
                proposed: Some(c.proposed_text.clone()),
                state: "applied".into(),
                session_id: Some(sid.clone()),
                change_id: Some(c.id),
            });
        }
        out
    }

    /// Apply a set of actions as one session. Always returns the stored
    /// session, including on failure, so the caller can show exactly what happened.
    pub fn apply(&self, ctx: &Ctx, requests: &[ApplyRequest], max_level: u8, label: &str) -> anyhow::Result<Session> {
        let _guard = self.lock.lock();
        if requests.is_empty() {
            anyhow::bail!("Nothing was selected");
        }
        let active = self.active_changes();
        let mut session = Session {
            schema: HISTORY_SCHEMA,
            id: format!("{}-{}", now(), &uuid::Uuid::new_v4().simple().to_string()[..8]),
            label: label.chars().take(120).collect(),
            started_at: now(),
            level: max_level,
            status: SessionStatus::InProgress,
            error: None,
            changes: Vec::new(),
        };
        let mut failure: Option<String> = None;

        for (i, req) in requests.iter().enumerate() {
            let Some(action) = self.action(&req.id) else {
                failure = Some(format!("'{}' is not a known action", req.id));
                break;
            };
            let meta = action.meta();
            let level = action.level(ctx, &req.params);
            let key = action.key(&req.params);
            if level > max_level {
                failure = Some(format!("'{}' is a level {level} action; the optimiser is set to level {max_level}", meta.name));
                break;
            }
            if let Err(reason) = action.applicable(ctx, &req.params) {
                failure = Some(format!("'{}' can't be used here: {reason}", meta.name));
                break;
            }
            if active.iter().any(|(_, c)| c.key == key) || session.changes.iter().any(|c| c.key == key && c.status == ChangeStatus::Applied) {
                failure = Some(format!("'{}' is already applied. Revert it before applying it again.", meta.name));
                break;
            }
            let (before, proposed) = match (action.read(ctx, &req.params), action.proposed(ctx, &req.params)) {
                (Ok(b), Ok(p)) => (b, p),
                (Err(e), _) | (_, Err(e)) => {
                    failure = Some(format!("'{}': the current value could not be read: {e}", meta.name));
                    break;
                }
            };
            let mut change = Change {
                id: i as u32 + 1,
                action_id: meta.id.clone(),
                key,
                name: meta.name.clone(),
                category: meta.category,
                risk: action.risk(ctx, &req.params),
                level,
                restart: meta.restart,
                reversible: meta.reversible,
                temporary: meta.temporary,
                params: req.params.clone(),
                before_text: action.describe(&before),
                proposed_text: action.describe(&proposed),
                before,
                proposed,
                after: None,
                after_text: None,
                status: ChangeStatus::Pending,
                error: None,
                applied_at: now(),
                reverted_at: None,
            };
            if meta.reversible && action.verify(&change.before, &change.proposed) {
                change.status = ChangeStatus::Skipped;
                change.after_text = Some(change.before_text.clone());
                session.changes.push(change);
                continue;
            }
            // Write-ahead: the intent is on disk before the system is touched.
            session.changes.push(change);
            self.store.save(&session)?;
            let change = session.changes.last_mut().expect("just pushed");

            let result = action.apply(ctx, &req.params, &change.before, &change.proposed).and_then(|_| {
                let after = action.read(ctx, &req.params)?;
                change.after_text = Some(action.describe(&after));
                let ok = action.verify(&after, &change.proposed);
                change.after = Some(after);
                if ok {
                    Ok(())
                } else {
                    anyhow::bail!("the change did not take effect (read back: {})", change.after_text.clone().unwrap_or_default())
                }
            });
            match result {
                Ok(()) => {
                    change.status = ChangeStatus::Applied;
                    log::info!("FrameForge applied {} ({} -> {})", change.key, change.before_text, change.proposed_text);
                }
                Err(e) => {
                    change.status = ChangeStatus::Failed;
                    change.error = Some(format!("{e:#}"));
                    log::warn!("FrameForge failed to apply {}: {e:#}", change.key);
                    // Put this action's own target back if the failed apply left it changed.
                    let unchanged = action.read(ctx, &req.params).map(|now_value| action.verify_restored(&now_value, &change.before)).unwrap_or(false);
                    if meta.reversible && !unchanged {
                        if let Err(re) = action.rollback(ctx, &req.params, &change.before) {
                            change.error = Some(format!("{e:#}. Restoring the old value also failed: {re:#}"));
                            change.status = ChangeStatus::RevertFailed;
                        }
                    }
                    failure = Some(format!("'{}' failed: {e:#}", meta.name));
                    break;
                }
            }
        }

        if let Some(msg) = failure {
            let mut dirty = session.changes.iter().any(|c| c.status == ChangeStatus::RevertFailed);
            for c in session.changes.iter_mut().rev() {
                if c.status != ChangeStatus::Applied {
                    continue;
                }
                if !c.reversible {
                    continue;
                }
                let Some(action) = self.action(&c.action_id) else { continue };
                match action.rollback(ctx, &c.params, &c.before) {
                    Ok(()) => {
                        c.status = ChangeStatus::RolledBack;
                        c.reverted_at = Some(now());
                    }
                    Err(e) => {
                        c.status = ChangeStatus::RevertFailed;
                        c.error = Some(format!("Rollback failed: {e:#}"));
                        dirty = true;
                    }
                }
            }
            session.status = if dirty { SessionStatus::FailedDirty } else { SessionStatus::Failed };
            session.error = Some(msg);
        } else {
            session.status = SessionStatus::Applied;
        }
        self.store.save(&session)?;
        Ok(session)
    }

    fn revert_one(&self, ctx: &Ctx, c: &mut Change) -> Result<(), String> {
        if c.status != ChangeStatus::Applied {
            return Err(format!("'{}' is not currently applied", c.name));
        }
        if !c.reversible {
            return Err(format!("'{}' cannot be undone", c.name));
        }
        let action = self.action(&c.action_id).ok_or_else(|| format!("The action '{}' no longer exists in this version of Arcadia", c.action_id))?;
        let result = action.rollback(ctx, &c.params, &c.before).and_then(|_| {
            let now_value = action.read(ctx, &c.params)?;
            if action.verify_restored(&now_value, &c.before) {
                Ok(())
            } else {
                anyhow::bail!("the old value did not come back (now: {})", action.describe(&now_value))
            }
        });
        match result {
            Ok(()) => {
                c.status = ChangeStatus::Reverted;
                c.reverted_at = Some(now());
                c.error = None;
                log::info!("FrameForge reverted {} (back to {})", c.key, c.before_text);
                Ok(())
            }
            Err(e) => {
                c.error = Some(format!("Revert failed: {e:#}"));
                log::warn!("FrameForge failed to revert {}: {e:#}", c.key);
                Err(format!("'{}' could not be reverted: {e:#}", c.name))
            }
        }
    }

    /// Undo one change of a session.
    pub fn revert_change(&self, ctx: &Ctx, session_id: &str, change_id: u32) -> anyhow::Result<Session> {
        let _guard = self.lock.lock();
        let mut s = self.store.load(session_id).ok_or_else(|| anyhow::anyhow!("That optimisation session no longer exists"))?;
        let c = s.changes.iter_mut().find(|c| c.id == change_id).ok_or_else(|| anyhow::anyhow!("That change is not part of the session"))?;
        let r = self.revert_one(ctx, c);
        s.settle();
        self.store.save(&s)?;
        r.map_err(|e| anyhow::anyhow!(e))?;
        Ok(s)
    }

    /// Undo every applied change of a session, newest first. Changes that
    /// fail to revert stay applied and carry the error.
    pub fn revert_session(&self, ctx: &Ctx, session_id: &str) -> anyhow::Result<Session> {
        let _guard = self.lock.lock();
        let mut s = self.store.load(session_id).ok_or_else(|| anyhow::anyhow!("That optimisation session no longer exists"))?;
        let mut errors = Vec::new();
        for c in s.changes.iter_mut().rev() {
            if c.status == ChangeStatus::Applied && c.reversible {
                if let Err(e) = self.revert_one(ctx, c) {
                    errors.push(e);
                }
            }
        }
        s.settle();
        if !errors.is_empty() {
            s.error = Some(errors.join("; "));
        }
        self.store.save(&s)?;
        Ok(s)
    }

    /// Undo every still-applied temporary change (called when a game exits).
    pub fn revert_temporary(&self, ctx: &Ctx, only_keys_with: Option<&str>) -> Vec<String> {
        let mut errors = Vec::new();
        for (sid, c) in self.active_changes() {
            if c.temporary && only_keys_with.map(|k| c.key.contains(k)).unwrap_or(true) {
                if let Err(e) = self.revert_change(ctx, &sid, c.id) {
                    errors.push(e.to_string());
                }
            }
        }
        errors
    }
}

#[cfg(test)]
mod tests {
    use super::actions::*;
    use super::ops::{MockOps, MockProcs};
    use super::*;
    use crate::frameforge::hardware::StartupApp;
    use serde_json::json;

    struct World {
        ops: MockOps,
        procs: MockProcs,
        gpus: GpuRegistry,
        inv: Inventory,
        processes: Vec<ProcessInfo>,
        _dir: tempfile::TempDir,
        engine: Engine,
    }

    impl World {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let ops = MockOps::typical();
            ops.0.lock().strings.insert((RUN_KEY.into(), "Discord".into()), "\"C:\\Discord\\Update.exe\" --start".into());
            let mut inv = Inventory::default();
            inv.system.is_windows = true;
            inv.system.startup_apps = vec![StartupApp { name: "Discord".into(), command: "x".into(), scope: "HKCU".into(), enabled: true }];
            Self { ops, procs: MockProcs::with(&[100, 200]), gpus: GpuRegistry::empty(), inv, processes: Vec::new(), engine: Engine::new(dir.path().to_path_buf()), _dir: dir }
        }
        fn ctx(&self) -> Ctx<'_> {
            Ctx { ops: &self.ops, procs: &self.procs, gpus: &self.gpus, inv: &self.inv, processes: &self.processes }
        }
        fn req(id: &str) -> ApplyRequest {
            ApplyRequest { id: id.into(), params: Value::Null }
        }
    }

    #[test]
    fn apply_reads_records_verifies_and_reverts() {
        let w = World::new();
        let s = w.engine.apply(&w.ctx(), &[World::req("power.plan.high_performance")], 1, "test").unwrap();
        assert_eq!(s.status, SessionStatus::Applied);
        let c = &s.changes[0];
        assert_eq!(c.status, ChangeStatus::Applied);
        assert_eq!(c.before_text, "Balanced");
        assert_eq!(c.after_text.as_deref(), Some("High performance"));
        assert!(w.ops.power_plans().iter().any(|p| p.active && p.name == "High performance"));
        // Persisted, and listed as applied with a way back.
        assert_eq!(w.engine.history().len(), 1);
        let view = w.engine.list(&w.ctx()).into_iter().find(|v| v.meta.id == "power.plan.high_performance").unwrap();
        assert_eq!(view.state, "applied");
        let s = w.engine.revert_change(&w.ctx(), &s.id, c.id).unwrap();
        assert_eq!(s.status, SessionStatus::Reverted);
        assert!(w.ops.power_plans().iter().any(|p| p.active && p.name == "Balanced"));
        // A second revert is refused.
        assert!(w.engine.revert_change(&w.ctx(), &s.id, 1).is_err());
    }

    #[test]
    fn duplicate_actions_are_refused() {
        let w = World::new();
        w.ops.0.lock().u32s.insert((GAMEBAR_KEY.into(), "AutoGameModeEnabled".into()), 0);
        assert_eq!(w.engine.apply(&w.ctx(), &[World::req("windows.game_mode")], 2, "a").unwrap().status, SessionStatus::Applied);
        // Turned off by something else in the meantime: Arcadia still holds an active record.
        w.ops.0.lock().u32s.insert((GAMEBAR_KEY.into(), "AutoGameModeEnabled".into()), 0);
        let s = w.engine.apply(&w.ctx(), &[World::req("windows.game_mode")], 2, "b").unwrap();
        assert_eq!(s.status, SessionStatus::Failed);
        assert!(s.error.unwrap().contains("already applied"));
    }

    #[test]
    fn already_set_values_are_skipped_not_rewritten() {
        let w = World::new();
        w.ops.0.lock().u32s.insert((GAMEDVR_KEY.into(), "HistoricalCaptureEnabled".into()), 0);
        let s = w.engine.apply(&w.ctx(), &[World::req("windows.background_recording_off")], 2, "x").unwrap();
        assert_eq!(s.changes[0].status, ChangeStatus::Skipped);
        assert!(w.ops.0.lock().log.is_empty());
    }

    #[test]
    fn partial_failure_rolls_back_everything_applied_so_far() {
        let w = World::new();
        w.ops.0.lock().u32s.insert((GAMEDVR_KEY.into(), "HistoricalCaptureEnabled".into()), 1);
        w.ops.0.lock().fail_on = Some("AppCaptureEnabled".into());
        let reqs = [World::req("power.plan.high_performance"), World::req("windows.background_recording_off"), World::req("windows.game_capture_off")];
        let s = w.engine.apply(&w.ctx(), &reqs, 2, "three").unwrap();
        assert_eq!(s.status, SessionStatus::Failed);
        assert!(s.error.as_ref().unwrap().contains("Game Bar"));
        assert_eq!(s.changes.iter().map(|c| c.status).collect::<Vec<_>>(), vec![ChangeStatus::RolledBack, ChangeStatus::RolledBack, ChangeStatus::Failed]);
        // The machine is back where it started.
        assert!(w.ops.power_plans().iter().any(|p| p.active && p.name == "Balanced"));
        assert_eq!(w.ops.user_u32(GAMEDVR_KEY, "HistoricalCaptureEnabled"), Some(1));
        // The failed session is in the history for review.
        let h = w.engine.history();
        assert_eq!(h[0].status, SessionStatus::Failed);
        assert!(h[0].changes[2].error.is_some());
        assert!(w.engine.active_changes().is_empty());
    }

    #[test]
    fn a_write_that_does_not_stick_fails_verification() {
        let w = World::new();
        w.ops.0.lock().u32s.insert((GAMEBAR_KEY.into(), "AutoGameModeEnabled".into()), 0);
        w.ops.0.lock().ignore_writes = true;
        let s = w.engine.apply(&w.ctx(), &[World::req("windows.game_mode")], 2, "x").unwrap();
        assert_eq!(s.status, SessionStatus::Failed);
        assert!(s.changes[0].error.as_ref().unwrap().contains("did not take effect"));
    }

    #[test]
    fn level_and_applicability_are_enforced() {
        let w = World::new();
        let s = w.engine.apply(&w.ctx(), &[World::req("windows.game_mode")], 1, "x").unwrap();
        assert!(s.error.unwrap().contains("level 2"));
        assert!(w.ops.0.lock().log.is_empty());
        // Tuning the gaming plan before it exists is "not applicable".
        let s = w.engine.apply(&w.ctx(), &[World::req("power.gaming.cpu_min_state")], 3, "x").unwrap();
        assert!(s.error.unwrap().contains("Arcadia Gaming"));
        let s = w.engine.apply(&w.ctx(), &[World::req("made.up")], 3, "x").unwrap();
        assert!(s.error.unwrap().contains("not a known action"));
        assert!(w.engine.apply(&w.ctx(), &[], 3, "x").is_err());
    }

    #[test]
    fn gaming_plan_create_tune_and_remove() {
        let w = World::new();
        let s = w.engine.apply(&w.ctx(), &[World::req("power.plan.gaming"), World::req("power.gaming.cpu_min_state"), World::req("power.gaming.pcie_aspm_off")], 2, "plan").unwrap();
        assert_eq!(s.status, SessionStatus::Applied, "{:?}", s.error);
        let plans = w.ops.power_plans();
        let gaming = plans.iter().find(|p| p.name == GAMING_PLAN_NAME).unwrap();
        assert!(gaming.active);
        assert_eq!(w.ops.read_power_value(&gaming.guid, SUB_PROCESSOR, SET_PROC_MIN).unwrap(), 100);
        // Only the Arcadia plan was edited, never a Windows plan.
        assert_eq!(w.ops.read_power_value(PLAN_HIGH_PERFORMANCE, SUB_PROCESSOR, SET_PROC_MIN).unwrap(), 0);
        let s = w.engine.revert_session(&w.ctx(), &s.id).unwrap();
        assert_eq!(s.status, SessionStatus::Reverted, "{:?}", s.error);
        let plans = w.ops.power_plans();
        assert!(plans.iter().any(|p| p.active && p.name == "Balanced"));
        assert!(!plans.iter().any(|p| p.name == GAMING_PLAN_NAME));
    }

    #[test]
    fn startup_apps_are_listed_disabled_and_restored() {
        let w = World::new();
        let views = w.engine.list(&w.ctx());
        let v = views.iter().find(|v| v.meta.id == "startup.disable").unwrap();
        assert_eq!(v.label.as_deref(), Some("Discord"));
        let req = ApplyRequest { id: "startup.disable".into(), params: json!({ "name": "Discord" }) };
        let s = w.engine.apply(&w.ctx(), &[req], 2, "startup").unwrap();
        assert_eq!(s.status, SessionStatus::Applied);
        assert!(w.ops.user_string(RUN_KEY, "Discord").is_none());
        w.engine.revert_session(&w.ctx(), &s.id).unwrap();
        assert_eq!(w.ops.user_string(RUN_KEY, "Discord").as_deref(), Some("\"C:\\Discord\\Update.exe\" --start"));
        // Unknown names are rejected rather than creating a value.
        let bad = ApplyRequest { id: "startup.disable".into(), params: json!({ "name": "Nope" }) };
        assert_eq!(w.engine.apply(&w.ctx(), &[bad], 2, "x").unwrap().status, SessionStatus::Failed);
    }

    #[test]
    fn process_actions_respect_protection_and_priority_rules() {
        let mut w = World::new();
        let mk = |pid, name: &str, cat, closable| ProcessInfo {
            pid, name: name.into(), exe: None, cpu_pct: 0.0, memory_bytes: 0, gpu_pct: None, owner: None, category: cat, game_id: None, game_name: None,
            relevance: String::new(), closable, protected_reason: (!closable && cat == crate::frameforge::processes::Category::Protected).then(|| "Windows component".into()),
        };
        use crate::frameforge::processes::Category as C;
        w.processes = vec![mk(100, "game.exe", C::Game, false), mk(200, "chrome.exe", C::Browser, true), mk(300, "MsMpEng.exe", C::Protected, false)];
        // Protected: refused for close, end and priority.
        for id in ["process.close", "process.end", "process.priority"] {
            let r = ApplyRequest { id: id.into(), params: json!({ "pid": 300, "class": "High" }) };
            let s = w.engine.apply(&w.ctx(), &[r], 3, "x").unwrap();
            assert_eq!(s.status, SessionStatus::Failed, "{id}");
        }
        // Realtime: refused.
        let r = ApplyRequest { id: "process.priority".into(), params: json!({ "pid": 100, "class": "Realtime" }) };
        assert!(w.engine.apply(&w.ctx(), &[r], 3, "x").unwrap().error.unwrap().contains("never set"));
        // Priority is temporary and comes back.
        let r = ApplyRequest { id: "process.priority".into(), params: json!({ "pid": 100, "class": "High" }) };
        let s = w.engine.apply(&w.ctx(), &[r], 2, "prio").unwrap();
        assert_eq!(s.status, SessionStatus::Applied);
        assert!(s.changes[0].temporary);
        assert_eq!(w.procs.priority(100).unwrap(), "High");
        assert!(w.engine.revert_temporary(&w.ctx(), None).is_empty());
        assert_eq!(w.procs.priority(100).unwrap(), "Normal");
        // Closing a browser is allowed, recorded, and not reversible.
        let r = ApplyRequest { id: "process.close".into(), params: json!({ "pid": 200 }) };
        let s = w.engine.apply(&w.ctx(), &[r], 1, "close").unwrap();
        assert_eq!(s.status, SessionStatus::Applied);
        assert!(!s.changes[0].reversible);
        assert_eq!(w.procs.closed.lock().as_slice(), &[200]);
        assert!(w.engine.revert_change(&w.ctx(), &s.id, 1).is_err());
    }

    #[test]
    fn priority_restore_after_the_game_exited_is_not_an_error() {
        let mut w = World::new();
        use crate::frameforge::processes::Category as C;
        w.processes = vec![ProcessInfo { pid: 100, name: "game.exe".into(), exe: None, cpu_pct: 0.0, memory_bytes: 0, gpu_pct: None, owner: None, category: C::Game, game_id: None, game_name: None, relevance: String::new(), closable: false, protected_reason: None }];
        let r = ApplyRequest { id: "process.priority".into(), params: json!({ "pid": 100, "class": "AboveNormal" }) };
        let s = w.engine.apply(&w.ctx(), &[r], 2, "prio").unwrap();
        w.procs.running.lock().clear();
        let s = w.engine.revert_session(&w.ctx(), &s.id).unwrap();
        assert_eq!(s.status, SessionStatus::Reverted, "{:?}", s.error);
    }

    #[test]
    fn gpu_preference_needs_two_gpus_and_keeps_other_flags() {
        use crate::frameforge::gpu::{GpuDevice, GpuKind};
        let mut w = World::new();
        let exe = r"D:\Games\Thing\thing.exe";
        let r = || ApplyRequest { id: "graphics.gpu_preference".into(), params: json!({ "exe": exe }) };
        assert!(w.engine.apply(&w.ctx(), &[r()], 2, "x").unwrap().error.unwrap().contains("one graphics adapter"));
        w.inv.gpus = vec![GpuDevice { kind: GpuKind::Integrated, ..Default::default() }, GpuDevice { kind: GpuKind::Discrete, ..Default::default() }];
        w.ops.0.lock().strings.insert((GPU_PREF_KEY.into(), exe.into()), "SwapEffectUpgradeEnable=1;GpuPreference=1;".into());
        let s = w.engine.apply(&w.ctx(), &[r()], 2, "x").unwrap();
        assert_eq!(s.status, SessionStatus::Applied, "{:?}", s.error);
        assert_eq!(w.ops.user_string(GPU_PREF_KEY, exe).unwrap(), "SwapEffectUpgradeEnable=1;GpuPreference=2;");
        w.engine.revert_session(&w.ctx(), &s.id).unwrap();
        assert_eq!(w.ops.user_string(GPU_PREF_KEY, exe).unwrap(), "SwapEffectUpgradeEnable=1;GpuPreference=1;");
        let bad = ApplyRequest { id: "graphics.gpu_preference".into(), params: json!({ "exe": "..\\evil.exe" }) };
        assert_eq!(w.engine.apply(&w.ctx(), &[bad], 2, "x").unwrap().status, SessionStatus::Failed);
    }

    #[test]
    fn vendor_controls_go_through_the_engine_with_history() {
        use crate::frameforge::gpu::mock::{MockBase, MockVendor};
        use crate::frameforge::gpu::{ControlKind, GpuControl, GpuDevice, Vendor};
        let mut w = World::new();
        let dev = GpuDevice { id: "g0".into(), vendor: Vendor::Intel, name: "Arc".into(), ..Default::default() };
        let mut intel = MockVendor::available(Vendor::Intel);
        intel.controls = vec![GpuControl {
            id: "igcl.power_limit".into(), vendor: Vendor::Intel, name: "Power limit".into(), description: String::new(), category: "Tuning".into(),
            kind: ControlKind::Range { min: 100.0, max: 228.0, step: 1.0, unit: "W".into() }, value: Some(190.0), default: Some(190.0),
            supported: true, unsupported_reason: None, risk: Risk::High, level: 3, requires_restart: false,
        }];
        w.gpus = GpuRegistry::new(Arc::new(MockBase::new(vec![dev])), vec![Arc::new(intel)]);
        let r = |v: f64| ApplyRequest { id: "gpu.control".into(), params: json!({ "gpu": "g0", "control": "igcl.power_limit", "value": v }) };
        // Expert control: refused below level 3, and outside the driver's range.
        assert!(w.engine.apply(&w.ctx(), &[r(200.0)], 2, "x").unwrap().error.unwrap().contains("level 3"));
        assert!(w.engine.apply(&w.ctx(), &[r(400.0)], 3, "x").unwrap().error.unwrap().contains("between"));
        let s = w.engine.apply(&w.ctx(), &[r(200.0)], 3, "x").unwrap();
        assert_eq!(s.status, SessionStatus::Applied, "{:?}", s.error);
        assert_eq!(s.changes[0].before_text, "190 W");
        assert_eq!(s.changes[0].after_text.as_deref(), Some("200 W"));
        let s = w.engine.revert_session(&w.ctx(), &s.id).unwrap();
        assert_eq!(s.status, SessionStatus::Reverted);
        assert_eq!(w.gpus.controls(&w.gpus.devices()[0])[0].value, Some(190.0));
    }

    #[test]
    fn history_survives_a_restart_and_tolerates_junk_files() {
        let w = World::new();
        let s = w.engine.apply(&w.ctx(), &[World::req("power.plan.high_performance")], 1, "persist").unwrap();
        std::fs::write(w._dir.path().join("garbage.json"), b"{not json").unwrap();
        let again = Engine::new(w._dir.path().to_path_buf());
        let h = again.history();
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].id, s.id);
        assert_eq!(again.active_changes().len(), 1);
        // The fresh engine can still revert it.
        assert_eq!(again.revert_session(&w.ctx(), &s.id).unwrap().status, SessionStatus::Reverted);
    }
}
