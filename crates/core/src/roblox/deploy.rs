//! Downloading and installing Roblox Player from Roblox's deployment CDN —
//! the same flow Bloxstrap/Fishstrap/Froststrap use.

use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

const VERSION_API: &str = "https://clientsettingscdn.roblox.com/v2/client-version/WindowsPlayer";
const SETUP_CDN: &str = "https://setup.rbxcdn.com";

/// Where each package zip unpacks inside a version folder.
pub const PACKAGE_MAP: &[(&str, &str)] = &[
    ("RobloxApp.zip", ""),
    ("redist.zip", ""),
    ("Libraries.zip", ""),
    ("WebView2.zip", ""),
    ("shaders.zip", "shaders/"),
    ("ssl.zip", "ssl/"),
    ("WebView2RuntimeInstaller.zip", "WebView2RuntimeInstaller/"),
    ("content-avatar.zip", "content/avatar/"),
    ("content-configs.zip", "content/configs/"),
    ("content-fonts.zip", "content/fonts/"),
    ("content-sky.zip", "content/sky/"),
    ("content-sounds.zip", "content/sounds/"),
    ("content-textures2.zip", "content/textures/"),
    ("content-models.zip", "content/models/"),
    ("content-platform-fonts.zip", "PlatformContent/pc/fonts/"),
    ("content-platform-dictionaries.zip", "PlatformContent/pc/shared_compression_dictionaries/"),
    ("content-terrain.zip", "PlatformContent/pc/terrain/"),
    ("content-textures3.zip", "PlatformContent/pc/textures/"),
    ("extracontent-luapackages.zip", "ExtraContent/LuaPackages/"),
    ("extracontent-translations.zip", "ExtraContent/translations/"),
    ("extracontent-models.zip", "ExtraContent/models/"),
    ("extracontent-textures.zip", "ExtraContent/textures/"),
    ("extracontent-places.zip", "ExtraContent/places/"),
];

const APP_SETTINGS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Settings>
	<ContentFolder>content</ContentFolder>
	<BaseUrl>http://www.roblox.com</BaseUrl>
</Settings>
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientVersion {
    pub version: String,
    pub client_version_upload: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Package {
    pub name: String,
    pub md5: String,
    pub packed_size: u64,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "stage", rename_all = "camelCase")]
pub enum Progress {
    Checking,
    Downloading { package: String, done: u64, total: u64 },
    Extracting { package: String, index: usize, count: usize },
    Finalizing,
    Ready { version: String },
}

/// Which version is installed and active (stored next to the versions).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InstallState {
    pub version: Option<String>,
    pub channel: Option<String>,
}

fn state_file() -> PathBuf {
    crate::paths::roblox_dir().join("state.json")
}

