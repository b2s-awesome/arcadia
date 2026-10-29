//! Full Minecraft pipeline against a fake Mojang / Fabric / Modrinth server:
//! create a Fabric instance, install everything, build the launch command,
//! and import a Modrinth pack (with a tampered file that must be rejected).

use arcadia_core::minecraft::{self, import, instance, launch, loaders::Loader, OnProgress};
use bytes::Bytes;
use http_body_util::Full;
use hyper::service::service_fn;
use hyper::{Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde_json::json;
use sha1::{Digest, Sha1};
use sha2::Sha512;
use std::collections::HashMap;
use std::convert::Infallible;
use std::io::Write;
use std::sync::{Arc, Mutex};

type Files = Arc<Mutex<HashMap<String, Vec<u8>>>>;

async fn serve(files: Files) -> String {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let (s, _) = l.accept().await.unwrap();
            let files = files.clone();
            tokio::spawn(async move {
                let svc = service_fn(move |req: hyper::Request<hyper::body::Incoming>| {
                    let body = files.lock().unwrap().get(req.uri().path()).cloned();
                    async move {
                        Ok::<_, Infallible>(match body {
                            Some(b) => Response::new(Full::new(Bytes::from(b))),
                            None => {
                                let mut r = Response::new(Full::new(Bytes::new()));
                                *r.status_mut() = StatusCode::NOT_FOUND;
                                r
                            }
                        })
                    }
                });
                let _ = hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(s), svc).await;
            });
        }
    });
    format!("http://127.0.0.1:{port}")
}

fn sha1(b: &[u8]) -> String {
    hex::encode(Sha1::digest(b))
}
fn sha512(b: &[u8]) -> String {
    hex::encode(Sha512::digest(b))
}

fn jar(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut c = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut c);
        for (n, d) in files {
            z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(d).unwrap();
        }
        z.finish().unwrap();
    }
    c.into_inner()
}

