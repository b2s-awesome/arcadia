//! Roblox's web APIs: who's signed in, join tickets, game details, public
//! servers and where a server is. Only ever talks to roblox.com hosts, plus
//! geojs.io to place a server's IP address on a map.

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const UA: &str = concat!("Arcadia/", env!("CARGO_PKG_VERSION"), " (https://github.com/b2s-awesome/arcadia)");

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder().user_agent(UA).timeout(Duration::from_secs(20)).build()?)
}

fn cookie_header(cookie: &str) -> String {
    format!(".ROBLOSECURITY={}", cookie.trim())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RobloxUser {
    pub id: u64,
    pub name: String,
    pub display_name: String,
}

/// Who a login cookie belongs to. Fails if it's expired or signed out.
pub async fn whoami(cookie: &str) -> Result<RobloxUser> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Me {
        id: u64,
        name: String,
        display_name: String,
    }
    let r = client()?.get("https://users.roblox.com/v1/users/authenticated").header("Cookie", cookie_header(cookie)).send().await?;
    if r.status() == reqwest::StatusCode::UNAUTHORIZED {
        bail!("This account is signed out. Sign in to it again.");
    }
    let me: Me = r.error_for_status()?.json().await?;
    Ok(RobloxUser { id: me.id, name: me.name, display_name: me.display_name })
}

/// Head-shot pictures for users, by id.
pub async fn avatars(ids: &[u64]) -> Result<Vec<(u64, String)>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Item {
        target_id: u64,
        image_url: Option<String>,
    }
    #[derive(Deserialize)]
    struct Resp {
        data: Vec<Item>,
    }
    let list = ids.iter().map(u64::to_string).collect::<Vec<_>>().join(",");
    let r: Resp = client()?
        .get("https://thumbnails.roblox.com/v1/users/avatar-headshot")
        .query(&[("userIds", list.as_str()), ("size", "150x150"), ("format", "Png"), ("isCircular", "false")])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(r.data.into_iter().filter_map(|i| Some((i.target_id, i.image_url?))).collect())
}

/// POST with Roblox's CSRF dance: the first try returns a token to resend.
async fn post_csrf(c: &reqwest::Client, url: &str, cookie: &str, body: serde_json::Value, extra: &[(&str, &str)]) -> Result<reqwest::Response> {
    let mut token = String::new();
    for _ in 0..2 {
        let mut req = c.post(url).header("Cookie", cookie_header(cookie)).header("Referer", "https://www.roblox.com/").json(&body);
        if !token.is_empty() {
            req = req.header("x-csrf-token", &token);
        }
        for (k, v) in extra {
            req = req.header(*k, *v);
        }
        let r = req.send().await?;
        if r.status() == reqwest::StatusCode::FORBIDDEN {
            if let Some(t) = r.headers().get("x-csrf-token").and_then(|v| v.to_str().ok()) {
                if token.is_empty() {
                    token = t.to_string();
                    continue;
                }
            }
        }
        return Ok(r);
    }
    bail!("Roblox didn't accept the request")
}

/// A one-time ticket that signs the Roblox client in as this account.
pub async fn auth_ticket(cookie: &str) -> Result<String> {
    let c = client()?;
    let r = post_csrf(&c, "https://auth.roblox.com/v1/authentication-ticket", cookie, serde_json::json!({}), &[]).await?;
    if r.status() == reqwest::StatusCode::UNAUTHORIZED {
        bail!("This account is signed out. Sign in to it again.");
    }
    let t = r.headers().get("rbx-authentication-ticket").and_then(|v| v.to_str().ok()).map(str::to_string);
    t.ok_or_else(|| anyhow!("Roblox didn't give a sign-in ticket ({})", r.status()))
}

/// Where to go when the client starts.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// The app's home screen.
    Home,
    Place { place: u64, job: Option<String> },
}

