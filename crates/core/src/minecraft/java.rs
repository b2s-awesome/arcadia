//! Java runtimes from Mojang's own runtime feed, so players never need to
//! install Java themselves. Each Minecraft version names the runtime it wants
//! (`javaVersion.component`, e.g. `java-runtime-delta`).

use super::download::{fetch_all, Job};
use super::{progress, OnProgress};
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Deserialize)]
struct Entry {
    manifest: Ref,
}
#[derive(Deserialize)]
struct Ref {
    url: String,
    #[serde(default)]
    sha1: Option<String>,
}
#[derive(Deserialize)]
struct RuntimeManifest {
    files: HashMap<String, RuntimeFile>,
}
#[derive(Deserialize)]
struct RuntimeFile {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    executable: bool,
    #[serde(default)]
    downloads: Option<HashMap<String, Raw>>,
}
#[derive(Deserialize)]
struct Raw {
    url: String,
    sha1: Option<String>,
    size: Option<u64>,
}

pub fn platform_key() -> &'static str {
    match (super::version::os_name(), super::version::arch_name()) {
        ("windows", "arm64") => "windows-arm64",
        ("windows", "x86") => "windows-x86",
        ("windows", _) => "windows-x64",
        ("osx", "arm64") => "mac-os-arm64",
        ("osx", _) => "mac-os",
        (_, "x86") => "linux-i386",
        _ => "linux",
    }
}

pub fn java_exe(component: &str) -> PathBuf {
    let base = super::runtimes_dir().join(component);
    if cfg!(windows) {
        base.join("bin").join("javaw.exe")
    } else if cfg!(target_os = "macos") {
        base.join("jre.bundle/Contents/Home/bin/java")
    } else {
        base.join("bin").join("java")
    }
}

/// Command-line Java (with a console) — used to run loader installers.
pub fn java_console_exe(component: &str) -> PathBuf {
    let p = java_exe(component);
    if cfg!(windows) {
        p.with_file_name("java.exe")
    } else {
        p
    }
}

/// Make sure `component` is installed; returns the java executable.
pub async fn ensure(component: &str, task: &str, on: &OnProgress) -> Result<PathBuf> {
    let dir = super::runtimes_dir().join(component);
    let marker = dir.join(".arcadia-complete");
    let exe = java_exe(component);
    if marker.exists() && exe.exists() {
        return Ok(exe);
    }
    progress(on, task, format!("Getting Java ({component})"), 0, 1);
    let client = super::http();
    let all: HashMap<String, HashMap<String, Vec<Entry>>> = client
        .get(super::urls::java_runtimes())
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .context("reading Mojang's Java runtime list")?;
    let entry = all
        .get(platform_key())
        .and_then(|m| m.get(component))
        .and_then(|v| v.first())
        .ok_or_else(|| anyhow!("Mojang has no {component} Java runtime for {}", platform_key()))?;
    let manifest: RuntimeManifest = client.get(&entry.manifest.url).send().await?.error_for_status()?.json().await?;

    let mut jobs = Vec::new();
    let mut executables = Vec::new();
    for (path, f) in &manifest.files {
        let Some(rel) = super::safe_rel_path(path) else { continue };
        let target = dir.join(rel);
        match f.kind.as_str() {
            "directory" => std::fs::create_dir_all(&target)?,
            "file" => {
                if let Some(raw) = f.downloads.as_ref().and_then(|d| d.get("raw")) {
                    jobs.push(Job { url: raw.url.clone(), dest: target.clone(), sha1: raw.sha1.clone(), sha512: None, size: raw.size });
                    if f.executable {
                        executables.push(target);
                    }
                }
            }
            _ => {} // symlinks only matter on macOS bundles; the java binary isn't one.
        }
    }
    let on2 = on.clone();
    let t = task.to_string();
    fetch_all(jobs, move |d, n| progress(&on2, &t, "Downloading Java", d, n)).await?;
    #[cfg(unix)]
    for p in executables {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755));
    }
    #[cfg(not(unix))]
    drop(executables);
    std::fs::write(&marker, entry.manifest.sha1.clone().unwrap_or_default())?;
    Ok(exe)
}

/// Which runtime a version needs. Old versions without `javaVersion` use Java 8.
pub fn component_for(v: &super::version::VersionJson) -> String {
    v.java_version
        .as_ref()
        .and_then(|j| j.component.clone())
        .unwrap_or_else(|| "jre-legacy".into())
}
