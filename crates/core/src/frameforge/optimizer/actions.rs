//! The action registry: every change the optimiser can make.
//!
//! Interfaces used, and nothing else:
//! * Power plans: the Win32 power-management API (`PowerSetActiveScheme`,
//!   `PowerDuplicateScheme`, `PowerWriteACValueIndex`). Arcadia only edits a
//!   plan it created ("Arcadia Gaming"); Windows' own plans are switched
//!   between, never modified.
//! * Per-user Windows settings: the same `HKEY_CURRENT_USER` values the
//!   Settings app writes for Game Mode, Game Bar capture, the per-app GPU
//!   choice and the per-user start-up list.
//! * Processes: `WM_CLOSE`, `TerminateProcess` (after confirmation),
//!   `SetPriorityClass` (never REALTIME).
//! * GPU drivers: only controls a vendor API reports, inside its limits.
//!
//! Not here on purpose: services, Defender, VBS / Core Isolation, boot
//! configuration, timers, Windows Update, anti-cheat, game files or memory.

use super::*;
use crate::frameforge::gpu::{self, ControlKind, GpuKind};
use crate::frameforge::processes::{check_priority, Category as ProcCategory};
use serde_json::json;

pub const PLAN_BALANCED: &str = "381b4222-f694-41f0-9685-ff5bb260df2e";
pub const PLAN_HIGH_PERFORMANCE: &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";
pub const PLAN_POWER_SAVER: &str = "a1841308-3541-4fab-bc81-f71556f20b4a";
pub const PLAN_ULTIMATE: &str = "e9a42b02-d5df-448d-aa00-03f14749eb61";
pub const GAMING_PLAN_NAME: &str = "Arcadia Gaming";

// Power setting ids from the Windows SDK (winnt.h): GUID_PROCESSOR_SETTINGS_SUBGROUP,
// GUID_PROCESSOR_THROTTLE_MINIMUM, GUID_PCIEXPRESS_SETTINGS_SUBGROUP,
// GUID_PCIEXPRESS_ASPM_POLICY, and the USB selective-suspend setting.
pub const SUB_PROCESSOR: &str = "54533251-82be-4824-96c1-47b60b740d00";
pub const SET_PROC_MIN: &str = "893dee8e-2bef-41e0-89c6-b55d0929964c";
pub const SUB_PCIEXPRESS: &str = "501a4d13-42af-4429-9fd1-a8218c268e20";
pub const SET_ASPM: &str = "ee12f906-d277-404b-b6da-e5fa1a576df5";
pub const SUB_USB: &str = "2a737441-1930-4402-8d77-b2bebba308a3";
pub const SET_USB_SUSPEND: &str = "48e6b7a6-50f5-4782-a5d4-53bb8f07e226";

pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
pub const GAMEBAR_KEY: &str = r"Software\Microsoft\GameBar";
pub const GAMEDVR_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\GameDVR";
pub const GPU_PREF_KEY: &str = r"Software\Microsoft\DirectX\UserGpuPreferences";

const WIN10: &str = "Windows 10 and 11";

pub fn standard() -> Vec<Arc<dyn Action>> {
    vec![
        Arc::new(PlanHighPerformance),
        Arc::new(PlanGaming),
        Arc::new(PowerValue {
            id: "power.gaming.cpu_min_state",
            name: "Keep the processor at full speed (Arcadia Gaming plan)",
            description: "Sets the minimum processor state of the Arcadia Gaming plan to 100% while plugged in, so cores do not clock down between frames.",
            subgroup: SUB_PROCESSOR,
            setting: SET_PROC_MIN,
            target: 100,
            trade_off: "Higher idle power use and temperature while this plan is active.",
            show: |v| format!("{v}%"),
        }),
        Arc::new(PowerValue {
            id: "power.gaming.pcie_aspm_off",
            name: "Turn off PCI Express link power saving (Arcadia Gaming plan)",
            description: "Stops the graphics card's PCI Express link from entering low-power states while the Arcadia Gaming plan is active.",
            subgroup: SUB_PCIEXPRESS,
            setting: SET_ASPM,
            target: 0,
            trade_off: "A little more idle power use. On laptops it shortens battery life.",
            show: |v| match v {
                0 => "Off".into(),
                1 => "Moderate power savings".into(),
                2 => "Maximum power savings".into(),
                other => other.to_string(),
            },
        }),
        Arc::new(PowerValue {
            id: "power.gaming.usb_suspend_off",
            name: "Turn off USB selective suspend (Arcadia Gaming plan)",
            description: "Keeps USB devices such as mice, keyboards and headsets from being suspended while the Arcadia Gaming plan is active.",
            subgroup: SUB_USB,
            setting: SET_USB_SUSPEND,
            target: 0,
            trade_off: "Idle USB devices draw slightly more power.",
            show: |v| if v == 0 { "Off".into() } else { "On".into() },
        }),
        Arc::new(UserDword {
            id: "windows.game_mode",
            name: "Turn on Windows Game Mode",
            description: "Game Mode stops Windows Update from installing drivers and showing restart prompts while you play, and favours the game when sharing the processor.",
            path: GAMEBAR_KEY,
            value: "AutoGameModeEnabled",
            target: 1,
            absent: Some(1),
            trade_off: "None known on current Windows versions.",
            risk: Risk::Low,
        }),
        Arc::new(UserDword {
            id: "windows.background_recording_off",
            name: "Turn off background recording",
            description: "\"Record what happened\" keeps a video encoder running the whole time a game is open. Turning it off frees that work; manual recording still works.",
            path: GAMEDVR_KEY,
            value: "HistoricalCaptureEnabled",
            target: 0,
            absent: Some(0),
            trade_off: "The \"record the last 30 seconds\" shortcut stops working.",
            risk: Risk::Low,
        }),
        Arc::new(UserDword {
            id: "windows.game_capture_off",
            name: "Turn off Game Bar capture",
            description: "Disables Game Bar screen and clip capture for this Windows account.",
            path: GAMEDVR_KEY,
            value: "AppCaptureEnabled",
            target: 0,
            absent: None,
            trade_off: "Game Bar can no longer record clips or take screenshots until this is reverted.",
            risk: Risk::Low,
        }),
        Arc::new(StartupDisable),
        Arc::new(GpuPreference),
        Arc::new(ProcessClose),
        Arc::new(ProcessEnd),
        Arc::new(ProcessPriority),
        Arc::new(GpuControlAction),
    ]
}

