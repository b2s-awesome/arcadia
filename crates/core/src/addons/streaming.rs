//! Streaming & downloads addon: finds music online and fetches it with
//! yt-dlp (and ffmpeg when a conversion is asked for).
//!
//! "Streaming" fetches a song into a size-limited cache and plays it with
//! the native engine, so it gets gapless playback and full seeking.
//! Downloads go to the user's music folder and stay.
//!
//! Only what the sites offer for free is fetched. DRM-protected or
//! subscriber-only audio (e.g. SoundCloud Go+ full tracks) isn't touched;
//! those give the 30-second preview the site gives everyone.

use anyhow::{anyhow, bail, Context, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

pub const ID: &str = "streaming";

pub fn dir() -> PathBuf {
    super::root().join(ID)
}
pub fn bin_dir() -> PathBuf {
    dir().join("bin")
}
pub fn cache_dir() -> PathBuf {
    dir().join("cache")
}

/// Files the native engine can play.
const PLAYABLE: &[&str] = &["m4a", "mp4", "mp3", "flac", "wav", "ogg", "oga", "aac"];

pub fn is_playable(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| PLAYABLE.contains(&e.to_ascii_lowercase().as_str()))
}

// ───────────────────────── settings ─────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// What the site serves, untouched (AAC in M4A; MP3 kept as MP3).
    #[default]
    M4a,
    /// Re-encoded to MP3 320 kbps.
    Mp3,
    /// Re-wrapped as FLAC: same sound as the source, much bigger.
    Flac,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub format: Format,
    /// SoundCloud: take the artist's own upload when they allow downloads.
    pub sc_original: bool,
    /// Where downloads go. Empty: Music\Arcadia Downloads.
    pub download_dir: String,
    pub cache_limit_mb: u64,
    /// Use these instead of finding the tools.
    pub ytdlp_path: String,
    pub ffmpeg_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Config { format: Format::M4a, sc_original: true, download_dir: String::new(), cache_limit_mb: 1024, ytdlp_path: String::new(), ffmpeg_path: String::new() }
    }
}

impl Config {
    pub fn load() -> Config {
        super::get(ID, "config").ok().flatten().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
    }
    pub fn save(&self) -> Result<()> {
        super::set(ID, "config", Some(&serde_json::to_value(self)?))
    }
    pub fn download_dir(&self) -> PathBuf {
        if !self.download_dir.trim().is_empty() {
            return PathBuf::from(self.download_dir.trim());
        }
        let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join("Music").join("Arcadia Downloads")
    }
}

// ───────────────────────── tools ─────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tool {
    YtDlp,
    Ffmpeg,
}

impl Tool {
    fn exe(self) -> &'static str {
        match (self, cfg!(windows)) {
            (Tool::YtDlp, true) => "yt-dlp.exe",
            (Tool::YtDlp, false) => "yt-dlp",
            (Tool::Ffmpeg, true) => "ffmpeg.exe",
            (Tool::Ffmpeg, false) => "ffmpeg",
        }
    }
}

/// Where a tool is: the configured path, Arcadia's own copy, or on PATH.
pub fn find(tool: Tool, cfg: &Config) -> Option<PathBuf> {
    let custom = match tool {
        Tool::YtDlp => &cfg.ytdlp_path,
        Tool::Ffmpeg => &cfg.ffmpeg_path,
    };
    if !custom.trim().is_empty() {
        let p = PathBuf::from(custom.trim());
        return p.is_file().then_some(p);
    }
    let own = bin_dir().join(tool.exe());
    if own.is_file() {
        return Some(own);
    }
    std::env::var_os("PATH").and_then(|paths| std::env::split_paths(&paths).map(|d| d.join(tool.exe())).find(|p| p.is_file()))
}

fn command(exe: &Path) -> Command {
    let mut c = Command::new(exe);
    c.stdin(Stdio::null()).kill_on_drop(true);
    #[cfg(windows)]
    {
        // No console window flashing up.
        c.creation_flags(0x0800_0000);
    }
    c
}

