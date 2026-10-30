//! Discord Rich Presence over Discord's local IPC socket (a named pipe on
//! Windows). Only needs the app's public Application ID: no tokens, no bot.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::io::{Read, Write};

trait Pipe: Read + Write + Send {}
impl<T: Read + Write + Send> Pipe for T {}

pub struct Client {
    pipe: Box<dyn Pipe>,
    /// Who Discord is signed in as.
    pub user: String,
}

/// A Discord Application ID: 17–20 digits.
pub fn valid_id(id: &str) -> bool {
    let id = id.trim();
    (17..=20).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_digit())
}

#[cfg(windows)]
fn open() -> Result<Box<dyn Pipe>> {
    for i in 0..10 {
        if let Ok(f) = std::fs::OpenOptions::new().read(true).write(true).open(format!(r"\\.\pipe\discord-ipc-{i}")) {
            return Ok(Box::new(f));
        }
    }
    bail!("Discord isn't running on this PC")
}

#[cfg(unix)]
fn open() -> Result<Box<dyn Pipe>> {
    let mut bases: Vec<std::path::PathBuf> = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"].iter().filter_map(|v| std::env::var_os(v)).map(Into::into).collect();
    bases.push("/tmp".into());
    for b in bases {
        for sub in ["", "app/com.discordapp.Discord", "snap.discord", ".flatpak/dev.vencord.Vesktop/xdg-run"] {
            for i in 0..10 {
                if let Ok(s) = std::os::unix::net::UnixStream::connect(b.join(sub).join(format!("discord-ipc-{i}"))) {
                    return Ok(Box::new(s));
                }
            }
        }
    }
    bail!("Discord isn't running on this PC")
}

