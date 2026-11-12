//! Process list, classification and the few process controls FrameForge
//! offers (graceful close, user-approved end, temporary priority).
//!
//! Safety rules enforced here, not in the UI:
//! * protected processes (Windows core, security software, anti-cheat,
//!   drivers, Arcadia itself) can never be closed, ended or re-prioritised;
//! * priority is limited to Idle…High; REALTIME is rejected;
//! * closing asks first (WM_CLOSE). Ending a process is a separate call that
//!   the UI only makes after the user confirms it for that process.

use crate::library::{Game, LaunchTarget};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Game,
    Launcher,
    Browser,
    Overlay,
    Chat,
    /// Windows, drivers, security software, anti-cheat: never touched.
    Protected,
    App,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub exe: Option<String>,
    pub cpu_pct: f64,
    pub memory_bytes: u64,
    /// Share of the busiest GPU's 3D engine, when Windows reports it.
    pub gpu_pct: Option<f64>,
    pub owner: Option<String>,
    pub category: Category,
    /// Library game this process belongs to.
    pub game_id: Option<String>,
    pub game_name: Option<String>,
    /// Why it may matter for performance, in plain words. Empty when it doesn't.
    pub relevance: String,
    /// Arcadia may ask it to close.
    pub closable: bool,
    /// Why not, when `closable` is false.
    pub protected_reason: Option<String>,
}

/// Names that are never closed, ended or re-prioritised. Lower-case.
const PROTECTED_NAMES: &[&str] = &[
    // Windows core
    "system", "system idle process", "registry", "memory compression", "secure system", "smss.exe", "csrss.exe", "wininit.exe", "winlogon.exe",
    "services.exe", "lsass.exe", "lsaiso.exe", "svchost.exe", "dwm.exe", "fontdrvhost.exe", "explorer.exe", "sihost.exe", "ctfmon.exe",
    "conhost.exe", "runtimebroker.exe", "taskhostw.exe", "dllhost.exe", "audiodg.exe", "spoolsv.exe", "searchhost.exe", "searchindexer.exe",
    "startmenuexperiencehost.exe", "shellexperiencehost.exe", "textinputhost.exe", "applicationframehost.exe", "wudfhost.exe", "dashost.exe",
    "wmiprvse.exe", "unsecapp.exe", "userinit.exe", "logonui.exe", "taskmgr.exe", "trustedinstaller.exe", "tiworker.exe", "wuauclt.exe", "usoclient.exe",
    "mousocoreworker.exe", "ntoskrnl.exe", "crashpad_handler.exe",
    // Security
    "msmpeng.exe", "nissrv.exe", "mpdefendercoreservice.exe", "securityhealthservice.exe", "securityhealthsystray.exe", "smartscreen.exe", "sgrmbroker.exe",
    "msseces.exe", "mssense.exe",
    // Anti-cheat
    "vgc.exe", "vgtray.exe", "easyanticheat.exe", "easyanticheat_eos.exe", "beservice.exe", "beservice_x64.exe", "faceitservice.exe", "faceit.exe",
    "esportal.exe", "gameguard.des", "xigncode.exe", "atvi-crowdstrike.exe", "ricochet.exe",
    // GPU and audio drivers
    "nvcontainer.exe", "nvdisplay.container.exe", "nvsphelper64.exe", "atiesrxx.exe", "atieclxx.exe", "amdrsserv.exe", "amdfendrsr.exe", "igfxcuiservice.exe",
    "igfxem.exe", "intelgraphicssoftware.service.exe", "rtkauduservice64.exe", "nahimicservice.exe",
    // Measurement
    "presentmonservice.exe",
];