pub async fn version(exe: &Path, tool: Tool) -> Option<String> {
    let arg = if tool == Tool::Ffmpeg { "-version" } else { "--version" };
    let out = tokio::time::timeout(Duration::from_secs(15), command(exe).arg(arg).output()).await.ok()?.ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let line = s.lines().next()?.trim().to_string();
    // "ffmpeg version N-1234-g… Copyright…" → "N-1234-g…"
    Some(match tool {
        Tool::Ffmpeg => line.split_whitespace().nth(2).unwrap_or(&line).to_string(),
        Tool::YtDlp => line,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgress {
    pub tool: Tool,
    pub done: u64,
    pub total: u64,
    pub stage: String,
}

const YTDLP_BASE: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download";
const FFMPEG_BASE: &str = "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest";

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder().user_agent(concat!("Arcadia/", env!("CARGO_PKG_VERSION"))).connect_timeout(Duration::from_secs(20)).build()?)
}

/// The expected SHA-256 for `name` from a `sha256sum`-style list.
pub fn checksum_for(list: &str, name: &str) -> Option<String> {
    list.lines().find_map(|l| {
        let mut it = l.split_whitespace();
        let hash = it.next()?;
        let file = it.next()?.trim_start_matches('*');
        (file == name && hash.len() == 64).then(|| hash.to_ascii_lowercase())
    })
}

async fn fetch_verified(url: &str, sums_url: &str, name: &str, tool: Tool, progress: &(dyn Fn(InstallProgress) + Send + Sync)) -> Result<Vec<u8>> {
    let c = client()?;
    let sums = c.get(sums_url).send().await?.error_for_status()?.text().await?;
    let want = checksum_for(&sums, name).ok_or_else(|| anyhow!("The release has no checksum for {name}, so it wasn't installed"))?;
    let r = c.get(url).send().await?.error_for_status()?;
    let total = r.content_length().unwrap_or(0);
    let mut buf = Vec::with_capacity(total as usize);
    let mut stream = r.bytes_stream();
    let mut last = 0u64;
    while let Some(chunk) = stream.next().await {
        buf.extend_from_slice(&chunk?);
        let done = buf.len() as u64;
        if done - last > 256 * 1024 || done == total {
            last = done;
            progress(InstallProgress { tool, done, total, stage: "downloading".into() });
        }
    }
    let got = hex::encode(Sha256::digest(&buf));
    if got != want {
        bail!("{name} didn't match its published checksum, so it wasn't installed. Try again later.");
    }
    Ok(buf)
}

fn write_exe(path: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::create_dir_all(path.parent().unwrap())?;
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Download a tool from its official GitHub release into Arcadia's own
/// folder, checking it against the release's published checksums.
pub async fn install(tool: Tool, progress: impl Fn(InstallProgress) + Send + Sync) -> Result<PathBuf> {
    match tool {
        Tool::YtDlp => {
            let name = if cfg!(windows) {
                "yt-dlp.exe"
            } else if cfg!(target_os = "macos") {
                "yt-dlp_macos"
            } else {
                "yt-dlp_linux"
            };
            let bytes = fetch_verified(&format!("{YTDLP_BASE}/{name}"), &format!("{YTDLP_BASE}/SHA2-256SUMS"), name, tool, &progress).await?;
            let dest = bin_dir().join(tool.exe());
            write_exe(&dest, &bytes)?;
            progress(InstallProgress { tool, done: 1, total: 1, stage: "done".into() });
            Ok(dest)
        }
        Tool::Ffmpeg => {
            if !cfg!(windows) {
                bail!("Install ffmpeg with your package manager (e.g. `sudo apt install ffmpeg`); Arcadia will find it.");
            }
            // The "shared" build is less than half the size; its DLLs sit beside ffmpeg.exe.
            let name = "ffmpeg-master-latest-win64-gpl-shared.zip";
            let bytes = fetch_verified(&format!("{FFMPEG_BASE}/{name}"), &format!("{FFMPEG_BASE}/checksums.sha256"), name, tool, &progress).await?;
            progress(InstallProgress { tool, done: 0, total: 0, stage: "unpacking".into() });
            let dest = tokio::task::spawn_blocking(move || unpack_ffmpeg(&bytes)).await??;
            progress(InstallProgress { tool, done: 1, total: 1, stage: "done".into() });
            Ok(dest)
        }
    }
}

/// Copy `bin/*` (ffmpeg, ffprobe and their DLLs) out of the build zip.
fn unpack_ffmpeg(zip_bytes: &[u8]) -> Result<PathBuf> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes))?;
    let out = bin_dir();
    std::fs::create_dir_all(&out)?;
    let mut found = false;
    for i in 0..z.len() {
        let mut f = z.by_index(i)?;
        let Some(name) = f.enclosed_name() else { continue };
        let parts: Vec<_> = name.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
        if parts.len() < 2 || parts[parts.len() - 2] != "bin" || f.is_dir() {
            continue;
        }
        let file = &parts[parts.len() - 1];
        let lower = file.to_ascii_lowercase();
        if !(lower.ends_with(".exe") || lower.ends_with(".dll")) || lower == "ffplay.exe" {
            continue;
        }
        let dest = out.join(file);
        let mut w = std::fs::File::create(&dest)?;
        std::io::copy(&mut f, &mut w)?;
        found |= lower == "ffmpeg.exe";
    }
    if !found {
        bail!("The ffmpeg download didn't contain ffmpeg.exe");
    }
    Ok(out.join("ffmpeg.exe"))
}