fn frame(op: u32, v: &Value) -> Vec<u8> {
    let body = v.to_string().into_bytes();
    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(&op.to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend(body);
    out
}

impl Client {
    fn send(&mut self, op: u32, v: &Value) -> Result<()> {
        self.pipe.write_all(&frame(op, v))?;
        self.pipe.flush()?;
        Ok(())
    }

    fn recv(&mut self) -> Result<(u32, Value)> {
        let mut head = [0u8; 8];
        self.pipe.read_exact(&mut head)?;
        let op = u32::from_le_bytes(head[..4].try_into()?);
        let len = u32::from_le_bytes(head[4..].try_into()?) as usize;
        if len > 1 << 20 {
            bail!("Discord sent something unexpected");
        }
        let mut body = vec![0; len];
        self.pipe.read_exact(&mut body)?;
        Ok((op, serde_json::from_slice(&body).context("Discord's reply")?))
    }

    /// Connect and say which app we are.
    pub fn connect(client_id: &str) -> Result<Client> {
        if !valid_id(client_id) {
            bail!("That doesn't look like an Application ID (it's a long number)");
        }
        Self::connect_with(open()?, client_id)
    }

    fn connect_with(pipe: Box<dyn Pipe>, client_id: &str) -> Result<Client> {
        let mut c = Client { pipe, user: String::new() };
        c.send(0, &json!({ "v": 1, "client_id": client_id.trim() }))?;
        let (op, v) = c.recv()?;
        if op == 2 || v["evt"] != "READY" {
            let msg = v["message"].as_str().unwrap_or("Discord refused the connection");
            bail!("{}", if msg.contains("client id") || v["code"] == 4000 { "Discord doesn't know that Application ID" } else { msg });
        }
        let u = &v["data"]["user"];
        c.user = u["global_name"].as_str().filter(|s| !s.is_empty()).or(u["username"].as_str()).unwrap_or("").to_string();
        Ok(c)
    }

    /// Show `activity`, or clear it with `None`.
    pub fn set(&mut self, activity: Option<&Activity>) -> Result<()> {
        let nonce = uuid::Uuid::new_v4().to_string();
        self.send(1, &json!({ "cmd": "SET_ACTIVITY", "args": { "pid": std::process::id(), "activity": activity.map(Activity::to_json) }, "nonce": nonce }))?;
        // Discord may send other events first; wait for our reply.
        for _ in 0..8 {
            let (op, v) = self.recv()?;
            if op == 2 {
                bail!("Discord closed the connection");
            }
            if v["nonce"] == nonce.as_str() {
                if v["evt"] == "ERROR" {
                    return Err(anyhow!("Discord: {}", v["data"]["message"].as_str().unwrap_or("didn't accept that")));
                }
                return Ok(());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    /// 0 playing, 2 listening.
    pub kind: u8,
    pub details: String,
    pub state: String,
    /// Unix milliseconds.
    pub start: Option<u64>,
    pub end: Option<u64>,
    /// An uploaded asset name or an https link.
    pub large_image: String,
    pub large_text: String,
    pub small_image: String,
    pub small_text: String,
    pub buttons: Vec<(String, String)>,
}

/// Discord wants 2–128 characters.
fn text(s: &str) -> Option<String> {
    let s: String = s.trim().chars().take(128).collect();
    match s.chars().count() {
        0 => None,
        1 => Some(format!("{s} ")),
        _ => Some(s),
    }
}

fn image(s: &str) -> Option<String> {
    let s = s.trim();
    let ok = if s.starts_with("https://") { s.len() <= 300 } else { !s.is_empty() && s.len() <= 32 };
    ok.then(|| s.to_string())
}

impl Activity {
    pub fn to_json(&self) -> Value {
        let mut a = serde_json::Map::new();
        a.insert("type".into(), self.kind.into());
        if let Some(d) = text(&self.details) {
            a.insert("details".into(), d.into());
        }
        if let Some(s) = text(&self.state) {
            a.insert("state".into(), s.into());
        }
        let mut ts = serde_json::Map::new();
        if let Some(s) = self.start {
            ts.insert("start".into(), s.into());
        }
        if let Some(e) = self.end {
            ts.insert("end".into(), e.into());
        }
        if !ts.is_empty() {
            a.insert("timestamps".into(), ts.into());
        }
        let mut assets = serde_json::Map::new();
        for (k, v) in [("large_image", &self.large_image), ("small_image", &self.small_image)] {
            if let Some(i) = image(v) {
                assets.insert(k.into(), i.into());
            }
        }
        for (k, v) in [("large_text", &self.large_text), ("small_text", &self.small_text)] {
            if let Some(t) = text(v) {
                assets.insert(k.into(), t.into());
            }
        }
        if !assets.is_empty() {
            a.insert("assets".into(), assets.into());
        }
        let buttons: Vec<Value> = self
            .buttons
            .iter()
            .filter(|(l, u)| !l.trim().is_empty() && u.starts_with("https://") && u.len() <= 512)
            .take(2)
            .map(|(l, u)| json!({ "label": l.trim().chars().take(32).collect::<String>(), "url": u }))
            .collect();
        if !buttons.is_empty() {
            a.insert("buttons".into(), buttons.into());
        }
        Value::Object(a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_activity_json_within_discords_limits() {
        let a = Activity {
            kind: 0,
            details: "Blox Fruits".into(),
            state: "x".into(),
            start: Some(1_700_000_000_000),
            large_image: "https://tr.rbxcdn.com/abc/150/150/Image/Png".into(),
            large_text: "Blox Fruits".into(),
            small_image: "arcadia".into(),
            buttons: vec![("Join my server".into(), "https://www.roblox.com/games/start?placeId=1".into()), ("Bad".into(), "http://insecure".into())],
            ..Default::default()
        };
        let v = a.to_json();
        assert_eq!(v["details"], "Blox Fruits");
        assert_eq!(v["state"], "x ");
        assert_eq!(v["timestamps"]["start"], 1_700_000_000_000u64);
        assert_eq!(v["assets"]["small_image"], "arcadia");
        assert_eq!(v["buttons"].as_array().unwrap().len(), 1);
        assert!(v.get("assets").unwrap().get("small_text").is_none());
        let long = Activity { details: "a".repeat(300), ..Default::default() };
        assert_eq!(long.to_json()["details"].as_str().unwrap().len(), 128);
        assert!(valid_id("1234567890123456789"));
        assert!(!valid_id("12345") && !valid_id("abc4567890123456789"));
    }

    #[cfg(unix)]
    #[test]
    fn talks_to_a_fake_discord() {
        use std::os::unix::net::UnixStream;
        let (ours, mut theirs) = UnixStream::pair().unwrap();
        let server = std::thread::spawn(move || {
            let mut read = |s: &mut UnixStream| -> (u32, Value) {
                let mut h = [0u8; 8];
                s.read_exact(&mut h).unwrap();
                let len = u32::from_le_bytes(h[4..].try_into().unwrap()) as usize;
                let mut b = vec![0; len];
                s.read_exact(&mut b).unwrap();
                (u32::from_le_bytes(h[..4].try_into().unwrap()), serde_json::from_slice(&b).unwrap())
            };
            let (op, hello) = read(&mut theirs);
            assert_eq!((op, hello["client_id"].as_str()), (0, Some("1234567890123456789")));
            theirs.write_all(&frame(1, &json!({"cmd":"DISPATCH","evt":"READY","data":{"user":{"username":"sky","global_name":"Skyline"}}}))).unwrap();
            let (_, set) = read(&mut theirs);
            assert_eq!(set["cmd"], "SET_ACTIVITY");
            assert_eq!(set["args"]["activity"]["details"], "Testing");
            theirs.write_all(&frame(1, &json!({"cmd":"SET_ACTIVITY","nonce":set["nonce"],"data":{}}))).unwrap();
            let (_, clear) = read(&mut theirs);
            assert!(clear["args"]["activity"].is_null());
            theirs.write_all(&frame(1, &json!({"cmd":"SET_ACTIVITY","evt":"ERROR","nonce":clear["nonce"],"data":{"message":"nope"}}))).unwrap();
        });
        let mut c = Client::connect_with(Box::new(ours), "1234567890123456789").unwrap();
        assert_eq!(c.user, "Skyline");
        c.set(Some(&Activity { details: "Testing".into(), ..Default::default() })).unwrap();
        assert!(c.set(None).unwrap_err().to_string().contains("nope"));
        server.join().unwrap();
    }
}
