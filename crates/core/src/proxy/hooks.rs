//! Pure request/response transforms for the Roblox endpoints the proxy
//! intercepts. Kept free of I/O so they're easy to test.

use super::rules::Action;
use serde_json::{Map, Value};
use std::collections::HashMap;

pub const ASSET_HOST: &str = "assetdelivery.roblox.com";
pub const FILE_PREFIX: &str = "/__arcadia/file/";

/// Hosts whose TLS the proxy terminates. Everything else is tunneled
/// untouched (so logins, purchases, chat etc. are never decrypted).
pub const INTERCEPT_HOSTS: &[&str] = &[ASSET_HOST, "clientsettingscdn.roblox.com", "clientsettings.roblox.com"];

pub fn file_url(asset_id: u64) -> String {
    format!("https://{ASSET_HOST}{FILE_PREFIX}{asset_id}")
}

fn as_u64(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
}

fn request_key(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

#[derive(Debug, Default)]
pub struct BatchRewrite {
    pub body: Vec<u8>,
    /// requestId → the URL its `location` must point at (a local file Arcadia
    /// serves, or a web link).
    pub location_overrides: HashMap<String, String>,
    pub replaced: usize,
    pub removed: usize,
    pub files: usize,
    pub links: usize,
}

/// Rewrite a `POST /v1/assets/batch` body: swap replaced ids, drop removed
/// ones, and remember which entries need a local-file location.
pub fn rewrite_batch_request(body: &[u8], rules: &HashMap<u64, Action>) -> Option<BatchRewrite> {
    let Value::Array(items) = serde_json::from_slice::<Value>(body).ok()? else { return None };
    let mut out = BatchRewrite::default();
    let mut kept = Vec::with_capacity(items.len());
    for mut item in items {
        let id = item.get("assetId").and_then(as_u64);
        match id.and_then(|id| rules.get(&id).map(|a| (id, a))) {
            Some((_, Action::Remove)) => {
                out.removed += 1;
                continue;
            }
            Some((_, Action::Replace { with_id })) => {
                if let Some(o) = item.as_object_mut() {
                    o.insert("assetId".into(), Value::from(*with_id));
                    out.replaced += 1;
                }
            }
            Some((id, Action::File { .. })) => {
                if let Some(k) = item.get("requestId").and_then(request_key) {
                    out.location_overrides.insert(k, file_url(id));
                    out.files += 1;
                }
            }
            Some((_, Action::Link { url })) => {
                if let Some(k) = item.get("requestId").and_then(request_key) {
                    out.location_overrides.insert(k, url.clone());
                    out.links += 1;
                }
            }
            None => {}
        }
        kept.push(item);
    }
    if out.replaced + out.removed + out.files + out.links == 0 {
        return None;
    }
    out.body = serde_json::to_vec(&Value::Array(kept)).ok()?;
    Some(out)
}

/// Point every `location` field inside `v` at `url`.
pub fn rewrite_locations(v: &mut Value, url: &str) {
    match v {
        Value::Object(m) => {
            for (k, val) in m.iter_mut() {
                if k.eq_ignore_ascii_case("location") && val.is_string() {
                    *val = Value::String(url.to_string());
                } else {
                    rewrite_locations(val, url);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| rewrite_locations(x, url)),
        _ => {}
    }
}

/// Fix up the batch response so file- and link-replaced entries load from
/// their new location.
pub fn rewrite_batch_response(body: &[u8], overrides: &HashMap<String, String>) -> Option<Vec<u8>> {
    if overrides.is_empty() {
        return None;
    }
    let Value::Array(mut items) = serde_json::from_slice::<Value>(body).ok()? else { return None };
    for item in items.iter_mut() {
        let Some(url) = item.get("requestId").and_then(request_key).and_then(|k| overrides.get(&k)).cloned() else {
            continue;
        };
        if let Some(o) = item.as_object_mut() {
            // A failed upstream lookup still gets our file.
            o.remove("errors");
            o.insert("location".into(), Value::String(url.clone()));
        }
        rewrite_locations(item, &url);
    }
    serde_json::to_vec(&Value::Array(items)).ok()
}

/// For single-asset GET endpoints, find the asset id being requested.
///   /v1/asset?id=N   /v1/asset/?ID=N   /v2/asset?id=N
///   /v1/assetId/N    /v2/assetId/N
pub fn single_asset_id(path_and_query: &str) -> Option<u64> {
    let (path, query) = path_and_query.split_once('?').unwrap_or((path_and_query, ""));
    let lp = path.to_ascii_lowercase();
    if lp.starts_with("/v1/assetid/") || lp.starts_with("/v2/assetid/") {
        return path.rsplit('/').find(|s| !s.is_empty())?.parse().ok();
    }
    if lp.trim_end_matches('/') == "/v1/asset" || lp.trim_end_matches('/') == "/v2/asset" {
        for kv in query.split('&') {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            if k.eq_ignore_ascii_case("id") {
                return v.parse().ok();
            }
        }
    }
    None
}

/// Replace the asset id in a single-asset path/query.
pub fn swap_single_asset_id(path_and_query: &str, from: u64, to: u64) -> String {
    let (path, query) = path_and_query.split_once('?').unwrap_or((path_and_query, ""));
    let lp = path.to_ascii_lowercase();
    if lp.starts_with("/v1/assetid/") || lp.starts_with("/v2/assetid/") {
        let p = path.replacen(&format!("/{from}"), &format!("/{to}"), 1);
        return if query.is_empty() { p } else { format!("{p}?{query}") };
    }
    let q: Vec<String> = query
        .split('&')
        .map(|kv| match kv.split_once('=') {
            Some((k, _)) if k.eq_ignore_ascii_case("id") => format!("{k}={to}"),
            _ => kv.to_string(),
        })
        .collect();
    format!("{path}?{}", q.join("&"))
}

/// A Roblox ClientSettings document the player reads its server flags from.
/// The bootstrapper's own document (`PCClientBootstrapper`) is left alone.
pub fn is_client_settings(host: &str, path: &str) -> bool {
    let path = path.split('?').next().unwrap_or(path);
    host.starts_with("clientsettings")
        && (path.contains("/settings/application/") || path.contains("/settings-compressed/application/"))
        && !path.contains("PCClientBootstrapper")
}

/// A stable fingerprint of a flag set, to tell whether Roblox already has it.
pub fn flag_signature(flags: &Map<String, Value>) -> String {
    let mut v: Vec<(String, String)> =
        flags.iter().map(|(k, v)| (k.clone(), crate::roblox::flagcache::flag_string(v))).collect();
    v.sort();
    serde_json::to_string(&v).unwrap_or_default()
}

/// Merge live FastFlag overrides into a ClientSettings response.
pub fn inject_fflags(body: &[u8], overrides: &Map<String, Value>) -> Option<(Vec<u8>, usize)> {
    if overrides.is_empty() {
        return None;
    }
    let mut v: Value = serde_json::from_slice(body).ok()?;
    let target = match v.get_mut("applicationSettings") {
        Some(Value::Object(m)) => m,
        _ => v.as_object_mut()?,
    };
    for (k, val) in overrides {
        // Roblox serves every flag value as a string.
        target.insert(k.clone(), Value::String(crate::roblox::flagcache::flag_string(val)));
    }
    Some((serde_json::to_vec(&v).ok()?, overrides.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rules() -> HashMap<u64, Action> {
        HashMap::from([
            (111, Action::Replace { with_id: 999 }),
            (222, Action::Remove),
            (333, Action::File { path: "x.png".into() }),
            (555, Action::Link { url: "https://example.com/a.png".into() }),
        ])
    }

    #[test]
    fn batch_request_rewrite() {
        let body = json!([
            {"assetId": 111, "requestId": "0"},
            {"assetId": 222, "requestId": "1"},
            {"assetId": 333, "requestId": "2"},
            {"assetId": 444, "requestId": "3"},
            {"assetId": 555, "requestId": "4"}
        ]);
        let r = rewrite_batch_request(body.to_string().as_bytes(), &rules()).unwrap();
        let v: Value = serde_json::from_slice(&r.body).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 4);
        assert_eq!(v[0]["assetId"], 999);
        assert_eq!(v[1]["assetId"], 333);
        assert_eq!((r.replaced, r.removed, r.files, r.links), (1, 1, 1, 1));
        assert_eq!(r.location_overrides["2"], file_url(333));
        assert_eq!(r.location_overrides["4"], "https://example.com/a.png");

        let resp = json!([{"requestId": "0", "location": "https://c0.rbxcdn.com/a"},
                          {"requestId": "2", "location": "https://c0.rbxcdn.com/b"},
                          {"requestId": "4", "errors": [{"code": 403}]}]);
        let out = rewrite_batch_response(resp.to_string().as_bytes(), &r.location_overrides).unwrap();
        let v: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v[0]["location"], "https://c0.rbxcdn.com/a");
        assert_eq!(v[1]["location"], file_url(333));
        assert_eq!(v[2]["location"], "https://example.com/a.png");
        assert!(v[2].get("errors").is_none());
    }

    #[test]
    fn untouched_batch_returns_none() {
        assert!(rewrite_batch_request(br#"[{"assetId":5,"requestId":"0"}]"#, &rules()).is_none());
    }

    #[test]
    fn single_asset_paths() {
        assert_eq!(single_asset_id("/v1/asset/?id=123&serverplaceid=0"), Some(123));
        assert_eq!(single_asset_id("/v1/asset?ID=55"), Some(55));
        assert_eq!(single_asset_id("/v2/assetId/77"), Some(77));
        assert_eq!(single_asset_id("/v1/assets/batch"), None);
        assert_eq!(swap_single_asset_id("/v1/asset/?id=123&x=1", 123, 9), "/v1/asset/?id=9&x=1");
        assert_eq!(swap_single_asset_id("/v2/assetId/77", 77, 8), "/v2/assetId/8");
    }

    #[test]
    fn recognises_player_settings_only() {
        assert!(is_client_settings("clientsettingscdn.roblox.com", "/v2/settings/application/PCDesktopClient"));
        assert!(is_client_settings("clientsettings.roblox.com", "/v2/settings-compressed/application/PCDesktopClient?x=1"));
        assert!(!is_client_settings("clientsettingscdn.roblox.com", "/v2/settings/application/PCClientBootstrapper"));
        assert!(!is_client_settings("assetdelivery.roblox.com", "/v2/settings/application/PCDesktopClient"));
        let mut a = Map::new();
        a.insert("B".into(), json!(true));
        a.insert("A".into(), json!("1"));
        let mut b = Map::new();
        b.insert("A".into(), json!(1));
        b.insert("B".into(), json!("True"));
        assert_eq!(flag_signature(&a), flag_signature(&b));
    }

    #[test]
    fn fflag_injection() {
        let mut o = Map::new();
        o.insert("FFlagA".into(), json!(true));
        o.insert("DFIntB".into(), json!(5));
        let (b, n) = inject_fflags(br#"{"applicationSettings":{"FFlagA":"False","X":"1"}}"#, &o).unwrap();
        let v: Value = serde_json::from_slice(&b).unwrap();
        assert_eq!(n, 2);
        assert_eq!(v["applicationSettings"]["FFlagA"], "True");
        assert_eq!(v["applicationSettings"]["DFIntB"], "5");
        assert_eq!(v["applicationSettings"]["X"], "1");
    }
}
