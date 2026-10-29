//! Roblox accounts. The list (names, pictures, notes) is a plain file; each
//! account's login cookie lives only in Windows Credential Manager, split
//! across a few entries because one entry holds at most 1280 characters.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Credential Manager entries are filed under "Arcadia addon roblox-accounts".
const VAULT: &str = "roblox-accounts";
const CHUNK: usize = 1000;
const MAX_CHUNKS: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: u64,
    pub name: String,
    pub display_name: String,
    #[serde(default)]
    pub avatar: Option<String>,
    /// Your own label ("main", "alt for trading").
    #[serde(default)]
    pub note: String,
    pub added: u64,
    #[serde(default)]
    pub last_used: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Accounts {
    pub list: Vec<Account>,
    /// Used by Play when no account is picked. None = whoever the Roblox app
    /// is signed in as.
    #[serde(default)]
    pub default: Option<u64>,
}

fn file() -> PathBuf {
    crate::paths::roblox_dir().join("accounts.json")
}

pub fn load() -> Accounts {
    std::fs::read(file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn save(a: &Accounts) -> Result<()> {
    let p = file();
    std::fs::create_dir_all(p.parent().unwrap())?;
    crate::paths::write_atomic(&p, &serde_json::to_vec_pretty(a)?)?;
    Ok(())
}

fn key(id: u64, i: usize) -> String {
    format!("u{id}-{i}")
}

/// Split a cookie into Credential-Manager-sized pieces.
pub fn chunks(cookie: &str) -> Vec<String> {
    let chars: Vec<char> = cookie.chars().collect();
    chars.chunks(CHUNK).map(|c| c.iter().collect()).collect()
}

fn store_cookie(id: u64, cookie: &str) -> Result<()> {
    let parts = chunks(cookie.trim());
    if parts.is_empty() || parts.len() > MAX_CHUNKS {
        bail!("That login cookie doesn't look right");
    }
    for (i, p) in parts.iter().enumerate() {
        crate::addons::secrets::set(VAULT, &key(id, i), Some(p))?;
    }
    for i in parts.len()..MAX_CHUNKS {
        if crate::addons::secrets::get(VAULT, &key(id, i)).is_none() {
            break;
        }
        crate::addons::secrets::set(VAULT, &key(id, i), None)?;
    }
    Ok(())
}

/// The saved login cookie for an account.
pub fn cookie(id: u64) -> Option<String> {
    let mut out = String::new();
    for i in 0..MAX_CHUNKS {
        match crate::addons::secrets::get(VAULT, &key(id, i)) {
            Some(p) => out.push_str(&p),
            None => break,
        }
    }
    (!out.is_empty()).then_some(out)
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Add (or refresh) the account a login cookie belongs to.
pub async fn add(cookie: &str) -> Result<Account> {
    let cookie = cookie.trim().trim_start_matches(".ROBLOSECURITY=");
    let user = super::web::whoami(cookie).await?;
    let avatar = super::web::avatars(&[user.id]).await.ok().and_then(|v| v.into_iter().next()).map(|(_, u)| u);
    store_cookie(user.id, cookie)?;
    let mut all = load();
    let acc = match all.list.iter_mut().find(|a| a.id == user.id) {
        Some(a) => {
            a.name = user.name;
            a.display_name = user.display_name;
            a.avatar = avatar.or(a.avatar.take());
            a.clone()
        }
        None => {
            let a = Account { id: user.id, name: user.name, display_name: user.display_name, avatar, note: String::new(), added: now(), last_used: None };
            all.list.push(a.clone());
            a
        }
    };
    if all.default.is_none() && all.list.len() == 1 {
        all.default = Some(acc.id);
    }
    save(&all)?;
    Ok(acc)
}

/// Forget an account and delete its saved login.
pub fn remove(id: u64) -> Result<()> {
    for i in 0..MAX_CHUNKS {
        let _ = crate::addons::secrets::set(VAULT, &key(id, i), None);
    }
    let mut all = load();
    all.list.retain(|a| a.id != id);
    if all.default == Some(id) {
        all.default = None;
    }
    save(&all)
}

pub fn set_default(id: Option<u64>) -> Result<()> {
    let mut all = load();
    all.default = id.filter(|i| all.list.iter().any(|a| a.id == *i));
    save(&all)
}

pub fn set_note(id: u64, note: &str) -> Result<()> {
    let mut all = load();
    if let Some(a) = all.list.iter_mut().find(|a| a.id == id) {
        a.note = note.chars().take(60).collect();
    }
    save(&all)
}

/// Put accounts in this order (ids not listed keep their place at the end).
pub fn reorder(ids: &[u64]) -> Result<()> {
    let mut all = load();
    all.list.sort_by_key(|a| ids.iter().position(|i| *i == a.id).unwrap_or(usize::MAX));
    save(&all)
}

pub fn touch(id: u64) {
    let mut all = load();
    if let Some(a) = all.list.iter_mut().find(|a| a.id == id) {
        a.last_used = Some(now());
        let _ = save(&all);
    }
}

/// A ticket that starts the client signed in as `id`.
pub async fn ticket(id: u64) -> Result<String> {
    let Some(c) = cookie(id) else { bail!("This account's login is missing. Sign in to it again.") };
    super::web::auth_ticket(&c).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cookies_split_into_small_pieces() {
        let c = "x".repeat(2350);
        let p = chunks(&c);
        assert_eq!(p.len(), 3);
        assert!(p.iter().all(|x| x.chars().count() <= CHUNK));
        assert_eq!(p.concat(), c);
        assert!(chunks("").is_empty());
    }
}