fn enc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// A `roblox-player:` link that starts the client signed in with `ticket`.
pub fn player_uri(ticket: &str, target: &Target, now_ms: u128, tracker: u64) -> String {
    match target {
        Target::Home => format!("roblox-player:1+launchmode:app+gameinfo:{ticket}+launchtime:{now_ms}+browsertrackerid:{tracker}+robloxLocale:en_us+gameLocale:en_us+LaunchExp:InApp"),
        Target::Place { place, job } => {
            let launcher = match job {
                Some(j) => format!("https://www.roblox.com/Game/PlaceLauncher.ashx?request=RequestGameJob&browserTrackerId={tracker}&placeId={place}&gameId={j}&isPlayTogetherGame=false"),
                None => format!("https://www.roblox.com/Game/PlaceLauncher.ashx?request=RequestGame&browserTrackerId={tracker}&placeId={place}&isPlayTogetherGame=false"),
            };
            format!(
                "roblox-player:1+launchmode:play+gameinfo:{ticket}+launchtime:{now_ms}+placelauncherurl:{}+browsertrackerid:{tracker}+robloxLocale:en_us+gameLocale:en_us+LaunchExp:InApp",
                enc(&launcher)
            )
        }
    }
}

/// A deep link that joins with whoever the Roblox app is signed in as.
pub fn deeplink(place: u64, job: Option<&str>) -> String {
    match job {
        Some(j) => format!("roblox://experiences/start?placeId={place}&gameInstanceId={j}"),
        None => format!("roblox://experiences/start?placeId={place}"),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub place_id: u64,
    pub universe_id: u64,
    pub name: String,
    pub creator: String,
    pub icon: Option<String>,
    pub playing: u64,
}

/// Names, creators and icons for places.
pub async fn games(places: &[u64]) -> Result<Vec<Game>> {
    let c = client()?;
    let mut out = Vec::new();
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Universe {
        universe_id: Option<u64>,
    }
    let mut pairs = Vec::new();
    for &p in places.iter().take(50) {
        let u: Universe = c.get(format!("https://apis.roblox.com/universes/v1/places/{p}/universe")).send().await?.error_for_status()?.json().await?;
        if let Some(u) = u.universe_id {
            pairs.push((p, u));
        }
    }
    if pairs.is_empty() {
        return Ok(out);
    }
    let ids = pairs.iter().map(|(_, u)| u.to_string()).collect::<Vec<_>>().join(",");
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Creator {
        name: String,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Detail {
        id: u64,
        name: String,
        creator: Creator,
        #[serde(default)]
        playing: u64,
    }
    #[derive(Deserialize)]
    struct Details {
        data: Vec<Detail>,
    }
    let d: Details = c.get("https://games.roblox.com/v1/games").query(&[("universeIds", ids.as_str())]).send().await?.error_for_status()?.json().await?;
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Icon {
        target_id: u64,
        image_url: Option<String>,
    }
    #[derive(Deserialize)]
    struct Icons {
        data: Vec<Icon>,
    }
    let icons: Icons = c
        .get("https://thumbnails.roblox.com/v1/games/icons")
        .query(&[("universeIds", ids.as_str()), ("size", "150x150"), ("format", "Png"), ("isCircular", "false")])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .unwrap_or(Icons { data: Vec::new() });
    for (p, u) in pairs {
        if let Some(g) = d.data.iter().find(|g| g.id == u) {
            out.push(Game {
                place_id: p,
                universe_id: u,
                name: g.name.clone(),
                creator: g.creator.name.clone(),
                icon: icons.data.iter().find(|i| i.target_id == u).and_then(|i| i.image_url.clone()),
                playing: g.playing,
            });
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    pub id: String,
    pub playing: u32,
    pub max_players: u32,
    #[serde(default)]
    pub ping: Option<u32>,
    #[serde(default)]
    pub fps: Option<f64>,
}

/// One page of a place's public servers, fullest first.
pub async fn servers(place: u64, cursor: Option<&str>) -> Result<(Vec<Server>, Option<String>)> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Page {
        data: Vec<Server>,
        next_page_cursor: Option<String>,
    }
    let mut q: Vec<(&str, &str)> = vec![("sortOrder", "Desc"), ("limit", "100"), ("excludeFullGames", "true")];
    if let Some(c) = cursor {
        q.push(("cursor", c));
    }
    let r = client()?.get(format!("https://games.roblox.com/v1/games/{place}/servers/Public")).query(&q).send().await?;
    if r.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        bail!("Roblox is limiting how fast servers can be listed. Wait a moment and try again.");
    }
    let p: Page = r.error_for_status()?.json().await?;
    Ok((p.data, p.next_page_cursor))
}

/// The address of a running server, as the join service tells the client.
pub async fn server_address(cookie: &str, place: u64, job: &str) -> Result<Option<String>> {
    let c = client()?;
    let body = serde_json::json!({ "placeId": place, "gameId": job, "gameJoinAttemptId": uuid::Uuid::new_v4().to_string() });
    let r = post_csrf(&c, "https://gamejoin.roblox.com/v1/join-game-instance", cookie, body, &[("User-Agent", "Roblox/WinInet")]).await?;
    if !r.status().is_success() {
        return Ok(None);
    }
    let v: serde_json::Value = r.json().await.context("join service reply")?;
    let js = &v["joinScript"];
    let ip = js["UdmuxEndpoints"][0]["Address"].as_str().or_else(|| js["MachineAddress"].as_str());
    Ok(ip.map(str::to_string))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Place {
    pub city: String,
    pub region: String,
    pub country: String,
    /// ISO country code.
    pub cc: String,
}

/// Roughly where an IP address is (city and country).
pub async fn locate(ip: &str) -> Result<Place> {
    if ip.parse::<std::net::IpAddr>().is_err() {
        bail!("Not an IP address");
    }
    #[derive(Deserialize)]
    struct Geo {
        #[serde(default)]
        city: Option<String>,
        #[serde(default)]
        region: Option<String>,
        #[serde(default)]
        country: Option<String>,
        #[serde(default)]
        country_code: Option<String>,
    }
    let g: Geo = client()?.get(format!("https://get.geojs.io/v1/ip/geo/{ip}.json")).send().await?.error_for_status()?.json().await?;
    Ok(Place {
        city: g.city.unwrap_or_default(),
        region: g.region.unwrap_or_default(),
        country: g.country.unwrap_or_default(),
        cc: g.country_code.unwrap_or_default(),
    })
}

/// A place id from a game link, a share link or a bare number.
pub fn place_from(text: &str) -> Option<u64> {
    let t = text.trim();
    if let Ok(n) = t.parse() {
        return Some(n);
    }
    let lower = t.to_ascii_lowercase();
    for key in ["/games/", "placeid="] {
        if let Some(i) = lower.find(key) {
            let digits: String = t[i + key.len()..].chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = digits.parse() {
                return Some(n);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_launch_links() {
        let u = player_uri("TICKET", &Target::Place { place: 920587237, job: Some("abc-1".into()) }, 1700000000000, 42);
        assert!(u.starts_with("roblox-player:1+launchmode:play+gameinfo:TICKET+launchtime:1700000000000+placelauncherurl:https%3A%2F%2Fwww.roblox.com%2FGame%2FPlaceLauncher.ashx%3Frequest%3DRequestGameJob"));
        assert!(u.contains("gameId%3Dabc-1"));
        assert!(!u.contains(' '));
        assert!(player_uri("T", &Target::Home, 1, 2).contains("launchmode:app"));
        assert_eq!(deeplink(5, Some("j")), "roblox://experiences/start?placeId=5&gameInstanceId=j");
    }

    #[test]
    fn reads_place_ids() {
        assert_eq!(place_from("920587237"), Some(920587237));
        assert_eq!(place_from("https://www.roblox.com/games/920587237/Adopt-Me"), Some(920587237));
        assert_eq!(place_from("roblox://experiences/start?placeId=123&x=1"), Some(123));
        assert_eq!(place_from("hello"), None);
    }
}