/// `yt-dlp -U` (only for Arcadia's own copy; other installs update their own way).
pub async fn update_ytdlp(exe: &Path) -> Result<String> {
    let out = command(exe).args(["-U", "--encoding", "utf-8"]).output().await?;
    let s = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    let last = s.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
    if !out.status.success() {
        bail!("{last}");
    }
    Ok(last)
}

// ───────────────────────── searching ─────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    YoutubeMusic,
    Soundcloud,
}

/// One thing found online: a song, or (for links) a playlist of them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub url: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration: Option<f64>,
    pub thumbnail: Option<String>,
    /// "youtube", "soundcloud", "bandcamp"…
    pub site: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    /// Set when a link was a playlist/album/set.
    pub playlist: Option<String>,
    pub items: Vec<Item>,
}

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(|x| x.as_str()).map(str::trim).filter(|x| !x.is_empty()).map(str::to_string)
}

fn site_of(url: &str, v: &Value) -> String {
    let host = url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default();
    if host.contains("youtube") || host.contains("youtu.be") {
        "youtube".into()
    } else if host.contains("soundcloud") {
        "soundcloud".into()
    } else if let Some(k) = s(v, "extractor_key").or_else(|| s(v, "ie_key")) {
        k.to_lowercase()
    } else {
        host.trim_start_matches("www.").split('.').next().unwrap_or("web").to_string()
    }
}

/// The best thumbnail URL: the largest square-ish one, else the last.
fn thumb(v: &Value) -> Option<String> {
    if let Some(t) = s(v, "thumbnail") {
        return Some(t);
    }
    let list = v.get("thumbnails")?.as_array()?;
    list.iter()
        .filter_map(|t| Some((s(t, "url")?, t.get("width").and_then(Value::as_u64).unwrap_or(0))))
        .max_by_key(|(_, w)| *w)
        .map(|(u, _)| u)
}

