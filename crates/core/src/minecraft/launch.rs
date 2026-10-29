//! Turn an instance + signed-in account into a running game.

use super::install::{self, LegacyAssets, Prepared};
use super::instance::{self, Instance};
use super::version::{self, VersionJson};
use super::{loaders, progress, OnProgress};
use anyhow::{bail, Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Who is playing. Comes from a real Microsoft sign-in (see `auth`).
#[derive(Debug, Clone)]
pub struct Session {
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    pub xuid: String,
    pub client_id: String,
}

/// Install the instance's loader if needed and return its version JSON.
pub async fn prepare_version(inst: &mut Instance, on: &OnProgress) -> Result<VersionJson> {
    let task = inst.id.clone();
    install::ensure_vanilla_json(&inst.mc_version).await?;
    let installed = inst.version_id.as_deref().map(|id| version::local_path(id).exists()).unwrap_or(false);
    if !installed {
        progress(on, &task, format!("Installing {}", inst.loader.label()), 0, 1);
        let lv = inst.loader_version.clone().unwrap_or_default();
        let id = loaders::install(inst.loader, &inst.mc_version, &lv, &task, on).await?;
        inst.version_id = Some(id);
        instance::save(inst)?;
    }
    version::load_resolved(inst.version_id.as_deref().unwrap())
}

pub struct Context2<'a> {
    pub game_dir: &'a Path,
    pub natives_dir: &'a Path,
    pub session: &'a Session,
    pub memory_mb: u32,
    pub extra_jvm: &'a str,
}

/// Split user JVM args on spaces, honouring double quotes.
pub fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in s.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Build the full Java command line (everything after the java executable).
pub fn build_args(v: &VersionJson, p: &Prepared, cx: &Context2) -> Result<Vec<String>> {
    let sep = if cfg!(windows) { ";" } else { ":" };
    let classpath = p.classpath.iter().map(|c| c.to_string_lossy().to_string()).collect::<Vec<_>>().join(sep);
    let assets_root = super::assets_dir();
    let game_assets = match &p.legacy_assets {
        Some(LegacyAssets::Virtual(d)) => d.clone(),
        Some(LegacyAssets::MapToResources(_)) => cx.game_dir.join("resources"),
        None => assets_root.clone(),
    };
    let s = |p: &Path| p.to_string_lossy().to_string();
    let mut vars: HashMap<&str, String> = HashMap::new();
    vars.insert("auth_player_name", cx.session.name.clone());
    vars.insert("version_name", v.id.clone());
    vars.insert("game_directory", s(cx.game_dir));
    vars.insert("assets_root", s(&assets_root));
    vars.insert("game_assets", s(&game_assets));
    vars.insert("assets_index_name", p.asset_index.clone());
    vars.insert("auth_uuid", cx.session.uuid.clone());
    vars.insert("auth_access_token", cx.session.access_token.clone());
    vars.insert("auth_session", format!("token:{}:{}", cx.session.access_token, cx.session.uuid));
    vars.insert("auth_xuid", cx.session.xuid.clone());
    vars.insert("clientid", cx.session.client_id.clone());
    vars.insert("user_type", "msa".into());
    vars.insert("user_properties", "{}".into());
    vars.insert("version_type", v.kind.clone().unwrap_or_else(|| "release".into()));
    vars.insert("natives_directory", s(cx.natives_dir));
    vars.insert("launcher_name", "Arcadia".into());
    vars.insert("launcher_version", env!("CARGO_PKG_VERSION").into());
    vars.insert("classpath", classpath);
    vars.insert("classpath_separator", sep.into());
    vars.insert("library_directory", s(&super::libraries_dir()));

    let features = HashSet::new();
    let mut args = vec![format!("-Xmx{}M", cx.memory_mb), format!("-Xms{}M", cx.memory_mb.min(1024))];
    // Belt and braces for Log4Shell alongside Mojang's patched logging config.
    args.push("-Dlog4j2.formatMsgNoLookups=true".into());
    if let Some(l) = &p.logging_arg {
        args.push(l.clone());
    }
    args.extend(split_args(cx.extra_jvm));

    let jvm: Vec<String> = match &v.arguments {
        Some(a) if !a.jvm.is_empty() => version::expand_args(&a.jvm, &features),
        _ => vec!["-Djava.library.path=${natives_directory}".into(), "-cp".into(), "${classpath}".into()],
    };
    args.extend(jvm.iter().map(|a| version::substitute(a, &vars)));

    let main = v.main_class.clone().context("version has no main class")?;
    args.push(main);

    let game: Vec<String> = match (&v.arguments, &v.minecraft_arguments) {
        (Some(a), _) if !a.game.is_empty() => version::expand_args(&a.game, &features),
        (_, Some(legacy)) => legacy.split_whitespace().map(String::from).collect(),
        _ => vec![],
    };
    args.extend(game.iter().map(|a| version::substitute(a, &vars)));

    if let Some(bad) = args.iter().find(|a| a.contains("${auth_access_token}")) {
        bail!("unexpanded argument {bad}");
    }
    Ok(args)
}

pub struct Ready {
    pub java: PathBuf,
    pub args: Vec<String>,
    pub game_dir: PathBuf,
}