fn active_plan(ctx: &Ctx) -> anyhow::Result<Value> {
    ctx.ops
        .power_plans()
        .into_iter()
        .find(|p| p.active)
        .map(|p| json!({ "guid": p.guid, "name": p.name }))
        .ok_or_else(|| anyhow::anyhow!("Windows reported no active power plan"))
}

fn plan_name(v: &Value) -> String {
    match v.get("name").and_then(|n| n.as_str()) {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => v.get("guid").and_then(|g| g.as_str()).unwrap_or("unknown plan").to_string(),
    }
}

fn guid_of(v: &Value) -> Option<&str> {
    v.get("guid").and_then(|g| g.as_str())
}

fn power_ok(ctx: &Ctx) -> Result<(), String> {
    if ctx.ops.power_plans().is_empty() {
        return Err("Power plans could not be read on this system".into());
    }
    if ctx.ops.on_battery() == Some(true) {
        return Err("The PC is running on battery. Plug it in first: a performance plan drains the battery quickly.".into());
    }
    Ok(())
}

/// Put `before` back as the active plan; fall back to Balanced when that plan is gone.
fn restore_plan(ctx: &Ctx, before: &Value) -> anyhow::Result<()> {
    let plans = ctx.ops.power_plans();
    let want = guid_of(before).filter(|g| plans.iter().any(|p| p.guid == *g)).or_else(|| plans.iter().find(|p| p.guid == PLAN_BALANCED).map(|_| PLAN_BALANCED));
    match want {
        Some(g) => ctx.ops.set_active_power_plan(g),
        None => anyhow::bail!("the previous power plan no longer exists"),
    }
}

struct PlanHighPerformance;

impl Action for PlanHighPerformance {
    fn meta(&self) -> ActionMeta {
        ActionMeta {
            id: "power.plan.high_performance".into(),
            name: "Switch to the High performance power plan".into(),
            description: "Makes Windows' own High performance plan the active one. Useful when the PC is on Power saver or a vendor \"quiet\" plan that holds the processor back.".into(),
            category: Category::Power,
            risk: Risk::Low,
            level: 1,
            restart: Restart::None,
            reversible: true,
            temporary: false,
            trade_off: "More power use and heat when idle. On Balanced the gain in games is usually small.".into(),
            source: "Win32 power management API (PowerSetActiveScheme). Switches between existing plans; no plan is edited.".into(),
            hardware: "Any PC on mains power".into(),
            os: WIN10.into(),
        }
    }
    fn applicable(&self, ctx: &Ctx, _: &Value) -> Result<(), String> {
        power_ok(ctx)?;
        self.proposed(ctx, &Value::Null).map(|_| ()).map_err(|e| e.to_string())
    }
    fn read(&self, ctx: &Ctx, _: &Value) -> anyhow::Result<Value> {
        active_plan(ctx)
    }
    fn proposed(&self, ctx: &Ctx, _: &Value) -> anyhow::Result<Value> {
        let plans = ctx.ops.power_plans();
        plans
            .iter()
            .find(|p| p.guid == PLAN_HIGH_PERFORMANCE)
            .or_else(|| plans.iter().find(|p| p.guid == PLAN_ULTIMATE))
            .map(|p| json!({ "guid": p.guid, "name": p.name }))
            .ok_or_else(|| anyhow::anyhow!("This PC has no High performance plan (common on laptops with Modern Standby). Use the Arcadia Gaming plan instead."))
    }
    fn apply(&self, ctx: &Ctx, _: &Value, _: &Value, proposed: &Value) -> anyhow::Result<()> {
        ctx.ops.set_active_power_plan(guid_of(proposed).ok_or_else(|| anyhow::anyhow!("no target plan"))?)
    }
    fn rollback(&self, ctx: &Ctx, _: &Value, before: &Value) -> anyhow::Result<()> {
        restore_plan(ctx, before)
    }
    fn verify(&self, after: &Value, proposed: &Value) -> bool {
        guid_of(after).is_some() && guid_of(after) == guid_of(proposed)
    }
    fn verify_restored(&self, now: &Value, before: &Value) -> bool {
        guid_of(now) == guid_of(before) || guid_of(now) == Some(PLAN_BALANCED)
    }
    fn describe(&self, v: &Value) -> String {
        plan_name(v)
    }
}

