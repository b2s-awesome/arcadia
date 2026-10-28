//! End-to-end: client → Arcadia proxy (CONNECT + MITM) → fake Roblox upstream.

use arcadia_core::proxy::ca::CertAuthority;
use arcadia_core::proxy::hooks::file_url;
use arcadia_core::proxy::rules::{Action, Profile, Rule, RuleConfig};
use arcadia_core::proxy::{Proxy, ProxyConfig, TestUpstream};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use serde_json::{json, Value};
use std::convert::Infallible;
use std::sync::Arc;
use tokio::net::TcpListener;

async fn upstream_handler(req: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = req.uri().path_and_query().unwrap().to_string();
    let method = req.method().clone();
    let body = req.into_body().collect().await.unwrap().to_bytes();
    let out = if method == hyper::Method::POST && path.starts_with("/v1/assets/batch") {
        let items: Vec<Value> = serde_json::from_slice(&body).unwrap();
        let resp: Vec<Value> = items
            .iter()
            .map(|i| json!({"requestId": i["requestId"], "location": format!("https://c0.rbxcdn.com/{}", i["assetId"]), "assetTypeId": 1}))
            .collect();
        serde_json::to_vec(&resp).unwrap()
    } else if path.starts_with("/v1/asset") {
        format!("asset-bytes for {path}").into_bytes()
    } else if path.contains("/settings/application/PCDesktopClient") {
        br#"{"applicationSettings":{"FFlagExisting":"True"}}"#.to_vec()
    } else {
        format!("upstream saw {path}").into_bytes()
    };
    Ok(Response::new(Full::new(Bytes::from(out))))
}

/// A TLS server that answers for any hostname (cert chosen by SNI), signed
/// by its own "real world" CA.
async fn start_upstream(ca: Arc<CertAuthority>) -> std::net::SocketAddr {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (s, _) = l.accept().await.unwrap();
            let ca = ca.clone();
            tokio::spawn(async move {
                let acc = tokio_rustls::LazyConfigAcceptor::new(rustls::server::Acceptor::default(), s);
                let Ok(start) = acc.await else { return };
                let sni = start.client_hello().server_name().unwrap_or("localhost").to_string();
                let cfg = ca.server_config(&sni).unwrap();
                let Ok(tls) = start.into_stream(cfg).await else { return };
                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(TokioIo::new(tls), service_fn(upstream_handler))
                    .await;
            });
        }
    });
    addr
}

fn pem_to_der(pem: &str) -> Vec<u8> {
    rustls_pemfile::certs(&mut pem.as_bytes()).next().unwrap().unwrap().to_vec()
}

#[tokio::test(flavor = "multi_thread")]
async fn proxy_end_to_end() {
    let home = tempfile::tempdir().unwrap();
    std::env::set_var("ARCADIA_HOME", home.path());

    let real_ca = Arc::new(CertAuthority::load_or_create(&home.path().join("realca")).unwrap());
    let up_addr = start_upstream(real_ca.clone()).await;

    let proxy = Proxy::start(ProxyConfig {
        port: 0,
        data_dir: home.path().join("proxy"),
        test_upstream: Some(TestUpstream { addr: up_addr, root_der: pem_to_der(real_ca.pem()) }),
    })
    .await
    .unwrap();

    let local_file = home.path().join("sky.png");
    std::fs::write(&local_file, b"MY-CUSTOM-SKYBOX").unwrap();
    proxy.set_rules(&RuleConfig {
        profiles: vec![Profile {
            name: "test".into(),
            enabled: true,
            rules: vec![
                Rule { id: "a".into(), name: "swap".into(), enabled: true, asset_ids: vec![111], action: Action::Replace { with_id: 999 } },
                Rule { id: "b".into(), name: "rm".into(), enabled: true, asset_ids: vec![222], action: Action::Remove },
                Rule {
                    id: "c".into(),
                    name: "file".into(),
                    enabled: true,
                    asset_ids: vec![333],
                    action: Action::File { path: local_file.to_string_lossy().into() },
                },
            ],
        }],
    });
    let mut flags = serde_json::Map::new();
    flags.insert("DFIntTaskSchedulerTargetFps".into(), json!(360));
    proxy.set_fflags(flags);

    // The client trusts Arcadia's CA (as Roblox does via ssl/cacert.pem) and
    // the upstream CA (standing in for real public roots).
    let client = reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{}", proxy.port)).unwrap())
        .add_root_certificate(reqwest::Certificate::from_pem(proxy.ca_pem().as_bytes()).unwrap())
        .add_root_certificate(reqwest::Certificate::from_pem(real_ca.pem().as_bytes()).unwrap())
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    // ── batch: replace, remove, local file, untouched ──
    let batch = json!([
        {"assetId": 111, "requestId": "0"},
        {"assetId": 222, "requestId": "1"},
        {"assetId": 333, "requestId": "2"},
        {"assetId": 444, "requestId": "3"}
    ]);
    let r: Vec<Value> = client
        .post("https://assetdelivery.roblox.com/v1/assets/batch")
        .json(&batch)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(r.len(), 3, "removed asset must be stripped: {r:?}");
    assert_eq!(r[0]["location"], "https://c0.rbxcdn.com/999");
    assert_eq!(r[1]["location"], file_url(333));
    assert_eq!(r[2]["location"], "https://c0.rbxcdn.com/444");

    // ── local file served at the rewritten location ──
    let f = client.get(file_url(333)).send().await.unwrap().bytes().await.unwrap();
    assert_eq!(&f[..], b"MY-CUSTOM-SKYBOX");

    // ── single-asset GETs ──
    let s = client.get("https://assetdelivery.roblox.com/v1/asset/?id=111").send().await.unwrap().text().await.unwrap();
    assert_eq!(s, "asset-bytes for /v1/asset/?id=999");
    let st = client.get("https://assetdelivery.roblox.com/v1/asset/?id=222").send().await.unwrap().status();
    assert_eq!(st, 404);

    // ── live FastFlags ──
    let v: Value = client
        .get("https://clientsettingscdn.roblox.com/v2/settings/application/PCDesktopClient")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(v["applicationSettings"]["DFIntTaskSchedulerTargetFps"], "360");
    assert_eq!(v["applicationSettings"]["FFlagExisting"], "True");

    // ── other hosts are tunneled, never decrypted ──
    // This client trusts ONLY the upstream CA, so it would fail if the proxy
    // had swapped in its own certificate.
    let strict = reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{}", proxy.port)).unwrap())
        .tls_built_in_root_certs(false)
        .add_root_certificate(reqwest::Certificate::from_pem(real_ca.pem().as_bytes()).unwrap())
        .build()
        .unwrap();
    let t = strict.get("https://auth.roblox.com/v2/login").send().await.unwrap().text().await.unwrap();
    assert_eq!(t, "upstream saw /v2/login");

    // ── inspector ──
    drop(strict);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let stats = proxy.inspector().stats();
    assert_eq!(stats.replaced, 2);
    assert_eq!(stats.removed, 2);
    assert_eq!(stats.served_files, 1);
    assert_eq!(stats.fflag_injections, 1);
    let log = proxy.inspector().snapshot();
    assert!(log.iter().any(|e| e.kind == "modified" && e.note.contains("replaced 1")));
    assert!(log.iter().any(|e| e.kind == "tunnel" && e.url.starts_with("auth.roblox.com")), "{log:#?}");
    proxy.stop();
}