const LAUNCHERS: &[&str] = &[
    "steam.exe", "steamwebhelper.exe", "steamservice.exe", "epicgameslauncher.exe", "epicwebhelper.exe", "battle.net.exe", "agent.exe", "galaxyclient.exe",
    "galaxyclient helper.exe", "eadesktop.exe", "eabackgroundservice.exe", "origin.exe", "upc.exe", "ubisoftconnect.exe", "uplaywebcore.exe",
    "riotclientservices.exe", "riotclientux.exe", "rockstarservice.exe", "launcher.exe", "xboxpcapp.exe", "gamingservices.exe",
];
const BROWSERS: &[&str] = &["chrome.exe", "msedge.exe", "firefox.exe", "opera.exe", "opera_gx.exe", "brave.exe", "vivaldi.exe", "arc.exe", "msedgewebview2.exe"];
const OVERLAYS: &[&str] = &[
    "obs64.exe", "obs32.exe", "streamlabs obs.exe", "gamebar.exe", "gamebarftserver.exe", "gamebarpresencewriter.exe", "nvidia share.exe", "nvidia overlay.exe",
    "nvidia app.exe", "radeonsoftware.exe", "amdow.exe", "rtss.exe", "msiafterburner.exe", "overwolf.exe", "medal.exe", "xsplit.core.exe", "action.exe",
    "bandicam.exe", "fraps.exe", "playnite.desktopapp.exe",
];
const CHAT: &[&str] = &["discord.exe", "discordptb.exe", "discordcanary.exe", "teams.exe", "ms-teams.exe", "slack.exe", "skype.exe", "telegram.exe", "whatsapp.exe", "zoom.exe", "spotify.exe"];

/// Is this process one FrameForge must leave alone? Returns the reason.
pub fn protection_reason(pid: u32, name: &str, exe: Option<&str>, owner: Option<&str>, own_pid: u32) -> Option<String> {
    let n = name.to_ascii_lowercase();
    if pid <= 4 {
        return Some("Windows kernel process".into());
    }
    if pid == own_pid {
        return Some("This is Arcadia".into());
    }
    if PROTECTED_NAMES.contains(&n.as_str()) {
        return Some("Windows, driver, security or anti-cheat component".into());
    }
    if n.contains("anticheat") || n.contains("anti-cheat") || n.starts_with("vgk") || n.starts_with("easyanticheat") || n.starts_with("battleye") {
        return Some("Anti-cheat component".into());
    }
    if let Some(o) = owner {
        let o = o.to_ascii_lowercase();
        if ["system", "local service", "network service", "nt authority\\system", "trustedinstaller"].iter().any(|s| o == *s || o.ends_with(&format!("\\{s}"))) || o.starts_with("dwm-") || o.starts_with("umfd-") {
            return Some("Runs as a Windows system account".into());
        }
    }
    if let Some(e) = exe {
        let e = e.to_ascii_lowercase().replace('/', "\\");
        if e.contains("\\windows\\system32\\") || e.contains("\\windows\\syswow64\\") || e.contains("\\windows\\systemapps\\") || e.contains("\\windows defender\\") {
            return Some("Part of Windows".into());
        }
    }
    None
}

pub fn categorize(name: &str) -> Category {
    let n = name.to_ascii_lowercase();
    let n = n.as_str();
    if LAUNCHERS.contains(&n) {
        Category::Launcher
    } else if BROWSERS.contains(&n) {
        Category::Browser
    } else if OVERLAYS.contains(&n) {
        Category::Overlay
    } else if CHAT.contains(&n) {
        Category::Chat
    } else {
        Category::App
    }
}

/// Library games indexed for matching against running processes.
#[derive(Debug, Clone, Default)]
pub struct GameIndex {
    /// (game id, name, lower-case install dir with trailing separator, lower-case exe path)
    entries: Vec<(String, String, Option<String>, Option<String>)>,
}

fn norm(p: &str) -> String {
    p.to_lowercase().replace('/', "\\")
}

impl GameIndex {
    pub fn new(games: &[Game]) -> Self {
        let entries = games
            .iter()
            .map(|g| {
                let dir = g.install_dir.as_deref().filter(|d| d.len() > 3).map(|d| {
                    let mut d = norm(d);
                    if !d.ends_with('\\') {
                        d.push('\\');
                    }
                    d
                });
                let exe = match &g.launch {
                    LaunchTarget::Exe { path, .. } => Some(norm(path)),
                    _ => None,
                };
                (g.id.clone(), g.name.clone(), dir, exe)
            })
            .collect();
        Self { entries }
    }

