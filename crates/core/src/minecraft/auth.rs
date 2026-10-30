//! Microsoft account sign-in for Minecraft: Java Edition.
//!
//! Uses the OAuth device-code flow (the player signs in on microsoft.com in
//! their browser; Arcadia never sees their password), then Xbox Live → XSTS →
//! Minecraft services. Requires an Azure app (client) ID that Microsoft has
//! approved for Minecraft; the player's own refresh token is stored in the
//! OS keychain on Windows.

use super::launch::Session;
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const MS_BASE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
const SCOPE: &str = "XboxLive.signin offline_access";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCode {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub uuid: String,
    pub name: String,
    pub xuid: String,
    #[serde(default)]
    pub active: bool,
}

#[derive(Default, Serialize, Deserialize)]
struct Store {
    accounts: Vec<Account>,
    /// Refresh tokens, only on platforms without a keychain backend.
    #[serde(default)]
    tokens: std::collections::HashMap<String, String>,
}

fn store_path() -> std::path::PathBuf {
    super::dir().join("accounts.json")
}

fn load_store() -> Store {
    std::fs::read_to_string(store_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save_store(s: &Store) -> Result<()> {
    std::fs::create_dir_all(super::dir())?;
    let p = store_path();
    crate::paths::write_atomic(&p, &serde_json::to_vec_pretty(s)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

#[cfg(windows)]
fn put_token(uuid: &str, token: &str) -> Result<()> {
    keyring::Entry::new("Arcadia Minecraft", uuid)?.set_password(token)?;
    Ok(())
}
#[cfg(windows)]
fn get_token(uuid: &str) -> Result<String> {
    Ok(keyring::Entry::new("Arcadia Minecraft", uuid)?.get_password()?)
}
#[cfg(windows)]
fn del_token(uuid: &str) {
    if let Ok(e) = keyring::Entry::new("Arcadia Minecraft", uuid) {
        let _ = e.delete_credential();
    }
}
#[cfg(not(windows))]
fn put_token(uuid: &str, token: &str) -> Result<()> {
    let mut s = load_store();
    s.tokens.insert(uuid.into(), token.into());
    save_store(&s)
}
#[cfg(not(windows))]
fn get_token(uuid: &str) -> Result<String> {
    load_store().tokens.get(uuid).cloned().ok_or_else(|| anyhow!("signed-out"))
}
#[cfg(not(windows))]
fn del_token(uuid: &str) {
    let mut s = load_store();
    s.tokens.remove(uuid);
    let _ = save_store(&s);
}

pub fn accounts() -> Vec<Account> {
    load_store().accounts
}

pub fn set_active(uuid: &str) -> Result<()> {
    let mut s = load_store();
    if !s.accounts.iter().any(|a| a.uuid == uuid) {
        bail!("no such account");
    }
    for a in &mut s.accounts {
        a.active = a.uuid == uuid;
    }
    save_store(&s)
}

pub fn remove(uuid: &str) -> Result<()> {
    let mut s = load_store();
    s.accounts.retain(|a| a.uuid != uuid);
    if !s.accounts.iter().any(|a| a.active) {
        if let Some(a) = s.accounts.first_mut() {
            a.active = true;
        }
    }
    save_store(&s)?;
    del_token(uuid);
    Ok(())
}

fn need_client_id(client_id: &str) -> Result<&str> {
    let c = client_id.trim();
    if c.is_empty() {
        bail!("Add your Azure app (client) ID in Minecraft settings before signing in. Microsoft has to approve it for Minecraft first.");
    }
    Ok(c)
}

pub async fn start(client_id: &str) -> Result<DeviceCode> {
    let cid = need_client_id(client_id)?;
    let r = super::http().post(format!("{MS_BASE}/devicecode")).form(&[("client_id", cid), ("scope", SCOPE)]).send().await?;
    if !r.status().is_success() {
        let body: Value = r.json().await.unwrap_or_default();
        bail!("Microsoft refused to start sign-in: {}", body["error_description"].as_str().unwrap_or("check the client ID"));
    }
    Ok(serde_json::from_value(r.json::<Value>().await?)?)
}

#[derive(Deserialize)]
struct MsTokens {
    access_token: String,
    refresh_token: String,
}

/// Wait for the player to finish signing in, then link the account.
pub async fn finish(client_id: &str, code: &DeviceCode) -> Result<Account> {
    let cid = need_client_id(client_id)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(code.expires_in.max(60));
    let mut interval = code.interval.max(2);
    let tokens: MsTokens = loop {
        tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        if std::time::Instant::now() > deadline {
            bail!("The sign-in code expired. Start again.");
        }
        let r = super::http()
            .post(format!("{MS_BASE}/token"))
            .form(&[("grant_type", "urn:ietf:params:oauth:grant-type:device_code"), ("client_id", cid), ("device_code", &code.device_code)])
            .send()
            .await?;
        let body: Value = r.json().await?;
        match body["error"].as_str() {
            None => break serde_json::from_value(body)?,
            Some("authorization_pending") => continue,
            Some("slow_down") => interval += 5,
            Some("authorization_declined") => bail!("Sign-in was cancelled."),
            Some("expired_token") => bail!("The sign-in code expired. Start again."),
            Some(e) => bail!("Microsoft sign-in failed: {}", body["error_description"].as_str().unwrap_or(e)),
        }
    };
    let (session, account) = minecraft_login(cid, &tokens.access_token).await?;
    put_token(&account.uuid, &tokens.refresh_token)?;
    let mut s = load_store();
    s.accounts.retain(|a| a.uuid != account.uuid);
    for a in &mut s.accounts {
        a.active = false;
    }
    let mut acc = account.clone();
    acc.active = true;
    s.accounts.push(acc.clone());
    save_store(&s)?;
    drop(session);
    Ok(acc)
}

fn xerr_message(code: u64) -> &'static str {
    match code {
        2148916233 => "This Microsoft account has no Xbox profile yet. Sign in once at xbox.com to create one.",
        2148916235 => "Xbox Live isn't available in this account's country.",
        2148916236 | 2148916237 => "This account needs adult verification on xbox.com (South Korea).",
        2148916238 => "This is a child account. An adult needs to add it to a Microsoft family first.",
        _ => "Xbox Live refused the sign-in.",
    }
}

async fn minecraft_login(client_id: &str, ms_access: &str) -> Result<(Session, Account)> {
    let c = super::http();
    // Xbox Live
    let xbl: Value = c
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&json!({
            "Properties": {"AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={ms_access}")},
            "RelyingParty": "http://auth.xboxlive.com", "TokenType": "JWT"
        }))
        .send()
        .await?
        .error_for_status()
        .context("Xbox Live sign-in failed")?
        .json()
        .await?;
    let xbl_token = xbl["Token"].as_str().ok_or_else(|| anyhow!("Xbox Live returned no token"))?;

    // XSTS
    let r = c
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&json!({
            "Properties": {"SandboxId": "RETAIL", "UserTokens": [xbl_token]},
            "RelyingParty": "rp://api.minecraftservices.com/", "TokenType": "JWT"
        }))
        .send()
        .await?;
    if r.status().as_u16() == 401 {
        let b: Value = r.json().await.unwrap_or_default();
        bail!("{}", xerr_message(b["XErr"].as_u64().unwrap_or(0)));
    }
    let xsts: Value = r.error_for_status()?.json().await?;
    let token = xsts["Token"].as_str().ok_or_else(|| anyhow!("XSTS returned no token"))?;
    let uhs = xsts["DisplayClaims"]["xui"][0]["uhs"].as_str().ok_or_else(|| anyhow!("XSTS returned no user hash"))?;
    let xuid = xsts["DisplayClaims"]["xui"][0]["xid"].as_str().unwrap_or_default().to_string();

    // Minecraft
    let r = c
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&json!({"identityToken": format!("XBL3.0 x={uhs};{token}")}))
        .send()
        .await?;
    if r.status().as_u16() == 403 {
        bail!("Microsoft hasn't approved this client ID for Minecraft yet. Until it's approved, sign-in can't finish.");
    }
    let mc: Value = r.error_for_status().context("Minecraft sign-in failed")?.json().await?;
    let access = mc["access_token"].as_str().ok_or_else(|| anyhow!("Minecraft returned no access token"))?.to_string();

    // Profile (also proves the account owns Java Edition, including via Game Pass).
    let r = c.get("https://api.minecraftservices.com/minecraft/profile").bearer_auth(&access).send().await?;
    if r.status().as_u16() == 404 {
        bail!("This Microsoft account doesn't own Minecraft: Java Edition.");
    }
    let p: Value = r.error_for_status()?.json().await?;
    let uuid = p["id"].as_str().ok_or_else(|| anyhow!("profile has no id"))?.to_string();
    let name = p["name"].as_str().unwrap_or_default().to_string();
    let session = Session { name: name.clone(), uuid: uuid.clone(), access_token: access, xuid: xuid.clone(), client_id: client_id.into() };
    Ok((session, Account { uuid, name, xuid, active: false }))
}