fn gaming_plan(ctx: &Ctx) -> Option<crate::frameforge::hardware::PowerPlan> {
    ctx.ops.power_plans().into_iter().find(|p| p.name == GAMING_PLAN_NAME)
}

struct PlanGaming;

impl Action for PlanGaming {
    fn meta(&self) -> ActionMeta {
        ActionMeta {
            id: "power.plan.gaming".into(),
            name: "Create and use the Arcadia Gaming power plan".into(),
            description: "Copies High performance (or Balanced when that plan does not exist) into a separate plan named \"Arcadia Gaming\" and switches to it. The plan-tuning actions below only ever edit this copy.".into(),
            category: Category::Power,
            risk: Risk::Medium,
            level: 2,
            restart: Restart::None,
            reversible: true,
            temporary: false,
            trade_off: "More power use and heat while the plan is active. Reverting switches back and deletes the plan.".into(),
            source: "Win32 power management API (PowerDuplicateScheme, PowerSetActiveScheme, PowerDeleteScheme).".into(),
            hardware: "Any PC on mains power".into(),
            os: WIN10.into(),
        }
    }
    fn applicable(&self, ctx: &Ctx, _: &Value) -> Result<(), String> {
        power_ok(ctx)
    }
    fn read(&self, ctx: &Ctx, _: &Value) -> anyhow::Result<Value> {
        active_plan(ctx)
    }
    fn proposed(&self, _: &Ctx, _: &Value) -> anyhow::Result<Value> {
        Ok(json!({ "name": GAMING_PLAN_NAME }))
    }
    fn apply(&self, ctx: &Ctx, _: &Value, _: &Value, _: &Value) -> anyhow::Result<()> {
        let guid = match gaming_plan(ctx) {
            Some(p) => p.guid,
            None => {
                let plans = ctx.ops.power_plans();
                let source = [PLAN_HIGH_PERFORMANCE, PLAN_BALANCED]
                    .into_iter()
                    .find(|g| plans.iter().any(|p| p.guid == *g))
                    .map(str::to_string)
                    .or_else(|| plans.iter().find(|p| p.active).map(|p| p.guid.clone()))
                    .ok_or_else(|| anyhow::anyhow!("no power plan to copy"))?;
                ctx.ops.duplicate_power_plan(&source, GAMING_PLAN_NAME)?
            }
        };
        ctx.ops.set_active_power_plan(&guid)
    }
    fn rollback(&self, ctx: &Ctx, _: &Value, before: &Value) -> anyhow::Result<()> {
        restore_plan(ctx, before)?;
        if let Some(p) = gaming_plan(ctx) {
            ctx.ops.delete_power_plan(&p.guid)?;
        }
        Ok(())
    }
    fn verify(&self, after: &Value, _: &Value) -> bool {
        after.get("name").and_then(|n| n.as_str()) == Some(GAMING_PLAN_NAME)
    }
    fn verify_restored(&self, now: &Value, _: &Value) -> bool {
        now.get("name").and_then(|n| n.as_str()) != Some(GAMING_PLAN_NAME)
    }
    fn describe(&self, v: &Value) -> String {
        plan_name(v)
    }
}

/// One plugged-in setting of the Arcadia Gaming plan.
struct PowerValue {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    subgroup: &'static str,
    setting: &'static str,
    target: u32,
    trade_off: &'static str,
    show: fn(u32) -> String,
}

impl PowerValue {
    fn write(&self, ctx: &Ctx, value: u32) -> anyhow::Result<()> {
        let Some(plan) = gaming_plan(ctx) else { return Ok(()) };
        ctx.ops.write_power_value(&plan.guid, self.subgroup, self.setting, value)?;
        // A change to the active plan only takes effect when the plan is activated again.
        if plan.active {
            ctx.ops.set_active_power_plan(&plan.guid)?;
        }
        Ok(())
    }
}

impl Action for PowerValue {
    fn meta(&self) -> ActionMeta {
        ActionMeta {
            id: self.id.into(),
            name: self.name.into(),
            description: self.description.into(),
            category: Category::Power,
            risk: Risk::Medium,
            level: 2,
            restart: Restart::None,
            reversible: true,
            temporary: false,
            trade_off: self.trade_off.into(),
            source: "Win32 power management API (PowerWriteACValueIndex) with setting ids from the Windows SDK. Written to the Arcadia Gaming plan only.".into(),
            hardware: "Any PC on mains power".into(),
            os: WIN10.into(),
        }
    }
    fn applicable(&self, ctx: &Ctx, _: &Value) -> Result<(), String> {
        gaming_plan(ctx).map(|_| ()).ok_or_else(|| format!("Create the {GAMING_PLAN_NAME} power plan first. Arcadia only tunes its own plan, never one of Windows' plans."))
    }
    fn read(&self, ctx: &Ctx, _: &Value) -> anyhow::Result<Value> {
        match gaming_plan(ctx) {
            Some(p) => Ok(json!(ctx.ops.read_power_value(&p.guid, self.subgroup, self.setting)?)),
            None => Ok(Value::Null),
        }
    }
    fn proposed(&self, _: &Ctx, _: &Value) -> anyhow::Result<Value> {
        Ok(json!(self.target))
    }
    fn apply(&self, ctx: &Ctx, _: &Value, _: &Value, _: &Value) -> anyhow::Result<()> {
        self.write(ctx, self.target)
    }
    fn rollback(&self, ctx: &Ctx, _: &Value, before: &Value) -> anyhow::Result<()> {
        match before.as_u64() {
            Some(v) => self.write(ctx, v as u32),
            None => Ok(()),
        }
    }
    fn verify_restored(&self, now: &Value, before: &Value) -> bool {
        // The plan may already have been removed; then there is nothing left to restore.
        now.is_null() || now == before
    }
    fn describe(&self, v: &Value) -> String {
        match v.as_u64() {
            Some(n) => (self.show)(n as u32),
            None => "plan not present".into(),
        }
    }
}