    /// The library game a process belongs to: exact executable first, then
    /// the longest install folder that contains it.
    pub fn game_for(&self, exe: &str) -> Option<(&str, &str)> {
        let e = norm(exe);
        if let Some(hit) = self.entries.iter().find(|(_, _, _, x)| x.as_deref() == Some(e.as_str())) {
            return Some((&hit.0, &hit.1));
        }
        self.entries
            .iter()
            .filter(|(_, _, dir, _)| dir.as_deref().map(|d| e.starts_with(d)).unwrap_or(false))
            .max_by_key(|(_, _, dir, _)| dir.as_ref().map(|d| d.len()).unwrap_or(0))
            .map(|(id, name, _, _)| (id.as_str(), name.as_str()))
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Executables inside a game's folder that are not the game itself.
const NOT_THE_GAME: &[&str] = &["crashreporter", "crashhandler", "crashpad", "unitycrashhandler", "launcher", "updater", "setup", "installer", "redist", "vc_redist", "dxsetup", "easyanticheat", "battleye", "beservice", "webhelper", "cefprocess", "subprocess"];

fn looks_like_helper(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    NOT_THE_GAME.iter().any(|h| n.contains(h))
}

pub struct ProcessLister {
    sys: sysinfo::System,
    users: sysinfo::Users,
    own_pid: u32,
    primed: bool,
}

impl ProcessLister {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self { sys: sysinfo::System::new()?, users: sysinfo::Users::new_with_refreshed_list()?, own_pid: std::process::id(), primed: false })
    }

    /// Refresh and list every process. `gpu_by_pid` comes from Windows' GPU
    /// counters when available. CPU usage needs two refreshes, so the first
    /// call reports 0% for everything.
    pub fn list(&mut self, games: &GameIndex, gpu_by_pid: &HashMap<u32, f64>) -> Vec<ProcessInfo> {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, UpdateKind};
        let kind = ProcessRefreshKind::nothing().with_cpu().with_memory().with_exe(UpdateKind::OnlyIfNotSet).with_user(UpdateKind::OnlyIfNotSet);
        self.sys.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
        self.sys.refresh_memory();
        self.primed = true;
        let cores = self.sys.cpus().len().max(std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)) as f64;
        let total_mem = self.sys.total_memory().max(1);
        let mut out: Vec<ProcessInfo> = self
            .sys
            .processes()
            .iter()
            .map(|(pid, p)| {
                let pid = pid.as_u32();
                let name = p.name().to_string_lossy().to_string();
                let exe = p.exe().map(|e| e.to_string_lossy().to_string()).filter(|e| !e.is_empty());
                let owner = p.user_id().and_then(|u| self.users.get_user_by_id(u)).map(|u| u.name().to_string());
                // sysinfo reports per-core percent; convert to percent of the whole CPU.
                let cpu_pct = (p.cpu_usage() as f64 / cores).clamp(0.0, 100.0);
                let memory_bytes = p.memory();
                let game = exe.as_deref().and_then(|e| games.game_for(e)).filter(|_| !looks_like_helper(&name));
                let protected = protection_reason(pid, &name, exe.as_deref(), owner.as_deref(), self.own_pid);
                let category = if protected.is_some() {
                    Category::Protected
                } else if game.is_some() {
                    Category::Game
                } else {
                    categorize(&name)
                };
                let gpu_pct = gpu_by_pid.get(&pid).copied();
                let mut why = Vec::new();
                if category != Category::Game && category != Category::Protected {
                    if cpu_pct >= 5.0 {
                        why.push(format!("using {cpu_pct:.0}% of the CPU"));
                    }
                    if memory_bytes as f64 / total_mem as f64 >= 0.05 {
                        why.push(format!("holding {:.1} GB of memory", memory_bytes as f64 / (1u64 << 30) as f64));
                    }
                    if gpu_pct.unwrap_or(0.0) >= 5.0 {
                        why.push(format!("using {:.0}% of the GPU", gpu_pct.unwrap_or(0.0)));
                    }
                    if category == Category::Overlay {
                        why.push("draws an overlay or records the screen".into());
                    }
                }
                ProcessInfo {
                    pid,
                    game_id: game.map(|g| g.0.to_string()),
                    game_name: game.map(|g| g.1.to_string()),
                    relevance: why.join(", "),
                    closable: protected.is_none() && category != Category::Game,
                    protected_reason: protected,
                    name,
                    exe,
                    cpu_pct,
                    memory_bytes,
                    gpu_pct,
                    owner,
                    category,
                }
            })
            .collect();
        out.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes));
        out
    }

    pub fn is_running(&mut self, pid: u32) -> bool {
        use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate};
        let pid = Pid::from_u32(pid);
        self.sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), true, ProcessRefreshKind::nothing());
        self.sys.process(pid).is_some()
    }
}