/// Get a fresh game session for the active (or given) account.
pub async fn session(client_id: &str, uuid: Option<&str>) -> Result<Session> {
    let cid = need_client_id(client_id)?;
    let s = load_store();
    let acc = match uuid {
        Some(u) => s.accounts.iter().find(|a| a.uuid == u),
        None => s.accounts.iter().find(|a| a.active).or_else(|| s.accounts.first()),
    }
    .ok_or_else(|| anyhow!("Sign in with a Microsoft account that owns Minecraft to play."))?
    .clone();
    let refresh = get_token(&acc.uuid).map_err(|_| anyhow!("{} needs to sign in again.", acc.name))?;
    let r = super::http()
        .post(format!("{MS_BASE}/token"))
        .form(&[("grant_type", "refresh_token"), ("client_id", cid), ("refresh_token", refresh.as_str()), ("scope", SCOPE)])
        .send()
        .await?;
    let body: Value = r.json().await?;
    if body.get("error").is_some() {
        bail!("{} needs to sign in again ({}).", acc.name, body["error"].as_str().unwrap_or("expired"));
    }
    let t: MsTokens = serde_json::from_value(body)?;
    put_token(&acc.uuid, &t.refresh_token)?;
    let (session, _) = minecraft_login(cid, &t.access_token).await?;
    Ok(session)
}