/// A per-user DWORD that the Windows Settings app also writes.
struct UserDword {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    path: &'static str,
    value: &'static str,
    target: u32,
    /// What Windows does when the value is absent.
    absent: Option<u32>,
    trade_off: &'static str,
    risk: Risk,
}

impl UserDword {
    fn effective(&self, v: &Value) -> Option<u64> {
        v.as_u64().or(self.absent.map(u64::from))
    }
}

impl Action for UserDword {
    fn meta(&self) -> ActionMeta {
        ActionMeta {
            id: self.id.into(),
            name: self.name.into(),
            description: self.description.into(),
            category: Category::Windows,
            risk: self.risk,
            level: 2,
            restart: Restart::Game,
            reversible: true,
            temporary: false,
            trade_off: self.trade_off.into(),
            source: format!("Per-user Windows setting, the value the Settings app writes (HKEY_CURRENT_USER\\{}\\{}). The same switch is under Settings > Gaming.", self.path, self.value),
            hardware: "Any".into(),
            os: WIN10.into(),
        }
    }
    fn applicable(&self, ctx: &Ctx, _: &Value) -> Result<(), String> {
        ctx.inv.system.is_windows.then_some(()).ok_or_else(|| "Windows only".to_string())
    }
    fn read(&self, ctx: &Ctx, _: &Value) -> anyhow::Result<Value> {
        Ok(ctx.ops.user_u32(self.path, self.value).map(|v| json!(v)).unwrap_or(Value::Null))
    }
    fn proposed(&self, _: &Ctx, _: &Value) -> anyhow::Result<Value> {
        Ok(json!(self.target))
    }
    fn apply(&self, ctx: &Ctx, _: &Value, _: &Value, _: &Value) -> anyhow::Result<()> {
        ctx.ops.set_user_u32(self.path, self.value, self.target)
    }
    fn rollback(&self, ctx: &Ctx, _: &Value, before: &Value) -> anyhow::Result<()> {
        match before.as_u64() {
            Some(v) => ctx.ops.set_user_u32(self.path, self.value, v as u32),
            None => ctx.ops.delete_user_value(self.path, self.value),
        }
    }
    fn verify(&self, after: &Value, proposed: &Value) -> bool {
        self.effective(after).is_some() && self.effective(after) == proposed.as_u64()
    }
    fn describe(&self, v: &Value) -> String {
        let word = |n: u64| if n != 0 { "On" } else { "Off" };
        match (v.as_u64(), self.absent) {
            (Some(n), _) => word(n).into(),
            (None, Some(d)) => format!("{} (Windows default)", word(d.into())),
            (None, None) => "Not set".into(),
        }
    }
}

struct StartupDisable;

fn param_str<'a>(params: &'a Value, key: &str) -> Result<&'a str, String> {
    params.get(key).and_then(|v| v.as_str()).filter(|s| !s.is_empty()).ok_or_else(|| format!("missing '{key}'"))
}

impl Action for StartupDisable {
    fn meta(&self) -> ActionMeta {
        ActionMeta {
            id: "startup.disable".into(),
            name: "Stop an app from starting with Windows".into(),
            description: "Removes the app's entry from your account's start-up list. The app itself is not changed and can still be opened normally.".into(),
            category: Category::Startup,
            risk: Risk::Low,
            level: 2,
            restart: Restart::SignOut,
            reversible: true,
            temporary: false,
            trade_off: "The app no longer opens by itself after sign-in (no background updates or notifications until you open it).".into(),
            source: "The documented per-user Run key (HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Run). The exact command is saved and written back on revert.".into(),
            hardware: "Any".into(),
            os: WIN10.into(),
        }
    }
    fn instances(&self, ctx: &Ctx) -> Instances {
        Instances::Many(
            ctx.inv
                .system
                .startup_apps
                .iter()
                .filter(|a| a.scope == "HKCU" && ctx.ops.user_string(RUN_KEY, &a.name).is_some())
                .map(|a| (json!({ "name": a.name }), a.name.clone()))
                .collect(),
        )
    }
    fn key(&self, params: &Value) -> String {
        format!("startup.disable:{}", params.get("name").and_then(|n| n.as_str()).unwrap_or_default().to_lowercase())
    }
    fn applicable(&self, ctx: &Ctx, params: &Value) -> Result<(), String> {
        let name = param_str(params, "name")?;
        if name.eq_ignore_ascii_case("Arcadia") {
            return Err("Arcadia's own start-up entry is managed in Settings".into());
        }
        ctx.ops.user_string(RUN_KEY, name).map(|_| ()).ok_or_else(|| format!("'{name}' is not in your account's start-up list. Entries set for all users need an administrator and are not changed by Arcadia."))
    }
    fn read(&self, ctx: &Ctx, params: &Value) -> anyhow::Result<Value> {
        let name = param_str(params, "name").map_err(|e| anyhow::anyhow!(e))?;
        Ok(ctx.ops.user_string(RUN_KEY, name).map(Value::String).unwrap_or(Value::Null))
    }
    fn proposed(&self, _: &Ctx, _: &Value) -> anyhow::Result<Value> {
        Ok(Value::Null)
    }
    fn apply(&self, ctx: &Ctx, params: &Value, _: &Value, _: &Value) -> anyhow::Result<()> {
        ctx.ops.delete_user_value(RUN_KEY, param_str(params, "name").map_err(|e| anyhow::anyhow!(e))?)
    }
    fn rollback(&self, ctx: &Ctx, params: &Value, before: &Value) -> anyhow::Result<()> {
        let name = param_str(params, "name").map_err(|e| anyhow::anyhow!(e))?;
        match before.as_str() {
            Some(cmd) => ctx.ops.set_user_string(RUN_KEY, name, cmd),
            None => Ok(()),
        }
    }
    fn describe(&self, v: &Value) -> String {
        if v.is_null() { "Does not start with Windows".into() } else { "Starts with Windows".into() }
    }
}