/// The running process most likely to be the game: a library match with the
/// highest GPU share, else the most memory.
pub fn pick_running_game(list: &[ProcessInfo]) -> Option<&ProcessInfo> {
    list.iter().filter(|p| p.category == Category::Game).max_by(|a, b| {
        let key = |p: &ProcessInfo| (p.gpu_pct.unwrap_or(0.0), p.memory_bytes as f64);
        key(a).partial_cmp(&key(b)).unwrap_or(std::cmp::Ordering::Equal)
    })
}

/// Process controls, behind a trait so the optimiser can be tested without
/// touching real processes.
pub trait ProcessOps: Send + Sync {
    /// Ask the process's windows to close. Returns how many windows were asked.
    fn request_close(&self, pid: u32) -> anyhow::Result<u32>;
    fn terminate(&self, pid: u32) -> anyhow::Result<()>;
    fn priority(&self, pid: u32) -> anyhow::Result<String>;
    fn set_priority(&self, pid: u32, class: &str) -> anyhow::Result<()>;
    fn is_running(&self, pid: u32) -> bool;
}

pub const ALLOWED_PRIORITIES: &[&str] = &["Idle", "BelowNormal", "Normal", "AboveNormal", "High"];

/// Reject priorities FrameForge will not set (REALTIME above all).
pub fn check_priority(class: &str) -> anyhow::Result<()> {
    if class.eq_ignore_ascii_case("realtime") {
        anyhow::bail!("Realtime priority is never set: it can starve Windows itself and freeze the PC");
    }
    if !ALLOWED_PRIORITIES.contains(&class) {
        anyhow::bail!("'{class}' is not a priority class");
    }
    Ok(())
}

pub struct SystemProcessOps;

