//! Discord lookups for the Profile Studio addon.
//!
//! Only official, documented sources are used:
//!
//! * **Discord's API with the user's own bot token.** `GET /users/{id}` gives
//!   avatar, banner, banner colour and avatar decoration. A username is turned
//!   into an id by searching the members of servers the bot is in, so the
//!   person has to share a server with that bot.
//! * **Lanyard** (`api.lanyard.rest`) for a user id with no token, which only
//!   knows people who joined Lanyard's Discord server. No banner.
//!
//! Bio, pronouns and Nitro profile colours aren't exposed to bots at all, so
//! those are always edited by hand in the studio. Nothing here uses a user
//! account token: that's a self-bot, which Discord bans.

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const API: &str = "https://discord.com/api/v10";
const CDN: &str = "https://cdn.discordapp.com";
const LANYARD: &str = "https://api.lanyard.rest/v1/users";
/// Discord's epoch (2015-01-01) in Unix milliseconds.
const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;
/// How many of the bot's servers to search for a username.
const MAX_GUILDS_SEARCHED: usize = 25;
const MAX_IMAGE_BYTES: usize = 12 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Query {
    Id(String),
    Name(String),
}

/// Accepts a user id, a `discord.com/users/<id>` link, `@name`, `name`, or
/// a legacy `name#1234`.
pub fn parse_query(raw: &str) -> Result<Query> {
    let s = raw.trim();
    if s.is_empty() {
        bail!("Type a Discord username or user ID");
    }
    if let Some(i) = s.find("/users/") {
        let id: String = s[i + 7..].chars().take_while(|c| c.is_ascii_digit()).collect();
        if is_snowflake(&id) {
            return Ok(Query::Id(id));
        }
    }
    if s.starts_with("<@") && s.ends_with('>') {
        let id: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
        if is_snowflake(&id) {
            return Ok(Query::Id(id));
        }
    }
    let s = s.trim_start_matches('@');
    if is_snowflake(s) {
        return Ok(Query::Id(s.to_string()));
    }
    let name = match s.split_once('#') {
        Some((n, d)) if d.chars().all(|c| c.is_ascii_digit()) => n,
        _ => s,
    }
    .to_lowercase();
    let valid = (2..=32).contains(&name.len())
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.')
        && !name.contains("..");
    if !valid {
        bail!("“{raw}” isn't a Discord username. Usernames are 2–32 characters: letters, numbers, _ and .");
    }
    Ok(Query::Name(name))
}

fn is_snowflake(s: &str) -> bool {
    (17..=20).contains(&s.len()) && s.chars().all(|c| c.is_ascii_digit())
}

/// When the account was created, from its id.
pub fn created_at_ms(id: &str) -> Option<u64> {
    id.parse::<u64>().ok().map(|n| (n >> 22) + DISCORD_EPOCH_MS)
}

fn default_avatar(id: &str, discriminator: Option<&str>) -> String {
    let index = match discriminator.and_then(|d| d.parse::<u64>().ok()) {
        Some(d) if d != 0 => d % 5,
        _ => id.parse::<u64>().map(|n| (n >> 22) % 6).unwrap_or(0),
    };
    format!("{CDN}/embed/avatars/{index}.png")
}

#[derive(Debug, Clone, Deserialize)]
struct ApiUser {
    id: String,
    username: String,
    #[serde(default)]
    global_name: Option<String>,
    #[serde(default)]
    discriminator: Option<String>,
    #[serde(default)]
    avatar: Option<String>,
    #[serde(default)]
    banner: Option<String>,
    #[serde(default)]
    accent_color: Option<u32>,
    #[serde(default)]
    avatar_decoration_data: Option<Decoration>,
    #[serde(default)]
    bot: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct Decoration {
    asset: String,
}

/// A looked-up profile, with image URLs ready to fetch.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub id: String,
    pub username: String,
    pub display_name: Option<String>,
    pub avatar_url: String,
    pub avatar_animated: bool,
    pub banner_url: Option<String>,
    pub banner_animated: bool,
    /// The plain banner colour, `#rrggbb`.
    pub accent_color: Option<String>,
    pub decoration_url: Option<String>,
    pub created_at: Option<u64>,
    pub bot: bool,
    /// "discord" or "lanyard".
    pub source: String,
    /// Things the user should know, e.g. "banner needs a bot token".
    pub notes: Vec<String>,
}