/// Is this an absolute Windows path to an .exe with no traversal?
pub fn valid_exe_path(p: &str) -> bool {
    let b = p.as_bytes();
    p.len() < 260
        && b.len() > 7
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && b[2] == b'\\'
        && p.to_ascii_lowercase().ends_with(".exe")
        && !p.split('\\').any(|part| part == ".." || part == ".")
        && !p.chars().any(|c| c.is_control() || matches!(c, '/' | '"' | '<' | '>' | '|' | '?' | '*'))
        && !p[2..].contains(':')
}

fn gpu_pref_text(v: Option<&str>) -> String {
    let pref = v.and_then(|s| s.split(';').find_map(|kv| kv.trim().strip_prefix("GpuPreference="))).unwrap_or("0");
    match pref.trim() {
        "2" => "High performance GPU".into(),
        "1" => "Power saving GPU".into(),
        _ => "Let Windows decide".into(),
    }
}

struct GpuPreference;

impl Action for GpuPreference {
    fn meta(&self) -> ActionMeta {
        ActionMeta {
            id: "graphics.gpu_preference".into(),
            name: "Run this game on the high-performance GPU".into(),
            description: "On PCs with two graphics adapters, tells Windows to start this game on the discrete one. This is the per-app choice under Settings > System > Display > Graphics.".into(),
            category: Category::Graphics,
            risk: Risk::Low,
            level: 2,
            restart: Restart::Game,
            reversible: true,
            temporary: false,
            trade_off: "Higher power use while the game runs.".into(),
            source: "Per-app GPU preference of Windows graphics settings (HKEY_CURRENT_USER\\Software\\Microsoft\\DirectX\\UserGpuPreferences), honoured by DXGI adapter enumeration by GPU preference.".into(),
            hardware: "PCs with an integrated and a discrete GPU".into(),
            os: "Windows 10 version 1803 and later".into(),
        }
    }
    fn instances(&self, _: &Ctx) -> Instances {
        Instances::OnDemand
    }
    fn key(&self, params: &Value) -> String {
        format!("graphics.gpu_preference:{}", params.get("exe").and_then(|n| n.as_str()).unwrap_or_default().to_lowercase())
    }
    fn applicable(&self, ctx: &Ctx, params: &Value) -> Result<(), String> {
        let exe = param_str(params, "exe")?;
        if !valid_exe_path(exe) {
            return Err("the game executable must be a full path to an .exe file".into());
        }
        let has = |k: GpuKind| ctx.inv.gpus.iter().any(|g| g.kind == k);
        if !(has(GpuKind::Integrated) && has(GpuKind::Discrete)) {
            return Err("This PC has one graphics adapter, so there is nothing to choose between".into());
        }
        Ok(())
    }
    fn read(&self, ctx: &Ctx, params: &Value) -> anyhow::Result<Value> {
        let exe = param_str(params, "exe").map_err(|e| anyhow::anyhow!(e))?;
        Ok(ctx.ops.user_string(GPU_PREF_KEY, exe).map(Value::String).unwrap_or(Value::Null))
    }
    fn proposed(&self, ctx: &Ctx, params: &Value) -> anyhow::Result<Value> {
        let current = self.read(ctx, params)?;
        // Keep any other per-app flags Windows stored in the same value.
        let mut parts: Vec<String> = current.as_str().unwrap_or_default().split(';').map(str::trim).filter(|s| !s.is_empty() && !s.starts_with("GpuPreference=")).map(String::from).collect();
        parts.push("GpuPreference=2".into());
        Ok(Value::String(format!("{};", parts.join(";"))))
    }
    fn apply(&self, ctx: &Ctx, params: &Value, _: &Value, proposed: &Value) -> anyhow::Result<()> {
        let exe = param_str(params, "exe").map_err(|e| anyhow::anyhow!(e))?;
        ctx.ops.set_user_string(GPU_PREF_KEY, exe, proposed.as_str().unwrap_or("GpuPreference=2;"))
    }
    fn rollback(&self, ctx: &Ctx, params: &Value, before: &Value) -> anyhow::Result<()> {
        let exe = param_str(params, "exe").map_err(|e| anyhow::anyhow!(e))?;
        match before.as_str() {
            Some(v) => ctx.ops.set_user_string(GPU_PREF_KEY, exe, v),
            None => ctx.ops.delete_user_value(GPU_PREF_KEY, exe),
        }
    }
    fn verify(&self, after: &Value, _: &Value) -> bool {
        gpu_pref_text(after.as_str()) == "High performance GPU"
    }
    fn describe(&self, v: &Value) -> String {
        gpu_pref_text(v.as_str())
    }
}