/// Install everything and produce the command to run.
/// `join` connects to a server as soon as the game starts.
pub async fn prepare(inst: &mut Instance, session: &Session, join: Option<&str>, on: &OnProgress) -> Result<Ready> {
    if !inst.manual_downloads.is_empty() {
        let mods = instance::game_dir(&inst.id);
        let missing: Vec<_> = inst
            .manual_downloads
            .iter()
            .filter(|m| !mods.join(&m.folder).join(&m.file_name).exists())
            .map(|m| m.name.clone())
            .collect();
        if !missing.is_empty() {
            bail!("This pack still needs {} file(s) you have to download yourself: {}", missing.len(), missing.join(", "));
        }
    }
    let v = prepare_version(inst, on).await?;
    let task = inst.id.clone();
    let p = install::ensure_files(&v, &inst.mc_version, &task, on).await?;
    let java = match inst.java.as_str() {
        "custom" => {
            let p = PathBuf::from(inst.java_path.clone().unwrap_or_default());
            if !p.is_file() {
                bail!("The Java chosen for this instance isn't there any more: {}", p.display());
            }
            p
        }
        "" => super::java::ensure(&super::java::component_for(&v), &task, on).await?,
        c if super::java::RUNTIMES.iter().any(|(id, _)| *id == c) => super::java::ensure(c, &task, on).await?,
        other => bail!("Unknown Java choice {other}"),
    };

    let game_dir = instance::game_dir(&inst.id);
    std::fs::create_dir_all(&game_dir)?;
    let natives = instance::instance_dir(&inst.id).join("natives");
    let _ = std::fs::remove_dir_all(&natives);
    install::extract_natives(&p.native_jars, &natives)?;
    if let Some(LegacyAssets::MapToResources(src)) = &p.legacy_assets {
        instance::copy_dir(src, &game_dir.join("resources"))?;
    }

    let mut args = build_args(&v, &p, &Context2 { game_dir: &game_dir, natives_dir: &natives, session, memory_mb: inst.memory_mb, extra_jvm: &inst.java_args })?;
    let join = join.map(str::to_string).or_else(|| (!inst.auto_join.trim().is_empty()).then(|| inst.auto_join.clone()));
    if let Some(addr) = join {
        args.extend(super::servers::join_args(&inst.mc_version, &addr));
    }
    progress(on, &task, "Starting Minecraft", 1, 1);
    Ok(Ready { java, args, game_dir })
}

/// Hide the access token when showing a command line.
pub fn redact(args: &[String], session: &Session) -> Vec<String> {
    args.iter().map(|a| a.replace(&session.access_token, "<token>")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn session() -> Session {
        Session { name: "Steve".into(), uuid: "uuid1".into(), access_token: "SECRET".into(), xuid: "x".into(), client_id: "cid".into() }
    }

    #[test]
    fn modern_and_legacy_argument_building() {
        let v: VersionJson = serde_json::from_value(json!({
            "id": "neoforge-21.1.77", "mainClass": "cpw.mods.bootstraplauncher.BootstrapLauncher", "type": "release",
            "arguments": {
                "game": ["--username", "${auth_player_name}", "--accessToken", "${auth_access_token}",
                         {"rules":[{"action":"allow","features":{"is_demo_user":true}}],"value":"--demo"}],
                "jvm": ["-Djava.library.path=${natives_directory}", "-p", "${library_directory}/a.jar${classpath_separator}${library_directory}/b.jar", "-cp", "${classpath}"]
            }
        }))
        .unwrap();
        let p = Prepared { classpath: vec!["/l/x.jar".into(), "/v/1.21.1.jar".into()], native_jars: vec![], asset_index: "17".into(), legacy_assets: None, logging_arg: Some("-Dlog4j.configurationFile=/a/client-1.12.xml".into()) };
        let s = session();
        let gd = PathBuf::from("/game");
        let nd = PathBuf::from("/natives");
        let a = build_args(&v, &p, &Context2 { game_dir: &gd, natives_dir: &nd, session: &s, memory_mb: 6144, extra_jvm: "-XX:+UseG1GC \"-Dname=a b\"" }).unwrap();
        assert_eq!(a[0], "-Xmx6144M");
        assert!(a.contains(&"-Dlog4j2.formatMsgNoLookups=true".to_string()));
        assert!(a.contains(&"-Dname=a b".to_string()));
        assert!(a.contains(&"-Djava.library.path=/natives".to_string()));
        assert!(!a.iter().any(|x| x == "--demo"));
        let main = a.iter().position(|x| x == "cpw.mods.bootstraplauncher.BootstrapLauncher").unwrap();
        assert_eq!(&a[main + 1..main + 5], &["--username", "Steve", "--accessToken", "SECRET"]);
        assert!(!redact(&a, &s).iter().any(|x| x.contains("SECRET")));

        let old: VersionJson = serde_json::from_value(json!({
            "id": "1.7.10", "mainClass": "net.minecraft.client.main.Main",
            "minecraftArguments": "--username ${auth_player_name} --session ${auth_session} --assetsDir ${game_assets}"
        }))
        .unwrap();
        let p2 = Prepared { legacy_assets: Some(LegacyAssets::Virtual("/assets/virtual/legacy".into())), ..p };
        let a = build_args(&old, &p2, &Context2 { game_dir: &gd, natives_dir: &nd, session: &s, memory_mb: 2048, extra_jvm: "" }).unwrap();
        assert!(a.contains(&"token:SECRET:uuid1".to_string()));
        assert!(a.contains(&"/assets/virtual/legacy".to_string()));
        assert!(a.contains(&"-cp".to_string()));
    }
}
