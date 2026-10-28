//! Read a mod jar's own metadata (Fabric, Quilt, NeoForge, Forge, legacy Forge).

use serde::Serialize;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct ModMeta {
    pub id: String,
    pub name: String,
    pub version: String,
    /// "fabric", "quilt", "neoforge", "forge" or "legacy-forge".
    pub loader: String,
    pub description: String,
}

fn read_entry(z: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<String> {
    let mut e = z.by_name(name).ok()?;
    if e.size() > 2 * 1024 * 1024 {
        return None;
    }
    let mut s = String::new();
    e.read_to_string(&mut s).ok()?;
    Some(s)
}

fn manifest_version(z: &mut zip::ZipArchive<std::fs::File>) -> Option<String> {
    let m = read_entry(z, "META-INF/MANIFEST.MF")?;
    m.lines()
        .find_map(|l| l.strip_prefix("Implementation-Version:"))
        .map(|v| v.trim().to_string())
}

pub fn read(path: &Path) -> Option<ModMeta> {
    let f = std::fs::File::open(path).ok()?;
    let mut z = zip::ZipArchive::new(f).ok()?;

    if let Some(s) = read_entry(&mut z, "fabric.mod.json") {
        // Some mods ship JSON with raw newlines in strings; be lenient.
        let v: serde_json::Value = serde_json::from_str(&s).or_else(|_| serde_json::from_str(&s.replace(['\n', '\r'], " "))).ok()?;
        let g = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string();
        return Some(ModMeta { id: g("id"), name: if g("name").is_empty() { g("id") } else { g("name") }, version: g("version"), loader: "fabric".into(), description: g("description") });
    }
    if let Some(s) = read_entry(&mut z, "quilt.mod.json") {
        let v: serde_json::Value = serde_json::from_str(&s).ok()?;
        let l = &v["quilt_loader"];
        let id = l["id"].as_str().unwrap_or_default().to_string();
        return Some(ModMeta {
            name: l["metadata"]["name"].as_str().map(String::from).unwrap_or_else(|| id.clone()),
            version: l["version"].as_str().unwrap_or_default().into(),
            description: l["metadata"]["description"].as_str().unwrap_or_default().into(),
            id,
            loader: "quilt".into(),
        });
    }
    for (file, loader) in [("META-INF/neoforge.mods.toml", "neoforge"), ("META-INF/mods.toml", "forge")] {
        if let Some(s) = read_entry(&mut z, file) {
            let v: toml::Value = toml::from_str(&s).ok()?;
            let m = v.get("mods")?.as_array()?.first()?;
            let g = |k: &str| m.get(k).and_then(|x| x.as_str()).unwrap_or_default().trim().to_string();
            let mut version = g("version");
            if version.contains("${") {
                version = manifest_version(&mut z).unwrap_or_default();
            }
            let id = g("modId");
            return Some(ModMeta { name: if g("displayName").is_empty() { id.clone() } else { g("displayName") }, id, version, loader: loader.into(), description: g("description") });
        }
    }
    if let Some(s) = read_entry(&mut z, "mcmod.info") {
        let v: serde_json::Value = serde_json::from_str(&s).ok()?;
        let first = match &v {
            serde_json::Value::Array(a) => a.first()?.clone(),
            o => o.get("modList").and_then(|l| l.as_array()).and_then(|a| a.first()).cloned()?,
        };
        let g = |k: &str| first.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string();
        return Some(ModMeta { id: g("modid"), name: g("name"), version: g("version"), loader: "legacy-forge".into(), description: g("description") });
    }
    None
}

/// Lowercase alphanumerics only, for matching names across sources.
pub fn normalize(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;

    pub fn make_jar(path: &Path, files: &[(&str, &str)]) {
        let f = std::fs::File::create(path).unwrap();
        let mut z = zip::ZipWriter::new(f);
        for (n, c) in files {
            z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(c.as_bytes()).unwrap();
        }
        z.finish().unwrap();
    }

    #[test]
    fn reads_each_metadata_format() {
        let t = tempfile::tempdir().unwrap();
        let a = t.path().join("a.jar");
        make_jar(&a, &[("fabric.mod.json", r#"{"id":"sodium","name":"Sodium","version":"0.6.0"}"#)]);
        assert_eq!(read(&a).unwrap().name, "Sodium");

        let b = t.path().join("b.jar");
        make_jar(&b, &[
            ("META-INF/neoforge.mods.toml", "modLoader=\"javafml\"\n[[mods]]\nmodId=\"jei\"\ndisplayName=\"Just Enough Items\"\nversion=\"${file.jarVersion}\"\n"),
            ("META-INF/MANIFEST.MF", "Manifest-Version: 1.0\nImplementation-Version: 19.21.0.247\n"),
        ]);
        let m = read(&b).unwrap();
        assert_eq!((m.id.as_str(), m.version.as_str(), m.loader.as_str()), ("jei", "19.21.0.247", "neoforge"));

        let c = t.path().join("c.jar");
        make_jar(&c, &[("mcmod.info", r#"[{"modid":"logisticspipes","name":"Logistics Pipes","version":"0.9.3.132"}]"#)]);
        assert_eq!(read(&c).unwrap().loader, "legacy-forge");

        let d = t.path().join("d.jar");
        make_jar(&d, &[("readme.txt", "hi")]);
        assert!(read(&d).is_none());
    }
}