fn pid_of(params: &Value) -> Result<u32, String> {
    params.get("pid").and_then(|p| p.as_u64()).and_then(|p| u32::try_from(p).ok()).ok_or_else(|| "missing 'pid'".to_string())
}

/// The process a request targets, from the list taken just before the session.
fn target<'a>(ctx: &'a Ctx, params: &Value) -> Result<&'a ProcessInfo, String> {
    let pid = pid_of(params)?;
    ctx.processes.iter().find(|p| p.pid == pid).ok_or_else(|| "That process is no longer running".to_string())
}

fn running_text(ctx: &Ctx, params: &Value) -> anyhow::Result<Value> {
    let pid = pid_of(params).map_err(|e| anyhow::anyhow!(e))?;
    Ok(Value::String(if ctx.procs.is_running(pid) { "running" } else { "not running" }.into()))
}

fn process_meta(id: &str, name: &str, description: &str, level: u8, risk: Risk, trade_off: &str, source: &str) -> ActionMeta {
    ActionMeta {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        category: Category::Process,
        risk,
        level,
        restart: Restart::None,
        reversible: false,
        temporary: true,
        trade_off: trade_off.into(),
        source: source.into(),
        hardware: "Any".into(),
        os: WIN10.into(),
    }
}

struct ProcessClose;

impl Action for ProcessClose {
    fn meta(&self) -> ActionMeta {
        process_meta(
            "process.close",
            "Ask an app to close",
            "Sends the app the same close request as clicking its close button. The app can still ask you to save your work.",
            1,
            Risk::Low,
            "The app closes. Arcadia does not reopen it.",
            "WM_CLOSE posted to the app's visible windows.",
        )
    }
    fn instances(&self, _: &Ctx) -> Instances {
        Instances::OnDemand
    }
    fn key(&self, params: &Value) -> String {
        format!("process.close:{}", params.get("pid").and_then(|p| p.as_u64()).unwrap_or(0))
    }
    fn applicable(&self, ctx: &Ctx, params: &Value) -> Result<(), String> {
        let p = target(ctx, params)?;
        if let Some(r) = &p.protected_reason {
            return Err(format!("{} is protected ({r}) and is never closed by Arcadia", p.name));
        }
        if !p.closable {
            return Err(format!("{} is a game; Arcadia does not close games", p.name));
        }
        Ok(())
    }
    fn read(&self, ctx: &Ctx, params: &Value) -> anyhow::Result<Value> {
        running_text(ctx, params)
    }
    fn proposed(&self, _: &Ctx, _: &Value) -> anyhow::Result<Value> {
        Ok(Value::String("asked to close".into()))
    }
    fn apply(&self, ctx: &Ctx, params: &Value, _: &Value, _: &Value) -> anyhow::Result<()> {
        let pid = pid_of(params).map_err(|e| anyhow::anyhow!(e))?;
        if ctx.procs.request_close(pid)? == 0 {
            anyhow::bail!("it has no window to ask. If you are sure, use End process instead.");
        }
        Ok(())
    }
    fn rollback(&self, _: &Ctx, _: &Value, _: &Value) -> anyhow::Result<()> {
        Ok(())
    }
    fn verify(&self, _: &Value, _: &Value) -> bool {
        // The app may legitimately stay open to ask about unsaved work.
        true
    }
}

struct ProcessEnd;