/// Turn one yt-dlp info dict (full or flat) into an [`Item`].
pub fn item(v: &Value) -> Option<Item> {
    let url = match s(v, "webpage_url").or_else(|| s(v, "url")).filter(|u| u.starts_with("http")) {
        Some(u) => u,
        // Flat YouTube entries sometimes give only an id.
        None => format!("https://music.youtube.com/watch?v={}", s(v, "id")?),
    };
    let artist = v
        .get("artists")
        .and_then(Value::as_array)
        .and_then(|a| {
            let names: Vec<&str> = a.iter().filter_map(Value::as_str).collect();
            (!names.is_empty()).then(|| names.join(", "))
        })
        .or_else(|| s(v, "artist"))
        .or_else(|| s(v, "creator"))
        .or_else(|| s(v, "uploader"))
        .or_else(|| s(v, "channel"))
        .unwrap_or_else(|| "Unknown artist".into());
    Some(Item {
        title: s(v, "track").or_else(|| s(v, "title")).unwrap_or_else(|| "Untitled".into()),
        artist: artist.trim_end_matches(" - Topic").to_string(),
        album: s(v, "album"),
        duration: v.get("duration").and_then(Value::as_f64).filter(|d| *d > 0.0),
        thumbnail: thumb(v),
        site: site_of(&url, v),
        url,
    })
}

/// Parse `yt-dlp -J` output for a search or link.
pub fn parse_found(v: &Value) -> Found {
    let is_list = v.get("_type").and_then(Value::as_str).is_some_and(|t| t == "playlist") || v.get("entries").is_some();
    if is_list {
        let items = v.get("entries").and_then(Value::as_array).map(|e| e.iter().filter_map(item).collect()).unwrap_or_default();
        Found { playlist: s(v, "title"), items }
    } else {
        Found { playlist: None, items: item(v).into_iter().collect() }
    }
}

fn base_args() -> Vec<String> {
    ["--encoding", "utf-8", "--no-warnings", "--ignore-config", "--no-cache-dir"].iter().map(|s| s.to_string()).collect()
}

async fn run_json(exe: &Path, args: &[String], target: &str, timeout: Duration) -> Result<Value> {
    let mut c = command(exe);
    c.args(base_args()).args(args).arg("--").arg(target).stdout(Stdio::piped()).stderr(Stdio::piped());
    let out = tokio::time::timeout(timeout, c.output()).await.map_err(|_| anyhow!("That took too long. Check your connection and try again."))??;
    if !out.status.success() {
        bail!("{}", friendly_error(&String::from_utf8_lossy(&out.stderr)));
    }
    serde_json::from_slice(&out.stdout).context("yt-dlp sent something unexpected")
}

/// yt-dlp's errors, said plainly.
pub fn friendly_error(stderr: &str) -> String {
    let line = stderr.lines().rev().find(|l| l.contains("ERROR")).or_else(|| stderr.lines().rev().find(|l| !l.trim().is_empty())).unwrap_or("").trim();
    let low = line.to_lowercase();
    if low.contains("sign in to confirm") || low.contains("not a bot") {
        return "YouTube wants to check you're not a bot. Wait a while, or update yt-dlp under Downloads → Setup.".into();
    }
    if low.contains("private video") || low.contains("video unavailable") || low.contains("404") {
        return "That isn't available (removed, private or region-locked).".into();
    }
    if low.contains("unsupported url") {
        return "That link isn't one yt-dlp knows how to read.".into();
    }
    if low.contains("ffmpeg") && (low.contains("not found") || low.contains("not installed")) {
        return "This needs ffmpeg. Get it under Downloads → Setup.".into();
    }
    if low.contains("drm") {
        return "That's protected with DRM, so Arcadia won't fetch it.".into();
    }
    let msg = line.trim_start_matches("ERROR:").trim();
    if msg.is_empty() {
        "yt-dlp couldn't do that. Try updating it under Downloads → Setup.".into()
    } else {
        msg.to_string()
    }
}

pub async fn search(exe: &Path, source: Source, query: &str) -> Result<Found> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Found { playlist: None, items: Vec::new() });
    }
    let target = match source {
        Source::YoutubeMusic => {
            let enc: String = url::form_urlencoded::byte_serialize(q.as_bytes()).collect();
            format!("https://music.youtube.com/search?q={enc}#songs")
        }
        Source::Soundcloud => format!("scsearch25:{q}"),
    };
    let args: Vec<String> = ["--flat-playlist", "-J", "--playlist-end", "30"].iter().map(|s| s.to_string()).collect();
    let mut f = parse_found(&run_json(exe, &args, &target, Duration::from_secs(45)).await?);
    f.playlist = None;
    Ok(f)
}