fn to_found(u: ApiUser, source: &str) -> Found {
    let avatar_animated = u.avatar.as_deref().is_some_and(|h| h.starts_with("a_"));
    let banner_animated = u.banner.as_deref().is_some_and(|h| h.starts_with("a_"));
    Found {
        avatar_url: match &u.avatar {
            Some(h) => format!("{CDN}/avatars/{}/{h}.{}?size=512", u.id, if avatar_animated { "gif" } else { "png" }),
            None => default_avatar(&u.id, u.discriminator.as_deref()),
        },
        avatar_animated,
        banner_url: u
            .banner
            .as_ref()
            .map(|h| format!("{CDN}/banners/{}/{h}.{}?size=1024", u.id, if banner_animated { "gif" } else { "png" })),
        banner_animated,
        accent_color: u.accent_color.map(|c| format!("#{:06x}", c & 0xff_ffff)),
        decoration_url: u
            .avatar_decoration_data
            .as_ref()
            .map(|d| format!("{CDN}/avatar-decoration-presets/{}.png?size=240&passthrough=true", d.asset)),
        created_at: created_at_ms(&u.id),
        display_name: u.global_name.filter(|n| !n.trim().is_empty()),
        username: u.username,
        id: u.id,
        bot: u.bot,
        source: source.to_string(),
        notes: Vec::new(),
    }
}

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(concat!("DiscordBot (https://github.com/b2s-awesome/arcadia, ", env!("CARGO_PKG_VERSION"), ") Arcadia"))
        .timeout(Duration::from_secs(15))
        .build()?)
}

/// GET a Discord API path as the bot, waiting out one rate limit.
async fn api_get<T: serde::de::DeserializeOwned>(c: &reqwest::Client, token: &str, path: &str) -> Result<T> {
    for attempt in 0..2 {
        let r = c.get(format!("{API}{path}")).header("Authorization", format!("Bot {token}")).send().await?;
        let status = r.status().as_u16();
        match status {
            200..=299 => return Ok(r.json().await?),
            401 => bail!("Discord rejected the bot token. Copy a fresh one from the Developer Portal (Bot → Reset Token)."),
            403 => bail!("forbidden"),
            404 => bail!("not-found"),
            429 if attempt == 0 => {
                #[derive(Deserialize)]
                struct Limit {
                    retry_after: f64,
                }
                let wait = r.json::<Limit>().await.map(|l| l.retry_after).unwrap_or(1.0).clamp(0.1, 5.0);
                tokio::time::sleep(Duration::from_secs_f64(wait)).await;
            }
            _ => bail!("Discord answered {status}"),
        }
    }
    bail!("Discord is rate limiting lookups; try again in a moment")
}

async fn user_by_id(c: &reqwest::Client, token: &str, id: &str) -> Result<ApiUser> {
    api_get(c, token, &format!("/users/{id}")).await.map_err(|e| match e.to_string().as_str() {
        "not-found" => anyhow!("No Discord account has the ID {id}"),
        _ => e,
    })
}

/// Find a username among members of the bot's servers.
async fn id_for_name(c: &reqwest::Client, token: &str, name: &str) -> Result<String> {
    #[derive(Deserialize)]
    struct Guild {
        id: String,
    }
    #[derive(Deserialize)]
    struct Member {
        user: ApiUser,
    }
    let guilds: Vec<Guild> = api_get(c, token, "/users/@me/guilds?limit=200").await?;
    if guilds.is_empty() {
        bail!("Your bot isn't in any servers, so it can't find people by username. Invite it to a server you share with them, or paste their user ID instead.");
    }
    let mut forbidden = 0;
    let mut near: Option<String> = None;
    for g in guilds.iter().take(MAX_GUILDS_SEARCHED) {
        let path = format!("/guilds/{}/members/search?query={}&limit=10", g.id, urlencode(name));
        let members: Vec<Member> = match api_get(c, token, &path).await {
            Ok(m) => m,
            Err(e) if e.to_string() == "forbidden" => {
                forbidden += 1;
                continue;
            }
            Err(e) => return Err(e),
        };
        if let Some(m) = members.iter().find(|m| m.user.username.eq_ignore_ascii_case(name)) {
            return Ok(m.user.id.clone());
        }
        if near.is_none() {
            near = members
                .iter()
                .find(|m| m.user.global_name.as_deref().is_some_and(|g| g.eq_ignore_ascii_case(name)))
                .map(|m| m.user.id.clone());
        }
    }
    if let Some(id) = near {
        return Ok(id);
    }
    if forbidden > 0 && forbidden == guilds.len().min(MAX_GUILDS_SEARCHED) {
        bail!("Your bot can't search members. In the Developer Portal, open your app → Bot and turn on “Server Members Intent”.");
    }
    bail!("No one called “{name}” shares a server with your bot. Paste their user ID instead (Discord: Developer Mode → right-click them → Copy User ID).")
}

fn urlencode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