pub fn installed() -> InstallState {
    std::fs::read_to_string(state_file())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn version_dir(version: &str) -> PathBuf {
    crate::paths::roblox_versions_dir().join(version)
}

fn http() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("Arcadia/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("http client")
}

/// `ARCADIA_SETUP_CDN` points installs at a mirror (used by tests).
fn setup_cdn() -> String {
    std::env::var("ARCADIA_SETUP_CDN").unwrap_or_else(|_| SETUP_CDN.to_string())
}

fn channel_base(channel: &str) -> String {
    let cdn = setup_cdn();
    if channel.is_empty() || channel.eq_ignore_ascii_case("production") || channel.eq_ignore_ascii_case("live") {
        format!("{cdn}/")
    } else {
        format!("{cdn}/channel/{}/", channel.to_lowercase())
    }
}

pub async fn latest(channel: &str) -> Result<ClientVersion> {
    let url = if channel.is_empty() || channel.eq_ignore_ascii_case("production") {
        VERSION_API.to_string()
    } else {
        format!("{VERSION_API}/channel/{channel}")
    };
    let r = http().get(&url).send().await?.error_for_status()?;
    Ok(r.json().await?)
}

pub fn parse_manifest(src: &str) -> Result<Vec<Package>> {
    let mut lines = src.lines().map(str::trim).filter(|l| !l.is_empty());
    if lines.next() != Some("v0") {
        bail!("unsupported package manifest format");
    }
    let rest: Vec<&str> = lines.collect();
    if rest.len() % 4 != 0 {
        bail!("truncated package manifest");
    }
    rest.chunks(4)
        .map(|c| {
            Ok(Package {
                name: c[0].to_string(),
                md5: c[1].to_lowercase(),
                packed_size: c[2].parse()?,
                size: c[3].parse()?,
            })
        })
        .collect()
}

/// Join an archive entry under `base`, refusing absolute paths and `..`.
fn safe_join(base: &Path, entry: &str) -> Option<PathBuf> {
    let rel = PathBuf::from(entry.replace('\\', "/"));
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(base.join(rel))
}

pub fn extract_zip(zip_path: &Path, dest: &Path) -> Result<()> {
    let f = std::fs::File::open(zip_path)?;
    let mut z = zip::ZipArchive::new(f)?;
    for i in 0..z.len() {
        let mut e = z.by_index(i)?;
        let name = e.name().to_string();
        let Some(out) = safe_join(dest, &name) else { continue };
        if e.is_dir() || name.ends_with('/') || name.ends_with('\\') {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut w = std::fs::File::create(&out)?;
        std::io::copy(&mut e, &mut w)?;
    }
    Ok(())
}

async fn download(client: &reqwest::Client, url: &str, pkg: &Package, out: &Path, on: &(dyn Fn(Progress) + Send + Sync)) -> Result<()> {
    // Cached and intact? Skip the download.
    if let Ok(bytes) = std::fs::read(out) {
        if format!("{:x}", Md5::digest(&bytes)) == pkg.md5 {
            return Ok(());
        }
    }
    let tmp = out.with_extension("part");
    let mut last_err = None;
    for _attempt in 0..3 {
        let res = async {
            let r = client.get(url).send().await?.error_for_status()?;
            let mut f = std::fs::File::create(&tmp)?;
            let mut h = Md5::new();
            let mut done = 0u64;
            let mut stream = r.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                h.update(&chunk);
                f.write_all(&chunk)?;
                done += chunk.len() as u64;
                on(Progress::Downloading { package: pkg.name.clone(), done, total: pkg.packed_size });
            }
            f.flush()?;
            let got = format!("{:x}", h.finalize());
            if got != pkg.md5 {
                bail!("checksum mismatch for {} (expected {}, got {got})", pkg.name, pkg.md5);
            }
            anyhow::Ok(())
        }
        .await;
        match res {
            Ok(()) => {
                std::fs::rename(&tmp, out)?;
                return Ok(());
            }
            Err(e) => last_err = Some(e),
        }
    }
    let _ = std::fs::remove_file(&tmp);
    Err(last_err.unwrap())
}

/// Install `version` (a `version-xxxxxxxx` hash) from `channel` if it isn't
/// already present, then mark it active. Returns the version folder.
pub async fn install(version: &str, channel: &str, on: &(dyn Fn(Progress) + Send + Sync)) -> Result<PathBuf> {
    crate::paths::ensure_all()?;
    let dir = version_dir(version);
    if dir.join("RobloxPlayerBeta.exe").exists() && dir.join("AppSettings.xml").exists() {
        mark_active(version, channel)?;
        on(Progress::Ready { version: version.into() });
        return Ok(dir);
    }

    let client = http();
    let base = channel_base(channel);
    let manifest = client
        .get(format!("{base}{version}-rbxPkgManifest.txt"))
        .send()
        .await?
        .error_for_status()
        .context("fetching package manifest")?
        .text()
        .await?;
    let packages: Vec<Package> = parse_manifest(&manifest)?
        .into_iter()
        .filter(|p| PACKAGE_MAP.iter().any(|(n, _)| *n == p.name))
        .collect();
    if !packages.iter().any(|p| p.name == "RobloxApp.zip") {
        bail!("{version} is not a Windows Player build (no RobloxApp.zip)");
    }

    let staging = dir.with_extension("staging");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)?;
    let dl = crate::paths::roblox_downloads_dir();

    for p in &packages {
        let file = dl.join(&p.md5);
        download(&client, &format!("{base}{version}-{}", p.name), p, &file, on)
            .await
            .with_context(|| format!("downloading {}", p.name))?;
    }
    let count = packages.len();
    for (i, p) in packages.iter().enumerate() {
        on(Progress::Extracting { package: p.name.clone(), index: i + 1, count });
        let sub = PACKAGE_MAP.iter().find(|(n, _)| *n == p.name).map(|(_, d)| *d).unwrap_or("");
        let zip = dl.join(&p.md5);
        let dest = staging.join(sub);
        tokio::task::spawn_blocking(move || extract_zip(&zip, &dest)).await??;
    }

    on(Progress::Finalizing);
    std::fs::write(staging.join("AppSettings.xml"), APP_SETTINGS_XML)?;
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::rename(&staging, &dir)?;
    mark_active(version, channel)?;
    prune_old_versions(version);
    on(Progress::Ready { version: version.into() });
    Ok(dir)
}