/// Read a pasted link: one song, or a playlist/album/set.
pub async fn resolve(exe: &Path, link: &str) -> Result<Found> {
    let u = url::Url::parse(link.trim()).map_err(|_| anyhow!("That doesn't look like a link"))?;
    if !matches!(u.scheme(), "http" | "https") {
        bail!("Only web links can be opened");
    }
    let args: Vec<String> = ["--flat-playlist", "-J", "--playlist-end", "500"].iter().map(|s| s.to_string()).collect();
    let v = run_json(exe, &args, u.as_str(), Duration::from_secs(90)).await?;
    Ok(parse_found(&v))
}

// ───────────────────────── fetching ─────────────────────────

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub percent: f64,
    pub speed: String,
    pub eta: String,
    /// "downloading", "converting", "tagging"
    pub stage: String,
}

const PROGRESS_TAG: &str = "[arcadia]";

/// One progress line from our `--progress-template`.
pub fn parse_progress(line: &str) -> Option<Progress> {
    let rest = line.trim().strip_prefix(PROGRESS_TAG)?.trim();
    let mut it = rest.split('|').map(str::trim);
    let pct = it.next()?.trim_end_matches('%').trim().parse::<f64>().ok()?;
    let speed = it.next().unwrap_or("").to_string();
    let eta = it.next().unwrap_or("").to_string();
    Some(Progress { percent: pct.clamp(0.0, 100.0), speed: if speed == "NA" { String::new() } else { speed }, eta: if eta == "NA" { String::new() } else { eta }, stage: "downloading".into() })
}

/// yt-dlp's format choice for a target, preferring what the engine plays
/// without any conversion.
pub fn format_args(fmt: Format, sc_original: bool, url: &str, have_ffmpeg: bool) -> Result<Vec<String>> {
    let sc = url.contains("soundcloud.com");
    let mut v: Vec<String> = Vec::new();
    let select = match (fmt, sc && sc_original) {
        // The artist's own upload first, when they allow downloads.
        (Format::M4a, true) => "download/bestaudio[ext=m4a]/bestaudio[acodec=mp3]/bestaudio",
        (Format::M4a, false) => "bestaudio[ext=m4a]/bestaudio[acodec^=mp4a]/bestaudio[acodec=mp3]/bestaudio",
        (_, true) => "download/bestaudio",
        (_, false) => "bestaudio",
    };
    v.extend(["-f".into(), select.into()]);
    match fmt {
        Format::M4a => {}
        Format::Mp3 | Format::Flac => {
            if !have_ffmpeg {
                bail!("{} needs ffmpeg. Get it under Downloads → Setup, or pick M4A.", if fmt == Format::Mp3 { "MP3" } else { "FLAC" });
            }
            v.extend(["-x".into(), "--audio-format".into(), if fmt == Format::Mp3 { "mp3".into() } else { "flac".into() }]);
            if fmt == Format::Mp3 {
                v.extend(["--audio-quality".into(), "320K".into()]);
            }
        }
    }
    if have_ffmpeg {
        v.extend(["--embed-metadata".into(), "--embed-thumbnail".into(), "--convert-thumbnails".into(), "jpg".into()]);
    }
    Ok(v)
}

/// What a finished fetch produced: the file, and yt-dlp's info about it.
#[derive(Debug, Clone)]
pub struct Fetched {
    pub path: PathBuf,
    pub item: Option<Item>,
}

