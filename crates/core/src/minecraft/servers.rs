//! Multiplayer servers: the instance's saved list (`servers.dat`), a status
//! ping (players online, message of the day, latency) and the arguments that
//! join one straight from the launcher.

use super::instance::game_dir;
use super::nbt::{self, Tag};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SavedServer {
    pub name: String,
    pub address: String,
    /// The server's picture, as a data URL.
    #[serde(default)]
    pub icon: Option<String>,
}

fn file(id: &str) -> std::path::PathBuf {
    game_dir(id).join("servers.dat")
}

fn root(id: &str) -> Tag {
    std::fs::read(file(id)).ok().and_then(|b| nbt::read(&b).ok()).unwrap_or(Tag::Compound(vec![]))
}

pub fn list(id: &str) -> Vec<SavedServer> {
    root(id)
        .get("servers")
        .map(|l| {
            l.list()
                .iter()
                .filter_map(|s| {
                    Some(SavedServer {
                        name: s.get("name").and_then(Tag::str).unwrap_or("Minecraft Server").to_string(),
                        address: s.get("ip")?.str()?.to_string(),
                        icon: s.get("icon").and_then(Tag::str).map(|b| format!("data:image/png;base64,{b}")),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Add a server to the instance's list (Minecraft shows it under Multiplayer).
pub fn add(id: &str, name: &str, address: &str) -> Result<()> {
    let address = address.trim();
    if address.is_empty() || address.contains(char::is_whitespace) {
        bail!("That isn't a server address");
    }
    let mut r = root(id);
    let entry = Tag::Compound(vec![("name".into(), Tag::String(name.trim().into())), ("ip".into(), Tag::String(address.into())), ("acceptTextures".into(), Tag::Byte(0))]);
    let Tag::Compound(fields) = &mut r else { bail!("servers.dat is damaged") };
    match fields.iter_mut().find(|(k, _)| k == "servers") {
        Some((_, Tag::List(_, v))) => {
            if v.iter().any(|s| s.get("ip").and_then(Tag::str) == Some(address)) {
                return Ok(());
            }
            v.push(entry);
        }
        _ => fields.push(("servers".into(), Tag::List(10, vec![entry]))),
    }
    write(id, &r)
}

pub fn remove(id: &str, address: &str) -> Result<()> {
    let mut r = root(id);
    if let Tag::Compound(fields) = &mut r {
        if let Some((_, Tag::List(_, v))) = fields.iter_mut().find(|(k, _)| k == "servers") {
            v.retain(|s| s.get("ip").and_then(Tag::str) != Some(address));
        }
    }
    write(id, &r)
}

fn write(id: &str, r: &Tag) -> Result<()> {
    let p = file(id);
    std::fs::create_dir_all(p.parent().unwrap())?;
    crate::paths::write_atomic(&p, &nbt::write(r))?;
    Ok(())
}

/// Host and port from "play.example.net", "example.net:25566" or "[::1]:25565".
pub fn split_address(a: &str) -> (String, u16) {
    let a = a.trim();
    if let Some(rest) = a.strip_prefix('[') {
        if let Some((h, p)) = rest.split_once(']') {
            return (h.to_string(), p.trim_start_matches(':').parse().unwrap_or(25565));
        }
    }
    match a.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') => (h.to_string(), p.parse().unwrap_or(25565)),
        _ => (a.to_string(), 25565),
    }
}

/// Launch arguments that join `address` as soon as the game starts.
/// 1.20 and later use Quick Play; older versions use --server/--port.
pub fn join_args(mc_version: &str, address: &str) -> Vec<String> {
    let quick_play = super::loaders::cmp_versions(mc_version, "1.20") != std::cmp::Ordering::Less;
    if quick_play {
        vec!["--quickPlayMultiplayer".into(), address.trim().to_string()]
    } else {
        let (h, p) = split_address(address);
        vec!["--server".into(), h, "--port".into(), p.to_string()]
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub online: u32,
    pub max: u32,
    pub version: String,
    pub motd: String,
    pub icon: Option<String>,
    pub ping_ms: u32,
}

fn varint(mut v: i32, out: &mut Vec<u8>) {
    loop {
        let mut b = (v & 0x7f) as u8;
        v = ((v as u32) >> 7) as i32;
        if v != 0 {
            b |= 0x80;
        }
        out.push(b);
        if v == 0 {
            break;
        }
    }
}

async fn read_varint<R: AsyncReadExt + Unpin>(r: &mut R) -> Result<i32> {
    let mut v = 0i32;
    for i in 0..5 {
        let b = r.read_u8().await?;
        v |= ((b & 0x7f) as i32) << (7 * i);
        if b & 0x80 == 0 {
            return Ok(v);
        }
    }
    bail!("bad varint")
}

fn packet(id: i32, body: &[u8]) -> Vec<u8> {
    let mut inner = Vec::new();
    varint(id, &mut inner);
    inner.extend_from_slice(body);
    let mut out = Vec::new();
    varint(inner.len() as i32, &mut out);
    out.extend(inner);
    out
}

/// The text of a status `description`, which may be a string or chat JSON.
pub fn motd_text(v: &serde_json::Value) -> String {
    fn walk(v: &serde_json::Value, out: &mut String) {
        match v {
            serde_json::Value::String(s) => out.push_str(s),
            serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            serde_json::Value::Object(o) => {
                if let Some(t) = o.get("text") {
                    walk(t, out);
                }
                if let Some(e) = o.get("extra") {
                    walk(e, out);
                }
            }
            _ => {}
        }
    }
    let mut s = String::new();
    walk(v, &mut s);
    super::content::strip_formatting(&s)
}

/// Ask a server for its status (the Multiplayer screen's ping).
pub async fn ping(address: &str) -> Result<Status> {
    let (host, port) = split_address(address);
    let fut = async {
        let started = Instant::now();
        let mut s = tokio::net::TcpStream::connect((host.as_str(), port)).await.context("couldn't reach the server")?;
        let connect_ms = started.elapsed().as_millis() as u32;
        let mut hs = Vec::new();
        varint(-1, &mut hs);
        varint(host.len() as i32, &mut hs);
        hs.extend_from_slice(host.as_bytes());
        hs.extend_from_slice(&port.to_be_bytes());
        varint(1, &mut hs);
        s.write_all(&packet(0, &hs)).await?;
        s.write_all(&packet(0, &[])).await?;
        let len = read_varint(&mut s).await?;
        if !(1..=1 << 21).contains(&len) {
            bail!("unexpected reply");
        }
        let id = read_varint(&mut s).await?;
        if id != 0 {
            bail!("unexpected reply");
        }
        let n = read_varint(&mut s).await?;
        if !(0..=1 << 21).contains(&n) {
            bail!("unexpected reply");
        }
        let mut buf = vec![0; n as usize];
        s.read_exact(&mut buf).await?;
        let v: serde_json::Value = serde_json::from_slice(&buf).context("server status")?;
        // Latency: a ping packet round trip, falling back to connect time.
        let t = Instant::now();
        let ping_ms = match s.write_all(&packet(1, &1i64.to_be_bytes())).await {
            Ok(()) => match tokio::time::timeout(Duration::from_secs(3), async {
                let _ = read_varint(&mut s).await?;
                let _ = read_varint(&mut s).await?;
                Ok::<_, anyhow::Error>(s.read_i64().await?)
            })
            .await
            {
                Ok(Ok(_)) => t.elapsed().as_millis() as u32,
                _ => connect_ms,
            },
            Err(_) => connect_ms,
        };
        Ok(Status {
            online: v["players"]["online"].as_u64().unwrap_or(0) as u32,
            max: v["players"]["max"].as_u64().unwrap_or(0) as u32,
            version: super::content::strip_formatting(v["version"]["name"].as_str().unwrap_or("")),
            motd: motd_text(&v["description"]),
            icon: v["favicon"].as_str().filter(|f| f.starts_with("data:image/png;base64,") && f.len() < 200_000).map(str::to_string),
            ping_ms,
        })
    };
    tokio::time::timeout(Duration::from_secs(8), fut).await.map_err(|_| anyhow::anyhow!("the server didn't answer"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_and_join_args() {
        assert_eq!(split_address("mc.hypixel.net"), ("mc.hypixel.net".into(), 25565));
        assert_eq!(split_address("1.2.3.4:25570"), ("1.2.3.4".into(), 25570));
        assert_eq!(split_address("[::1]:5"), ("::1".into(), 5));
        assert_eq!(join_args("1.21.4", "a.b:1"), ["--quickPlayMultiplayer", "a.b:1"]);
        assert_eq!(join_args("1.8.9", "a.b:1"), ["--server", "a.b", "--port", "1"]);
        let mut v = Vec::new();
        varint(-1, &mut v);
        assert_eq!(v, [0xff, 0xff, 0xff, 0xff, 0x0f]);
        let d: serde_json::Value = serde_json::from_str(r#"{"text":"","extra":[{"text":"§6Hello "},"world"]}"#).unwrap();
        assert_eq!(motd_text(&d), "Hello world");
    }

    #[tokio::test]
    async fn pings_a_fake_server() {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut s, _) = l.accept().await.unwrap();
            // Handshake and status request.
            for _ in 0..2 {
                let n = read_varint(&mut s).await.unwrap();
                let mut b = vec![0; n as usize];
                s.read_exact(&mut b).await.unwrap();
            }
            let json = br#"{"version":{"name":"Paper 1.21.4"},"players":{"online":7,"max":50},"description":{"text":"Hi"}}"#;
            let mut body = Vec::new();
            varint(json.len() as i32, &mut body);
            body.extend_from_slice(json);
            s.write_all(&packet(0, &body)).await.unwrap();
            let n = read_varint(&mut s).await.unwrap();
            let mut b = vec![0; n as usize];
            s.read_exact(&mut b).await.unwrap();
            s.write_all(&packet(1, &b[1..])).await.unwrap();
        });
        let st = ping(&format!("127.0.0.1:{port}")).await.unwrap();
        assert_eq!((st.online, st.max, st.motd.as_str(), st.version.as_str()), (7, 50, "Hi", "Paper 1.21.4"));
    }

    #[test]
    fn edits_servers_dat() {
        let t = tempfile::tempdir().unwrap();
        let r = Tag::Compound(vec![]);
        let p = t.path().join("servers.dat");
        std::fs::write(&p, nbt::write(&r)).unwrap();
        let mut root = nbt::read(&std::fs::read(&p).unwrap()).unwrap();
        if let Tag::Compound(f) = &mut root {
            f.push(("servers".into(), Tag::List(10, vec![Tag::Compound(vec![("name".into(), Tag::String("A".into())), ("ip".into(), Tag::String("a.net".into()))])])));
        }
        std::fs::write(&p, nbt::write(&root)).unwrap();
        let back = nbt::read(&std::fs::read(&p).unwrap()).unwrap();
        assert_eq!(back.get("servers").unwrap().list().len(), 1);
    }
}