fn mark_active(version: &str, channel: &str) -> Result<()> {
    let s = InstallState { version: Some(version.into()), channel: Some(channel.into()) };
    std::fs::write(state_file(), serde_json::to_vec_pretty(&s)?)?;
    Ok(())
}

/// Keep the active version only. Folders in use (locked) are left alone.
fn prune_old_versions(keep: &str) {
    if let Ok(rd) = std::fs::read_dir(crate::paths::roblox_versions_dir()) {
        for e in rd.flatten() {
            if e.file_name() != keep {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
}

/// Make sure a usable Roblox install exists, updating if allowed.
pub async fn ensure(settings: &crate::Settings, on: &(dyn Fn(Progress) + Send + Sync)) -> Result<PathBuf> {
    on(Progress::Checking);
    if let Some(pin) = &settings.roblox_pinned_version {
        return install(pin, &settings.roblox_channel, on).await;
    }
    let state = installed();
    let same_channel = state.channel.as_deref() == Some(settings.roblox_channel.as_str());
    let usable = state
        .version
        .clone()
        .filter(|v| same_channel && version_dir(v).join("RobloxPlayerBeta.exe").exists());

    if let (Some(v), false) = (&usable, settings.roblox_auto_update) {
        on(Progress::Ready { version: v.clone() });
        return Ok(version_dir(v));
    }
    match latest(&settings.roblox_channel).await {
        Ok(v) => install(&v.client_version_upload, &settings.roblox_channel, on).await,
        // Offline: fall back to what we already have.
        Err(e) => match usable {
            Some(v) => {
                on(Progress::Ready { version: v.clone() });
                Ok(version_dir(&v))
            }
            None => Err(e.context("could not reach Roblox to download the client")),
        },
    }
}

/// Wipe the managed install so the next launch downloads it fresh.
pub fn force_reinstall() -> Result<()> {
    let _ = std::fs::remove_dir_all(crate::paths::roblox_versions_dir());
    let _ = std::fs::remove_file(state_file());
    crate::paths::ensure_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manifest() {
        let m = "v0\nRobloxApp.zip\nABCDEF0123\n100\n200\nssl.zip\n00aa\n5\n10\n";
        let p = parse_manifest(m).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].md5, "abcdef0123");
        assert_eq!(p[1].size, 10);
        assert!(parse_manifest("v1\n").is_err());
    }

    #[test]
    fn refuses_zip_slip() {
        let b = Path::new("/base");
        assert!(safe_join(b, "../evil.dll").is_none());
        assert!(safe_join(b, "/abs").is_none());
        assert_eq!(safe_join(b, r"content\fonts\a.ttf").unwrap(), Path::new("/base/content/fonts/a.ttf"));
    }

    #[test]
    fn channel_urls() {
        assert_eq!(channel_base("production"), "https://setup.rbxcdn.com/");
        assert_eq!(channel_base("ZFlag"), "https://setup.rbxcdn.com/channel/zflag/");
    }
}