impl Action for ProcessEnd {
    fn meta(&self) -> ActionMeta {
        process_meta(
            "process.end",
            "End a process",
            "Stops the process immediately, without giving it the chance to save. Only used after you confirm it for that process.",
            2,
            Risk::Medium,
            "Unsaved work in that app is lost.",
            "TerminateProcess, after explicit confirmation. Protected processes are refused.",
        )
    }
    fn instances(&self, _: &Ctx) -> Instances {
        Instances::OnDemand
    }
    fn key(&self, params: &Value) -> String {
        format!("process.end:{}", params.get("pid").and_then(|p| p.as_u64()).unwrap_or(0))
    }
    fn applicable(&self, ctx: &Ctx, params: &Value) -> Result<(), String> {
        ProcessClose.applicable(ctx, params)
    }
    fn read(&self, ctx: &Ctx, params: &Value) -> anyhow::Result<Value> {
        running_text(ctx, params)
    }
    fn proposed(&self, _: &Ctx, _: &Value) -> anyhow::Result<Value> {
        Ok(Value::String("not running".into()))
    }
    fn apply(&self, ctx: &Ctx, params: &Value, _: &Value, _: &Value) -> anyhow::Result<()> {
        let pid = pid_of(params).map_err(|e| anyhow::anyhow!(e))?;
        ctx.procs.terminate(pid)?;
        // Windows tears the process down asynchronously; give it a moment before the read-back.
        for _ in 0..40 {
            if !ctx.procs.is_running(pid) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        Ok(())
    }
    fn rollback(&self, _: &Ctx, _: &Value, _: &Value) -> anyhow::Result<()> {
        Ok(())
    }
}

struct ProcessPriority;

impl Action for ProcessPriority {
    fn meta(&self) -> ActionMeta {
        ActionMeta {
            reversible: true,
            ..process_meta(
                "process.priority",
                "Change a process's priority while you play",
                "Raises the game's priority class, or lowers a background app's, until the game exits or you revert it. Realtime is never used.",
                2,
                Risk::Medium,
                "A higher game priority can starve other apps (voice chat, streaming) when the processor is full. It does not help when the graphics card is the limit.",
                "SetPriorityClass with Idle, Below normal, Normal, Above normal or High.",
            )
        }
    }
    fn instances(&self, _: &Ctx) -> Instances {
        Instances::OnDemand
    }
    fn key(&self, params: &Value) -> String {
        format!("process.priority:{}", params.get("pid").and_then(|p| p.as_u64()).unwrap_or(0))
    }
    fn applicable(&self, ctx: &Ctx, params: &Value) -> Result<(), String> {
        let class = param_str(params, "class")?;
        check_priority(class).map_err(|e| e.to_string())?;
        let p = target(ctx, params)?;
        if p.category == ProcCategory::Protected {
            return Err(format!("{} is protected ({}) and is never re-prioritised", p.name, p.protected_reason.clone().unwrap_or_default()));
        }
        Ok(())
    }
    fn read(&self, ctx: &Ctx, params: &Value) -> anyhow::Result<Value> {
        let pid = pid_of(params).map_err(|e| anyhow::anyhow!(e))?;
        Ok(ctx.procs.priority(pid).map(Value::String).unwrap_or(Value::Null))
    }
    fn proposed(&self, _: &Ctx, params: &Value) -> anyhow::Result<Value> {
        Ok(Value::String(param_str(params, "class").map_err(|e| anyhow::anyhow!(e))?.to_string()))
    }
    fn apply(&self, ctx: &Ctx, params: &Value, _: &Value, proposed: &Value) -> anyhow::Result<()> {
        ctx.procs.set_priority(pid_of(params).map_err(|e| anyhow::anyhow!(e))?, proposed.as_str().unwrap_or("Normal"))
    }
    fn rollback(&self, ctx: &Ctx, params: &Value, before: &Value) -> anyhow::Result<()> {
        let pid = pid_of(params).map_err(|e| anyhow::anyhow!(e))?;
        if !ctx.procs.is_running(pid) {
            return Ok(()); // The process exited; its priority went with it.
        }
        match before.as_str().filter(|c| check_priority(c).is_ok()) {
            Some(class) => ctx.procs.set_priority(pid, class),
            None => ctx.procs.set_priority(pid, "Normal"),
        }
    }
    fn verify_restored(&self, now: &Value, before: &Value) -> bool {
        now.is_null() || now == before || now.as_str() == Some("Normal")
    }
    fn describe(&self, v: &Value) -> String {
        match v.as_str() {
            Some("BelowNormal") => "Below normal".into(),
            Some("AboveNormal") => "Above normal".into(),
            Some(s) => s.into(),
            None => "not running".into(),
        }
    }
}

/// A vendor driver control, applied through the engine so it is recorded and reversible.
struct GpuControlAction;

impl GpuControlAction {
    fn find(ctx: &Ctx, params: &Value) -> Result<(gpu::GpuDevice, gpu::GpuControl), String> {
        let gpu_id = param_str(params, "gpu")?;
        let control_id = param_str(params, "control")?;
        let device = ctx.gpus.devices().into_iter().find(|d| d.id == gpu_id).ok_or_else(|| "That graphics adapter is no longer present".to_string())?;
        let control = ctx.gpus.controls(&device).into_iter().find(|c| c.id == control_id).ok_or_else(|| format!("{} does not offer the control '{control_id}' for {}", device.vendor.api_name(), device.name))?;
        Ok((device, control))
    }