#[tokio::test(flavor = "multi_thread")]
async fn install_fabric_build_launch_and_import_mrpack() {
    let home = tempfile::tempdir().unwrap();
    std::env::set_var("ARCADIA_HOME", home.path());
    let files: Files = Arc::new(Mutex::new(HashMap::new()));
    let base = serve(files.clone()).await;
    let put = |p: &str, b: Vec<u8>| files.lock().unwrap().insert(p.to_string(), b);

    for (k, path) in [
        ("ARCADIA_MC_MANIFEST", "/mc/manifest.json"),
        ("ARCADIA_MC_RESOURCES", "/res"),
        ("ARCADIA_MC_LIBRARIES", "/libs"),
        ("ARCADIA_MC_JAVA", "/java/all.json"),
        ("ARCADIA_FABRIC_META", "/fabric"),
        ("ARCADIA_MODRINTH", "/modrinth"),
    ] {
        std::env::set_var(k, format!("{base}{path}"));
    }

    // ── Vanilla 1.21.1 ──
    let client_jar = b"fake client jar".to_vec();
    let lwjgl = b"lwjgl".to_vec();
    let asset_a = b"sound".to_vec();
    let asset_b = b"lang".to_vec();
    let index = json!({"objects": {
        "minecraft/sounds/a.ogg": {"hash": sha1(&asset_a), "size": asset_a.len()},
        "minecraft/lang/en_us.json": {"hash": sha1(&asset_b), "size": asset_b.len()}
    }});
    let index_bytes = serde_json::to_vec(&index).unwrap();
    let log_cfg = b"<Configuration/>".to_vec();
    let vanilla = json!({
        "id": "1.21.1", "type": "release", "mainClass": "net.minecraft.client.main.Main",
        "javaVersion": {"component": "java-runtime-delta", "majorVersion": 21},
        "assetIndex": {"id": "17", "url": format!("{base}/mc/17.json"), "sha1": sha1(&index_bytes)},
        "downloads": {"client": {"url": format!("{base}/mc/client.jar"), "sha1": sha1(&client_jar), "size": client_jar.len()}},
        "logging": {"client": {"argument": "-Dlog4j.configurationFile=${path}", "file": {"id": "client-1.12.xml", "url": format!("{base}/mc/log.xml"), "sha1": sha1(&log_cfg), "size": log_cfg.len()}}},
        "arguments": {
            "game": ["--username", "${auth_player_name}", "--version", "${version_name}", "--gameDir", "${game_directory}",
                     "--assetsDir", "${assets_root}", "--assetIndex", "${assets_index_name}", "--uuid", "${auth_uuid}",
                     "--accessToken", "${auth_access_token}", "--userType", "${user_type}"],
            "jvm": ["-Djava.library.path=${natives_directory}", "-cp", "${classpath}"]
        },
        "libraries": [
            {"name": "org.lwjgl:lwjgl:3.3.3", "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3.jar", "url": format!("{base}/libs/org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3.jar"), "sha1": sha1(&lwjgl), "size": lwjgl.len()}}},
            {"name": "org.ow2.asm:asm:9.3", "downloads": {"artifact": {"path": "org/ow2/asm/asm/9.3/asm-9.3.jar", "url": format!("{base}/libs/old-asm.jar"), "sha1": "0000000000000000000000000000000000000000", "size": 1}}},
            {"name": "only:other-os:1", "rules": [{"action": "allow", "os": {"name": "nonexistent-os"}}],
             "downloads": {"artifact": {"path": "x.jar", "url": format!("{base}/missing.jar")}}}
        ]
    });
    let vanilla_bytes = serde_json::to_vec(&vanilla).unwrap();
    put("/mc/manifest.json", serde_json::to_vec(&json!({"latest": {"release": "1.21.1", "snapshot": "1.21.1"},
        "versions": [{"id": "1.21.1", "type": "release", "url": format!("{base}/mc/1.21.1.json"), "sha1": sha1(&vanilla_bytes)}]})).unwrap());
    put("/mc/1.21.1.json", vanilla_bytes);
    put("/mc/client.jar", client_jar.clone());
    put("/mc/17.json", index_bytes);
    put("/mc/log.xml", log_cfg);
    put("/libs/org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3.jar", lwjgl);
    put(&format!("/res/{}/{}", &sha1(&asset_a)[..2], sha1(&asset_a)), asset_a);
    put(&format!("/res/{}/{}", &sha1(&asset_b)[..2], sha1(&asset_b)), asset_b);

    // ── Fabric profile (newer ASM must replace vanilla's) ──
    let loader_jar = b"fabric loader".to_vec();
    let asm = b"asm 9.7".to_vec();
    put("/fabric/v2/versions/loader/1.21.1", serde_json::to_vec(&json!([{"loader": {"version": "0.16.9", "stable": true}}])).unwrap());
    put("/fabric/v2/versions/loader/1.21.1/0.16.9/profile/json", serde_json::to_vec(&json!({
        "id": "fabric-loader-0.16.9-1.21.1", "inheritsFrom": "1.21.1",
        "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
        "arguments": {"game": [], "jvm": ["-DFabricMcEmu= net.minecraft.client.main.Main "]},
        "libraries": [
            {"name": "net.fabricmc:fabric-loader:0.16.9", "url": format!("{base}/fabricmaven/"), "sha1": sha1(&loader_jar)},
            {"name": "org.ow2.asm:asm:9.7", "url": format!("{base}/fabricmaven/"), "sha1": sha1(&asm)}
        ]
    })).unwrap());
    put("/fabricmaven/net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar", loader_jar);
    put("/fabricmaven/org/ow2/asm/asm/9.7/asm-9.7.jar", asm);

    // ── Java runtime ──
    let java_bin = b"#!/bin/sh\necho java".to_vec();
    let exe_rel = if cfg!(windows) { "bin/javaw.exe" } else if cfg!(target_os = "macos") { "jre.bundle/Contents/Home/bin/java" } else { "bin/java" };
    let rt_manifest = json!({"files": {
        "bin": {"type": "directory"},
        exe_rel: {"type": "file", "executable": true, "downloads": {"raw": {"url": format!("{base}/java/java"), "sha1": sha1(&java_bin), "size": java_bin.len()}}}
    }});
    let mut all = serde_json::Map::new();
    all.insert(minecraft::java::platform_key().into(), json!({"java-runtime-delta": [{"manifest": {"url": format!("{base}/java/delta.json"), "sha1": "x"}}]}));
    put("/java/all.json", serde_json::to_vec(&all).unwrap());
    put("/java/delta.json", serde_json::to_vec(&rt_manifest).unwrap());
    put("/java/java", java_bin);

    // ── Create + prepare a Fabric instance ──
    assert_eq!(minecraft::loaders::versions(Loader::Fabric, "1.21.1").await.unwrap()[0].version, "0.16.9");
    let mut inst = instance::create("Fabric Test", "1.21.1", Loader::Fabric, Some("0.16.9".into()), 3072).unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let ev = events.clone();
    let on: OnProgress = Arc::new(move |p| ev.lock().unwrap().push(p.message));
    let session = launch::Session { name: "Alex".into(), uuid: "u-1".into(), access_token: "TOKEN".into(), xuid: "x".into(), client_id: "cid".into() };
    let ready = launch::prepare(&mut inst, &session, None, &on).await.unwrap();

    assert!(ready.java.exists());
    let a = &ready.args;
    assert_eq!(a[0], "-Xmx3072M");
    let main = a.iter().position(|x| x == "net.fabricmc.loader.impl.launch.knot.KnotClient").expect("fabric main class");
    let cp = &a[a.iter().position(|x| x == "-cp").unwrap() + 1];
    assert!(cp.contains("fabric-loader-0.16.9.jar") && cp.contains("asm-9.7.jar") && !cp.contains("asm-9.3"), "{cp}");
    assert!(cp.contains("lwjgl-3.3.3.jar") && cp.ends_with("1.21.1.jar"), "client jar last: {cp}");
    assert!(!cp.contains("x.jar"), "library for another OS skipped");
    assert!(a.iter().any(|x| x.starts_with("-Dlog4j.configurationFile=") && x.ends_with("client-1.12.xml")));
    assert!(a[main..].windows(2).any(|w| w == ["--username", "Alex"]));
    assert!(a[main..].windows(2).any(|w| w == ["--assetIndex", "17"]));
    let objects = minecraft::assets_dir().join("objects");
    assert_eq!(std::fs::read_dir(&objects).unwrap().count(), 2);
    assert_eq!(instance::get(&inst.id).unwrap().version_id.as_deref(), Some("fabric-loader-0.16.9-1.21.1"));
    assert!(events.lock().unwrap().iter().any(|m| m.contains("Downloading game files")));

    // Second prepare: nothing to download, still works offline from cache.
    files.lock().unwrap().remove("/mc/manifest.json");
    launch::prepare(&mut inst, &session, None, &on).await.unwrap();

    // ── Modrinth pack import ──
    let sodium = jar(&[("fabric.mod.json", br#"{"id":"sodium","name":"Sodium","version":"0.6.0"}"#)]);
    let server_only = b"server".to_vec();
    put("/cdn/sodium.jar", sodium.clone());
    put("/cdn/server.jar", server_only.clone());
    let idx = json!({
        "formatVersion": 1, "game": "minecraft", "versionId": "1.0.0", "name": "Cozy Pack",
        "dependencies": {"minecraft": "1.21.1", "fabric-loader": "0.16.9"},
        "files": [
            {"path": "mods/sodium.jar", "hashes": {"sha1": sha1(&sodium), "sha512": sha512(&sodium)}, "downloads": [format!("{base}/cdn/sodium.jar")], "fileSize": sodium.len()},
            {"path": "mods/server-only.jar", "hashes": {"sha1": sha1(&server_only), "sha512": sha512(&server_only)}, "env": {"client": "unsupported", "server": "required"}, "downloads": [format!("{base}/cdn/server.jar")], "fileSize": 6},
            {"path": "../../escape.jar", "hashes": {"sha1": "a", "sha512": "b"}, "downloads": [format!("{base}/cdn/sodium.jar")], "fileSize": 1}
        ]
    });
    let pack = home.path().join("cozy.mrpack");
    std::fs::write(&pack, jar(&[
        ("modrinth.index.json", serde_json::to_string(&idx).unwrap().as_bytes()),
        ("overrides/config/sodium-options.json", b"{\"quality\":\"fancy\"}"),
        ("client-overrides/options.txt", b"fov:90"),
        ("overrides/../../evil.txt", b"no"),
    ])).unwrap();
    let r = import::any(&pack, "", &on).await.unwrap();
    let g = instance::game_dir(&r.instance.id);
    assert_eq!(r.instance.name, "Cozy Pack");
    assert_eq!(r.instance.loader, Loader::Fabric);
    assert!(g.join("mods/sodium.jar").exists());
    assert!(!g.join("mods/server-only.jar").exists(), "server-only files skipped");
    assert!(g.join("config/sodium-options.json").exists() && g.join("options.txt").exists());
    assert!(!home.path().join("escape.jar").exists() && !home.path().join("evil.txt").exists());
    assert!(r.warnings.iter().any(|w| w.contains("unsafe path")));
    assert!(r.warnings.iter().any(|w| w.contains("doesn't vet")), "local host isn't on Modrinth's list: {:?}", r.warnings);
    assert_eq!(instance::mods(&r.instance.id).unwrap()[0].meta.as_ref().unwrap().name, "Sodium");

    // A tampered download must fail the import.
    put("/cdn/sodium.jar", b"tampered".to_vec());
    let bad = home.path().join("bad.mrpack");
    let idx2 = json!({"formatVersion": 1, "game": "minecraft", "versionId": "1", "name": "Bad Pack",
        "dependencies": {"minecraft": "1.21.1"},
        "files": [{"path": "mods/sodium.jar", "hashes": {"sha1": sha1(&sodium), "sha512": sha512(&sodium)}, "downloads": [format!("{base}/cdn/sodium.jar")], "fileSize": sodium.len()}]});
    std::fs::write(&bad, jar(&[("modrinth.index.json", serde_json::to_string(&idx2).unwrap().as_bytes())])).unwrap();
    let err = import::any(&bad, "", &on).await.unwrap_err();
    assert!(format!("{err:#}").contains("checksum mismatch"), "{err:#}");
}