async fn lanyard(c: &reqwest::Client, id: &str) -> Result<ApiUser> {
    #[derive(Deserialize)]
    struct Resp {
        success: bool,
        data: Option<Data>,
    }
    #[derive(Deserialize)]
    struct Data {
        discord_user: ApiUser,
    }
    let r = c.get(format!("{LANYARD}/{id}")).send().await.context("Couldn't reach Lanyard")?;
    let body: Resp = r.json().await.context("Lanyard sent something unexpected")?;
    match body {
        Resp { success: true, data: Some(d) } => Ok(d.discord_user),
        _ => bail!(
            "Without a bot token Arcadia can only look up people who've joined Lanyard's Discord server, and this ID isn't one of them. Add a bot token in the studio's import settings to look anyone up."
        ),
    }
}

/// Look someone up. `token` is the user's own bot token, if they gave one.
pub async fn lookup(raw: &str, token: Option<&str>) -> Result<Found> {
    let q = parse_query(raw)?;
    let c = client()?;
    let token = token.map(str::trim).filter(|t| !t.is_empty());
    match (q, token) {
        (Query::Id(id), Some(t)) => Ok(to_found(user_by_id(&c, t, &id).await?, "discord")),
        (Query::Name(name), Some(t)) => {
            let id = id_for_name(&c, t, &name).await?;
            Ok(to_found(user_by_id(&c, t, &id).await?, "discord"))
        }
        (Query::Id(id), None) => {
            let mut f = to_found(lanyard(&c, &id).await?, "lanyard");
            f.notes.push("Banners only come through with a bot token, so add yours to pull the banner too.".into());
            Ok(f)
        }
        (Query::Name(_), None) => bail!(
            "Finding someone by username needs a bot token (Discord doesn't offer a public username search). Add one under “Import settings”, or paste a user ID."
        ),
    }
}

/// Download a Discord CDN image as a `data:` URL, so the studio can store it
/// and read its pixels for palettes.
pub async fn fetch_image(url: &str) -> Result<String> {
    let u = url::Url::parse(url).context("Not a link")?;
    let host = u.host_str().unwrap_or_default();
    if u.scheme() != "https" || !matches!(host, "cdn.discordapp.com" | "media.discordapp.net") {
        bail!("Only Discord's image CDN can be fetched");
    }
    let r = client()?.get(u).send().await?.error_for_status()?;
    let mime = r
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.split(';').next().unwrap_or("").trim().to_string())
        .filter(|m| m.starts_with("image/"))
        .unwrap_or_else(|| "image/png".into());
    let bytes = r.bytes().await?;
    if bytes.len() > MAX_IMAGE_BYTES {
        bail!("That image is over {} MB", MAX_IMAGE_BYTES / 1_048_576);
    }
    Ok(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(&bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries() {
        assert_eq!(parse_query("  @Some.Name ").unwrap(), Query::Name("some.name".into()));
        assert_eq!(parse_query("old#1234").unwrap(), Query::Name("old".into()));
        assert_eq!(parse_query("80351110224678912").unwrap(), Query::Id("80351110224678912".into()));
        assert_eq!(parse_query("<@80351110224678912>").unwrap(), Query::Id("80351110224678912".into()));
        assert_eq!(
            parse_query("https://discord.com/users/80351110224678912").unwrap(),
            Query::Id("80351110224678912".into())
        );
        assert!(parse_query("").is_err());
        assert!(parse_query("a").is_err());
        assert!(parse_query("has space").is_err());
        assert!(parse_query("a..b").is_err());
    }

    #[test]
    fn snowflake_dates_and_default_avatars() {
        // Discord's documented example snowflake: 2016-04-30 11:18:25.796 UTC.
        assert_eq!(created_at_ms("175928847299117063"), Some(1_462_015_105_796));
        assert!(default_avatar("175928847299117063", Some("0")).ends_with(&format!("/{}.png", (175928847299117063u64 >> 22) % 6)));
        assert!(default_avatar("1", Some("1337")).ends_with("/2.png"));
    }

    #[test]
    fn found_urls() {
        let u: ApiUser = serde_json::from_value(serde_json::json!({
            "id": "175928847299117063", "username": "nelly", "global_name": "Nelly",
            "avatar": "a_abc", "banner": "def", "accent_color": 16711680,
            "avatar_decoration_data": { "asset": "a_deco", "sku_id": "1" }
        }))
        .unwrap();
        let f = to_found(u, "discord");
        assert!(f.avatar_animated && f.avatar_url.contains("/avatars/175928847299117063/a_abc.gif"));
        assert_eq!(f.banner_url.as_deref(), Some("https://cdn.discordapp.com/banners/175928847299117063/def.png?size=1024"));
        assert_eq!(f.accent_color.as_deref(), Some("#ff0000"));
        assert!(f.decoration_url.unwrap().contains("avatar-decoration-presets/a_deco.png"));
        assert_eq!(f.display_name.as_deref(), Some("Nelly"));
    }

    #[tokio::test]
    async fn image_fetch_is_limited_to_discord() {
        assert!(fetch_image("https://example.com/a.png").await.is_err());
        assert!(fetch_image("http://cdn.discordapp.com/a.png").await.is_err());
    }
}
