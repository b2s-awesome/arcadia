//! Installer against a local fake Roblox deployment CDN.

use arcadia_core::roblox::deploy;
use bytes::Bytes;
use http_body_util::Full;
use hyper::service::service_fn;
use hyper::{Response, StatusCode};
use hyper_util::rt::TokioIo;
use md5::{Digest, Md5};
use std::collections::HashMap;
use std::convert::Infallible;
use std::io::Write;
use std::sync::{Arc, Mutex};

fn make_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let opt = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in files {
            z.start_file(*name, opt).unwrap();
            z.write_all(data).unwrap();
        }
        z.finish().unwrap();
    }
    buf.into_inner()
}

async fn serve(files: Arc<HashMap<String, Vec<u8>>>, hits: Arc<Mutex<Vec<String>>>) -> u16 {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let (s, _) = l.accept().await.unwrap();
            let files = files.clone();
            let hits = hits.clone();
            tokio::spawn(async move {
                let svc = service_fn(move |req: hyper::Request<hyper::body::Incoming>| {
                    let p = req.uri().path().to_string();
                    hits.lock().unwrap().push(p.clone());
                    let r = match files.get(&p) {
                        Some(b) => Response::new(Full::new(Bytes::from(b.clone()))),
                        None => {
                            let mut r = Response::new(Full::new(Bytes::new()));
                            *r.status_mut() = StatusCode::NOT_FOUND;
                            r
                        }
                    };
                    async move { Ok::<_, Infallible>(r) }
                });
                let _ = hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(s), svc).await;
            });
        }
    });
    port
}

#[tokio::test(flavor = "multi_thread")]
async fn installs_from_cdn_and_reuses_cache() {
    let home = tempfile::tempdir().unwrap();
    std::env::set_var("ARCADIA_HOME", home.path());

    let ver = "version-0123456789abcdef";
    let packages: Vec<(&str, Vec<u8>)> = vec![
        ("RobloxApp.zip", make_zip(&[("RobloxPlayerBeta.exe", b"MZ fake exe"), ("RobloxPlayerBeta.dll", b"dll")])),
        ("ssl.zip", make_zip(&[("cacert.pem", b"-----BEGIN CERTIFICATE-----\nROOTS\n")])),
        ("content-fonts.zip", make_zip(&[(r"families\Arial.json", br#"{"faces":[]}"#), ("arial.ttf", b"ttf")])),
        // Unknown package names are ignored rather than dumped in the root.
        ("RobloxPlayerLauncher.exe", b"launcher".to_vec()),
    ];
    let mut manifest = String::from("v0\n");
    let mut files = HashMap::new();
    for (name, data) in &packages {
        manifest += &format!("{name}\n{:x}\n{}\n{}\n", Md5::digest(data), data.len(), data.len() * 2);
        files.insert(format!("/{ver}-{name}"), data.clone());
    }
    files.insert(format!("/{ver}-rbxPkgManifest.txt"), manifest.into_bytes());
    let hits = Arc::new(Mutex::new(Vec::new()));
    let port = serve(Arc::new(files), hits.clone()).await;
    std::env::set_var("ARCADIA_SETUP_CDN", format!("http://127.0.0.1:{port}"));

    let stages = Arc::new(Mutex::new(Vec::new()));
    let s2 = stages.clone();
    let dir = deploy::install(ver, "production", &move |p| s2.lock().unwrap().push(format!("{p:?}")))
        .await
        .unwrap();

    assert!(dir.join("RobloxPlayerBeta.exe").exists());
    assert_eq!(std::fs::read(dir.join("ssl/cacert.pem")).unwrap(), b"-----BEGIN CERTIFICATE-----\nROOTS\n");
    assert!(dir.join("content/fonts/families/Arial.json").exists(), "backslash paths normalised");
    assert!(dir.join("AppSettings.xml").exists());
    assert!(!dir.join("RobloxPlayerLauncher.exe").exists());
    assert_eq!(deploy::installed().version.as_deref(), Some(ver));
    assert!(stages.lock().unwrap().last().unwrap().contains("Ready"));

    // Second install of the same version: nothing re-downloaded.
    let before = hits.lock().unwrap().len();
    deploy::install(ver, "production", &|_| {}).await.unwrap();
    assert_eq!(hits.lock().unwrap().len(), before);

    // Corrupted package is rejected.
    deploy::force_reinstall().unwrap();
    std::fs::remove_dir_all(arcadia_core::paths::roblox_downloads_dir()).unwrap();
    std::fs::create_dir_all(arcadia_core::paths::roblox_downloads_dir()).unwrap();
    let bad = "version-badbadbadbad0000";
    let app = make_zip(&[("RobloxPlayerBeta.exe", b"x")]);
    let mut files2 = HashMap::new();
    files2.insert(format!("/{bad}-rbxPkgManifest.txt"), format!("v0\nRobloxApp.zip\n{:032x}\n1\n1\n", 0).into_bytes());
    files2.insert(format!("/{bad}-RobloxApp.zip"), app);
    let port2 = serve(Arc::new(files2), Arc::new(Mutex::new(vec![]))).await;
    std::env::set_var("ARCADIA_SETUP_CDN", format!("http://127.0.0.1:{port2}"));
    let err = deploy::install(bad, "production", &|_| {}).await.unwrap_err();
    assert!(format!("{err:#}").contains("checksum mismatch"), "{err:#}");
}