impl ProcessOps for SystemProcessOps {
    fn request_close(&self, pid: u32) -> anyhow::Result<u32> {
        #[cfg(windows)]
        {
            Ok(super::winsys::request_close(pid))
        }
        #[cfg(not(windows))]
        {
            let _ = pid;
            anyhow::bail!("Closing applications is only supported on Windows")
        }
    }
    fn terminate(&self, pid: u32) -> anyhow::Result<()> {
        #[cfg(windows)]
        {
            super::winsys::terminate(pid)
        }
        #[cfg(not(windows))]
        {
            let _ = pid;
            anyhow::bail!("Ending processes is only supported on Windows")
        }
    }
    fn priority(&self, pid: u32) -> anyhow::Result<String> {
        #[cfg(windows)]
        {
            super::winsys::get_priority(pid)
        }
        #[cfg(not(windows))]
        {
            let _ = pid;
            anyhow::bail!("Process priority is only supported on Windows")
        }
    }
    fn set_priority(&self, pid: u32, class: &str) -> anyhow::Result<()> {
        check_priority(class)?;
        #[cfg(windows)]
        {
            super::winsys::set_priority(pid, class)
        }
        #[cfg(not(windows))]
        {
            let _ = pid;
            anyhow::bail!("Process priority is only supported on Windows")
        }
    }
    fn is_running(&self, pid: u32) -> bool {
        #[cfg(windows)]
        {
            super::winsys::is_running(pid)
        }
        #[cfg(not(windows))]
        {
            let _ = pid;
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Source;

    fn game(id: &str, name: &str, dir: Option<&str>, exe: Option<&str>) -> Game {
        Game {
            id: id.into(),
            name: name.into(),
            source: Source::Steam,
            install_dir: dir.map(String::from),
            launch: match exe {
                Some(e) => LaunchTarget::Exe { path: e.into(), args: vec![], cwd: None },
                None => LaunchTarget::Uri { uri: "steam://rungameid/1".into() },
            },
            cover: None,
            hero: None,
        }
    }

    #[test]
    fn protected_processes_are_recognised() {
        let p = |pid, name: &str, exe: Option<&str>, owner: Option<&str>| protection_reason(pid, name, exe, owner, 9999);
        assert!(p(4, "System", None, None).is_some());
        assert!(p(900, "MsMpEng.exe", None, None).is_some());
        assert!(p(901, "vgc.exe", None, None).is_some());
        assert!(p(902, "EasyAntiCheat_EOS.exe", None, None).is_some());
        assert!(p(903, "SomeAntiCheatLauncher.exe", None, None).is_some());
        assert!(p(904, "weird.exe", Some(r"C:\Windows\System32\weird.exe"), None).is_some());
        assert!(p(905, "service.exe", Some(r"C:\Tools\service.exe"), Some("SYSTEM")).is_some());
        assert!(p(9999, "Arcadia.exe", None, None).unwrap().contains("Arcadia"));
        assert!(p(906, "chrome.exe", Some(r"C:\Program Files\Google\Chrome\Application\chrome.exe"), Some("sienna")).is_none());
    }

    #[test]
    fn categories() {
        assert_eq!(categorize("Steam.exe"), Category::Launcher);
        assert_eq!(categorize("msedge.exe"), Category::Browser);
        assert_eq!(categorize("obs64.exe"), Category::Overlay);
        assert_eq!(categorize("Discord.exe"), Category::Chat);
        assert_eq!(categorize("notepad.exe"), Category::App);
    }

    #[test]
    fn games_match_by_exe_then_longest_folder() {
        let idx = GameIndex::new(&[
            game("steam:1", "Outer", Some(r"D:\Games"), None),
            game("steam:2", "Inner", Some(r"D:\Games\Inner"), None),
            game("custom:x", "Custom", None, Some(r"E:\Stuff\game.exe")),
        ]);
        assert_eq!(idx.game_for(r"d:\games\inner\bin\Game-Win64.exe").unwrap().0, "steam:2");
        assert_eq!(idx.game_for(r"D:/Games/other/x.exe").unwrap().0, "steam:1");
        assert_eq!(idx.game_for(r"E:\Stuff\GAME.EXE").unwrap().0, "custom:x");
        assert!(idx.game_for(r"D:\GamesBackup\x.exe").is_none(), "a folder prefix must end at a separator");
        assert!(idx.game_for(r"C:\Windows\notepad.exe").is_none());
    }

    #[test]
    fn listing_this_machine_never_marks_ourselves_closable() {
        let mut l = ProcessLister::new().unwrap();
        let list = l.list(&GameIndex::default(), &HashMap::new());
        assert!(!list.is_empty());
        let me = list.iter().find(|p| p.pid == std::process::id()).expect("own process listed");
        assert!(!me.closable);
        assert_eq!(me.category, Category::Protected);
        assert!(l.is_running(std::process::id()));
        assert!(!l.is_running(u32::MAX - 7));
    }

    #[test]
    fn realtime_priority_is_refused() {
        assert!(check_priority("Realtime").unwrap_err().to_string().contains("never set"));
        assert!(check_priority("REALTIME").is_err());
        assert!(check_priority("Bogus").is_err());
        assert!(check_priority("High").is_ok());
        assert!(SystemProcessOps.set_priority(1, "Realtime").is_err());
    }

    #[test]
    fn picks_the_game_with_most_gpu_use() {
        let mk = |pid, cat, gpu: Option<f64>, mem| ProcessInfo {
            pid, name: format!("p{pid}"), exe: None, cpu_pct: 0.0, memory_bytes: mem, gpu_pct: gpu, owner: None, category: cat,
            game_id: None, game_name: None, relevance: String::new(), closable: false, protected_reason: None,
        };
        let list = vec![mk(1, Category::App, Some(99.0), 10), mk(2, Category::Game, Some(5.0), 900), mk(3, Category::Game, Some(80.0), 100)];
        assert_eq!(pick_running_game(&list).unwrap().pid, 3);
        assert!(pick_running_game(&list[..1]).is_none());
    }
}