/// Run yt-dlp for one URL into `out_template`, reporting progress, until
/// done or `cancel` fires.
#[allow(clippy::too_many_arguments)]
pub async fn fetch(
    ytdlp: &Path,
    ffmpeg: Option<&Path>,
    url: &str,
    fmt: Format,
    sc_original: bool,
    out_template: &str,
    mut progress: impl FnMut(Progress),
    mut cancel: tokio::sync::watch::Receiver<bool>,
) -> Result<Fetched> {
    let mut args = base_args();
    args.extend(format_args(fmt, sc_original, url, ffmpeg.is_some())?);
    if let Some(f) = ffmpeg {
        args.extend(["--ffmpeg-location".into(), f.display().to_string()]);
    }
    args.extend(
        [
            "--no-playlist",
            "--windows-filenames",
            "--no-overwrites",
            "--newline",
            "--progress",
            "--progress-template",
            "download:[arcadia] %(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s",
            "-O",
            "after_move:%()j",
            "-o",
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    args.push(out_template.to_string());
    let mut c = command(ytdlp);
    c.args(&args).arg("--").arg(url).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = c.spawn().context("Couldn't start yt-dlp")?;
    let stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let err_task = tokio::spawn(async move {
        let mut s = String::new();
        let _ = tokio::io::AsyncReadExt::read_to_string(&mut stderr, &mut s).await;
        s
    });
    let mut lines = BufReader::new(stdout).lines();
    let mut info: Option<Value> = None;
    loop {
        tokio::select! {
            l = lines.next_line() => match l? {
                Some(line) => {
                    if let Some(p) = parse_progress(&line) {
                        progress(p);
                    } else if line.trim_start().starts_with('{') {
                        info = serde_json::from_str(&line).ok();
                        progress(Progress { percent: 100.0, stage: "tagging".into(), ..Default::default() });
                    }
                }
                None => break,
            },
            _ = cancel.changed() => {
                if *cancel.borrow() {
                    let _ = child.kill().await;
                    bail!("Cancelled");
                }
            }
        }
    }
    let status = child.wait().await?;
    let stderr = err_task.await.unwrap_or_default();
    if !status.success() {
        bail!("{}", friendly_error(&stderr));
    }
    let info = info.ok_or_else(|| anyhow!("{}", friendly_error(&stderr)))?;
    let path = s(&info, "filepath").map(PathBuf::from).ok_or_else(|| anyhow!("yt-dlp didn't say where it saved the file"))?;
    let path = if is_playable(&path) {
        path
    } else {
        // e.g. an Opus-only source: make it something the engine plays.
        let ff = ffmpeg.ok_or_else(|| anyhow!("This came as {} which Arcadia can't play yet; get ffmpeg under Downloads → Setup to convert it.", path.extension().and_then(|e| e.to_str()).unwrap_or("an unknown format").to_uppercase()))?;
        progress(Progress { percent: 100.0, stage: "converting".into(), ..Default::default() });
        convert_playable(ff, &path).await?
    };
    Ok(Fetched { path, item: item(&info) })
}

/// Lossless sources become FLAC; everything else AAC in M4A.
async fn convert_playable(ffmpeg: &Path, src: &Path) -> Result<PathBuf> {
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let lossless = matches!(ext.as_str(), "aif" | "aiff" | "alac" | "wv" | "ape");
    let (out_ext, codec): (&str, &[&str]) = if lossless { ("flac", &["-c:a", "flac"]) } else { ("m4a", &["-c:a", "aac", "-b:a", "256k"]) };
    let out = src.with_extension(out_ext);
    let st = command(ffmpeg)
        .args(["-y", "-loglevel", "error", "-i"])
        .arg(src)
        .args(["-vn", "-map_metadata", "0"])
        .args(codec)
        .arg(&out)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await?;
    if !st.status.success() {
        bail!("ffmpeg couldn't convert it: {}", String::from_utf8_lossy(&st.stderr).lines().last().unwrap_or(""));
    }
    let _ = std::fs::remove_file(src);
    Ok(out)
}

/// Fetch a cover/thumbnail image (for songs without embedded art).
pub async fn fetch_image(url: &str) -> Result<Vec<u8>> {
    let u = url::Url::parse(url)?;
    if u.scheme() != "https" {
        bail!("Only https images");
    }
    let r = client()?.get(u).timeout(Duration::from_secs(20)).send().await?.error_for_status()?;
    if r.content_length().is_some_and(|l| l > 15 * 1024 * 1024) {
        bail!("Image too large");
    }
    Ok(r.bytes().await?.to_vec())
}

/// A short, stable file name for a link in the cache.
pub fn cache_key(url: &str) -> String {
    use sha1::{Digest as _, Sha1};
    hex::encode(Sha1::digest(url.as_bytes()))[..16].to_string()
}

/// Bytes used by the stream cache.
pub fn cache_size() -> u64 {
    std::fs::read_dir(cache_dir()).map(|rd| rd.flatten().filter_map(|e| e.metadata().ok()).filter(|m| m.is_file()).map(|m| m.len()).sum()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_search_and_playlist_output() {
        let v = json!({
            "_type": "playlist", "title": "Late night",
            "entries": [
                {"id": "abc", "url": "https://music.youtube.com/watch?v=abc", "title": "Lantern Lights", "channel": "Ivy Lantern - Topic", "duration": 231.0,
                 "thumbnails": [{"url": "https://i.ytimg.com/s.jpg", "width": 120}, {"url": "https://i.ytimg.com/l.jpg", "width": 544}]},
                {"id": "def", "url": "https://soundcloud.com/ivy/porch-static", "title": "Porch Static", "uploader": "Ivy", "duration": 182},
                {"id": "xyz", "title": "No url"}
            ]
        });
        let f = parse_found(&v);
        assert_eq!(f.playlist.as_deref(), Some("Late night"));
        assert_eq!(f.items.len(), 3);
        assert_eq!(f.items[0].artist, "Ivy Lantern");
        assert_eq!(f.items[0].thumbnail.as_deref(), Some("https://i.ytimg.com/l.jpg"));
        assert_eq!(f.items[0].site, "youtube");
        assert_eq!(f.items[1].site, "soundcloud");
        assert_eq!(f.items[2].url, "https://music.youtube.com/watch?v=xyz");

        let one = parse_found(&json!({"webpage_url": "https://x.bandcamp.com/track/a", "track": "A", "artists": ["P", "Q"], "album": "Al", "extractor_key": "Bandcamp"}));
        assert!(one.playlist.is_none());
        assert_eq!((one.items[0].title.as_str(), one.items[0].artist.as_str(), one.items[0].site.as_str()), ("A", "P, Q", "bandcamp"));
    }

    #[test]
    fn progress_lines() {
        let p = parse_progress("[arcadia]  42.5%|1.20MiB/s|00:03").unwrap();
        assert_eq!((p.percent, p.speed.as_str(), p.eta.as_str()), (42.5, "1.20MiB/s", "00:03"));
        assert!(parse_progress("[download] Destination: x").is_none());
        assert_eq!(parse_progress("[arcadia] 100.0%|NA|NA").unwrap().speed, "");
    }

    #[test]
    fn formats_need_ffmpeg_only_when_converting() {
        let a = format_args(Format::M4a, true, "https://soundcloud.com/a/b", false).unwrap();
        assert_eq!(a[1], "download/bestaudio[ext=m4a]/bestaudio[acodec=mp3]/bestaudio");
        assert!(!a.contains(&"--embed-metadata".to_string()));
        assert!(format_args(Format::Mp3, false, "https://music.youtube.com/watch?v=a", false).is_err());
        let m = format_args(Format::Mp3, false, "https://music.youtube.com/watch?v=a", true).unwrap();
        assert!(m.windows(2).any(|w| w == ["--audio-format", "mp3"]));
        assert!(m.contains(&"--embed-thumbnail".to_string()));
    }

    #[test]
    fn checksums_and_errors() {
        let list = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  yt-dlp.exe\nffff  short\n";
        assert!(checksum_for(list, "yt-dlp.exe").is_some());
        assert!(checksum_for(list, "short").is_none());
        assert!(checksum_for(list, "missing").is_none());
        assert!(friendly_error("ERROR: [youtube] abc: Sign in to confirm you're not a bot").contains("bot"));
        assert!(friendly_error("ERROR: Unsupported URL: https://x").contains("isn't one"));
        assert!(is_playable(Path::new("a.M4A")) && !is_playable(Path::new("a.opus")));
    }
}
