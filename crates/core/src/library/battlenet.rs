use super::{Game, LaunchTarget, Source};
use crate::registry::uninstall_entries;
use std::path::PathBuf;

/// Battle.net uninstall strings contain `--uid=<product uid>`; the launcher's
/// `--exec="launch <code>"` switch wants the product code instead.
const UID_TO_CODE: &[(&str, &str)] = &[
    ("prometheus", "Pro"), // Overwatch 2
    ("fenris", "Fen"),     // Diablo IV
    ("wow", "WoW"),
    ("wow_classic", "WoWC"),
    ("wow_classic_era", "WoWC"),
    ("hs_beta", "WTCG"), // Hearthstone
    ("heroes", "Hero"),
    ("s2", "S2"),
    ("s1", "S1"),
    ("diablo3", "D3"),
    ("osi", "OSI"), // Diablo II: Resurrected
    ("anbs", "ANBS"), // Diablo Immortal
    ("w3", "W3"),
    ("auks", "AUKS"), // Call of Duty HQ
    ("zeus", "ZEUS"),
    ("viper", "VIPR"),
    ("odin", "ODIN"),
    ("lazarus", "LAZR"),
    ("fore", "FORE"),
    ("wlby", "WLBY"),
    ("rtro", "RTRO"),
    ("gryphon", "GRY"), // Warcraft Rumble
];

fn battlenet_exe() -> Option<PathBuf> {
    let pf86 = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from)?;
    let p = pf86.join(r"Battle.net\Battle.net.exe");
    p.exists().then_some(p)
}

fn uid_from_uninstall(s: &str) -> Option<String> {
    let i = s.find("--uid=")? + 6;
    let rest = &s[i..];
    let end = rest.find(|c: char| c == ' ' || c == '"').unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

pub fn scan() -> anyhow::Result<Option<Vec<Game>>> {
    let entries: Vec<_> = uninstall_entries()
        .into_iter()
        .filter(|e| e.publisher.contains("Blizzard Entertainment") && e.display_name != "Battle.net")
        .collect();
    let exe = battlenet_exe();
    if entries.is_empty() && exe.is_none() {
        return Ok(None);
    }
    let mut out = Vec::new();
    for e in entries {
        let uid = uid_from_uninstall(&e.uninstall_string);
        let code = uid
            .as_deref()
            .and_then(|u| UID_TO_CODE.iter().find(|(k, _)| *k == u).map(|(_, c)| *c));
        let launch = match (code, &exe) {
            (Some(code), Some(exe)) => LaunchTarget::Exe {
                path: exe.to_string_lossy().into(),
                args: vec![format!("--exec=launch {code}")],
                cwd: None,
            },
            (Some(code), None) => LaunchTarget::Uri { uri: format!("battlenet://{code}") },
            _ => LaunchTarget::Uri { uri: "battlenet://".into() },
        };
        out.push(Game {
            id: format!("battlenet:{}", uid.unwrap_or(e.key)),
            name: e.display_name,
            source: Source::Battlenet,
            install_dir: (!e.install_location.is_empty()).then_some(e.install_location),
            launch,
            cover: None,
            hero: None,
        });
    }
    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    #[test]
    fn extracts_uid() {
        let s = r#""C:\ProgramData\Battle.net\Agent\Blizzard Uninstaller.exe" --lang=enUS --uid=prometheus --displayname="Overwatch""#;
        assert_eq!(super::uid_from_uninstall(s).as_deref(), Some("prometheus"));
    }
}