    fn value_json(control: &gpu::GpuControl, value: Option<f64>) -> Value {
        let text = match (value, &control.kind) {
            (None, _) => "unknown".to_string(),
            (Some(v), ControlKind::Toggle) => if v != 0.0 { "On" } else { "Off" }.to_string(),
            (Some(v), ControlKind::Range { unit, .. }) => format!("{} {unit}", trim_float(v)).trim().to_string(),
            (Some(v), ControlKind::Choice { options }) => options.iter().find(|o| o.value == v).map(|o| o.label.clone()).unwrap_or_else(|| trim_float(v)),
            (Some(v), ControlKind::ReadOnly) => trim_float(v),
        };
        json!({ "value": value, "text": text })
    }
}

fn trim_float(v: f64) -> String {
    if v.fract() == 0.0 { format!("{v:.0}") } else { format!("{v:.2}") }
}

impl Action for GpuControlAction {
    fn meta(&self) -> ActionMeta {
        ActionMeta {
            id: "gpu.control".into(),
            name: "Change a graphics driver control".into(),
            description: "Sets one control the GPU vendor's own API exposes for this card, inside the limits the driver reports.".into(),
            category: Category::Driver,
            risk: Risk::High,
            level: 3,
            restart: Restart::None,
            reversible: true,
            temporary: false,
            trade_off: "Depends on the control; tuning controls can make the system unstable.".into(),
            source: "Vendor API (AMD ADLX or Intel IGCL). Values outside the driver-reported range are refused before they reach the driver.".into(),
            hardware: "GPUs whose vendor API reports the control as supported".into(),
            os: WIN10.into(),
        }
    }
    fn instances(&self, _: &Ctx) -> Instances {
        Instances::OnDemand
    }
    fn key(&self, params: &Value) -> String {
        format!("gpu.control:{}:{}", params.get("gpu").and_then(|v| v.as_str()).unwrap_or_default(), params.get("control").and_then(|v| v.as_str()).unwrap_or_default())
    }
    fn level(&self, ctx: &Ctx, params: &Value) -> u8 {
        Self::find(ctx, params).map(|(_, c)| c.level).unwrap_or(3)
    }
    fn risk(&self, ctx: &Ctx, params: &Value) -> Risk {
        Self::find(ctx, params).map(|(_, c)| c.risk).unwrap_or(Risk::High)
    }
    fn applicable(&self, ctx: &Ctx, params: &Value) -> Result<(), String> {
        let (device, control) = Self::find(ctx, params)?;
        let value = params.get("value").and_then(|v| v.as_f64()).ok_or_else(|| "missing 'value'".to_string())?;
        gpu::validate_control(&control, device.vendor, value).map_err(|e| e.to_string())
    }
    fn read(&self, ctx: &Ctx, params: &Value) -> anyhow::Result<Value> {
        let (_, control) = Self::find(ctx, params).map_err(|e| anyhow::anyhow!(e))?;
        Ok(Self::value_json(&control, control.value))
    }
    fn proposed(&self, ctx: &Ctx, params: &Value) -> anyhow::Result<Value> {
        let (_, control) = Self::find(ctx, params).map_err(|e| anyhow::anyhow!(e))?;
        Ok(Self::value_json(&control, params.get("value").and_then(|v| v.as_f64())))
    }
    fn apply(&self, ctx: &Ctx, params: &Value, _: &Value, proposed: &Value) -> anyhow::Result<()> {
        let (device, control) = Self::find(ctx, params).map_err(|e| anyhow::anyhow!(e))?;
        let value = proposed.get("value").and_then(|v| v.as_f64()).ok_or_else(|| anyhow::anyhow!("no value"))?;
        ctx.gpus.set_control(&device, &control.id, value)
    }
    fn rollback(&self, ctx: &Ctx, params: &Value, before: &Value) -> anyhow::Result<()> {
        let (device, control) = Self::find(ctx, params).map_err(|e| anyhow::anyhow!(e))?;
        let value = before.get("value").and_then(|v| v.as_f64()).or(control.default).ok_or_else(|| anyhow::anyhow!("the previous value was not readable, and the driver reports no default"))?;
        ctx.gpus.set_control(&device, &control.id, value)
    }
    fn verify(&self, after: &Value, proposed: &Value) -> bool {
        match (after.get("value").and_then(|v| v.as_f64()), proposed.get("value").and_then(|v| v.as_f64())) {
            (Some(a), Some(p)) => (a - p).abs() < 1e-6,
            _ => false,
        }
    }
    fn verify_restored(&self, now: &Value, before: &Value) -> bool {
        before.get("value").map(|v| v.is_null()).unwrap_or(true) || self.verify(now, before)
    }
    fn describe(&self, v: &Value) -> String {
        v.get("text").and_then(|t| t.as_str()).unwrap_or("unknown").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exe_paths_are_validated() {
        assert!(valid_exe_path(r"D:\Games\Thing\thing.exe"));
        assert!(!valid_exe_path(r"..\evil.exe"));
        assert!(!valid_exe_path(r"D:\Games\..\Windows\cmd.exe"));
        assert!(!valid_exe_path(r"D:\Games\thing.dll"));
        assert!(!valid_exe_path(r"D:/Games/thing.exe"));
        assert!(!valid_exe_path(r"\\server\share\thing.exe"));
        assert!(!valid_exe_path("D:\\Games\\thing.exe\" /c calc"));
        assert!(!valid_exe_path(r"D:\Games\thing.exe:stream.exe"));
    }

    #[test]
    fn every_action_documents_itself() {
        let mut ids = std::collections::HashSet::new();
        for a in standard() {
            let m = a.meta();
            assert!(ids.insert(m.id.clone()), "duplicate id {}", m.id);
            assert!((1..=3).contains(&m.level));
            for (what, text) in [("name", &m.name), ("description", &m.description), ("trade_off", &m.trade_off), ("source", &m.source), ("hardware", &m.hardware), ("os", &m.os)] {
                assert!(!text.trim().is_empty(), "{} has no {what}", m.id);
            }
            // Nothing in the registry reaches for the things FrameForge must never touch.
            let all = format!("{} {} {}", m.id, m.name, m.source).to_lowercase();
            for banned in ["defender", "bcdedit", "hpet", "services.msc", "core isolation", "windows update", "anticheat"] {
                assert!(!all.contains(banned), "{} mentions {banned}", m.id);
            }
        }
    }

    #[test]
    fn gpu_preference_text() {
        assert_eq!(gpu_pref_text(Some("GpuPreference=2;")), "High performance GPU");
        assert_eq!(gpu_pref_text(Some("SwapEffectUpgradeEnable=1;GpuPreference=1;")), "Power saving GPU");
        assert_eq!(gpu_pref_text(None), "Let Windows decide");
    }
}
