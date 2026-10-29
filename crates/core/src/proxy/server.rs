use super::ca::CertAuthority;
use super::hooks::{self, ASSET_HOST, FILE_PREFIX, INTERCEPT_HOSTS};
use super::inspector::{now_ms, preview, Entry, Inspector};
use super::rules::{Action, RuleConfig};
use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::header::{HeaderMap, HeaderValue};
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use parking_lot::RwLock;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::convert::Infallible;
use std::io::Read;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio_rustls::TlsAcceptor;

type Body = Full<Bytes>;

/// Send all upstream traffic to one local server (used by tests).
#[derive(Clone)]
pub struct TestUpstream {
    pub addr: SocketAddr,
    pub root_der: Vec<u8>,
}

pub struct ProxyConfig {
    /// Preferred port; if taken, a free loopback port is used instead.
    pub port: u16,
    pub data_dir: PathBuf,
    pub test_upstream: Option<TestUpstream>,
}

struct State {
    ca: CertAuthority,
    rules: RwLock<HashMap<u64, Action>>,
    fflags: RwLock<Map<String, Value>>,
    inspector: Inspector,
    upstream: reqwest::Client,
    test_upstream: Option<TestUpstream>,
}

pub struct Proxy {
    pub port: u16,
    state: Arc<State>,
    shutdown: Option<oneshot::Sender<()>>,
}

impl Proxy {
    pub async fn start(cfg: ProxyConfig) -> Result<Self> {
        let ca = CertAuthority::load_or_create(&cfg.data_dir).context("loading proxy CA")?;
        let mut b = reqwest::Client::builder()
            .no_proxy()
            .no_gzip()
            .redirect(reqwest::redirect::Policy::none())
            .pool_idle_timeout(std::time::Duration::from_secs(60));
        if let Some(t) = &cfg.test_upstream {
            b = b.add_root_certificate(reqwest::Certificate::from_der(&t.root_der)?);
            for h in INTERCEPT_HOSTS {
                b = b.resolve(h, t.addr);
            }
        }
        let state = Arc::new(State {
            ca,
            rules: RwLock::new(RuleConfig::load().compile()),
            fflags: RwLock::new(Map::new()),
            inspector: Inspector::default(),
            upstream: b.build()?,
            test_upstream: cfg.test_upstream,
        });

        let listener = match TcpListener::bind(("127.0.0.1", cfg.port)).await {
            Ok(l) => l,
            Err(_) => TcpListener::bind(("127.0.0.1", 0)).await?,
        };
        let port = listener.local_addr()?.port();
        let (tx, mut rx) = oneshot::channel::<()>();
        let st = state.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut rx => break,
                    acc = listener.accept() => {
                        let Ok((sock, _)) = acc else { continue };
                        let _ = sock.set_nodelay(true);
                        let st = st.clone();
                        tokio::spawn(async move {
                            let svc = service_fn(move |req| front(st.clone(), req));
                            let _ = hyper::server::conn::http1::Builder::new()
                                .serve_connection(TokioIo::new(sock), svc)
                                .with_upgrades()
                                .await;
                        });
                    }
                }
            }
        });
        log::info!("asset proxy listening on 127.0.0.1:{port}");
        Ok(Self { port, state, shutdown: Some(tx) })
    }

    pub fn ca_pem(&self) -> String {
        self.state.ca.pem().to_string()
    }
    pub fn set_rules(&self, cfg: &RuleConfig) {
        *self.state.rules.write() = cfg.compile();
    }
    pub fn set_fflags(&self, flags: Map<String, Value>) {
        *self.state.fflags.write() = flags;
    }
    pub fn inspector(&self) -> &Inspector {
        &self.state.inspector
    }
    pub fn stop(mut self) {
        if let Some(t) = self.shutdown.take() {
            let _ = t.send(());
        }
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        if let Some(t) = self.shutdown.take() {
            let _ = t.send(());
        }
    }
}

fn respond(status: StatusCode, body: impl Into<Bytes>) -> Response<Body> {
    let mut r = Response::new(Full::new(body.into()));
    *r.status_mut() = status;
    r
}

/// Requests arriving on the plain proxy port: CONNECT tunnels, or absolute
/// http:// URLs.
async fn front(st: Arc<State>, req: Request<Incoming>) -> Result<Response<Body>, Infallible> {
    if req.method() == Method::CONNECT {
        let Some(auth) = req.uri().authority().cloned() else {
            return Ok(respond(StatusCode::BAD_REQUEST, "CONNECT needs host:port"));
        };
        let host = auth.host().to_ascii_lowercase();
        let port = auth.port_u16().unwrap_or(443);
        let intercept = port == 443 && INTERCEPT_HOSTS.contains(&host.as_str());
        tokio::spawn(async move {
            let up = match hyper::upgrade::on(req).await {
                Ok(u) => u,
                Err(_) => return,
            };
            if intercept {
                if let Err(e) = mitm(st, host, TokioIo::new(up)).await {
                    log::debug!("mitm: {e:#}");
                }
            } else {
                tunnel(st, host, port, TokioIo::new(up)).await;
            }
        });
        return Ok(Response::new(Full::new(Bytes::new())));
    }
    // Plain-HTTP proxying: forward as-is.
    let url = req.uri().to_string();
    let host = req.uri().host().unwrap_or("").to_string();
    Ok(handle(st, req, host, url, false).await)
}

async fn tunnel(st: Arc<State>, host: String, port: u16, mut client: TokioIo<hyper::upgrade::Upgraded>) {
    let t0 = Instant::now();
    let target = match &st.test_upstream {
        Some(t) => TcpStream::connect(t.addr).await,
        None => TcpStream::connect((host.as_str(), port)).await,
    };
    let (status, kind) = match target {
        Ok(mut server) => {
            let _ = tokio::io::copy_bidirectional(&mut client, &mut server).await;
            (200, "tunnel")
        }
        Err(_) => (502, "error"),
    };
    st.inspector.record(Entry {
        id: 0,
        time_ms: now_ms(),
        method: "CONNECT".into(),
        url: format!("{host}:{port}"),
        status,
        req_bytes: 0,
        resp_bytes: 0,
        duration_ms: t0.elapsed().as_millis() as u64,
        kind: kind.into(),
        note: "not decrypted".into(),
        req_body: None,
        resp_body: None,
    });
}

async fn mitm(st: Arc<State>, host: String, io: TokioIo<hyper::upgrade::Upgraded>) -> Result<()> {
    let tls = TlsAcceptor::from(st.ca.server_config(&host)?).accept(io).await?;
    let svc = service_fn(move |req: Request<Incoming>| {
        let st = st.clone();
        let host = host.clone();
        async move {
            let pq = req.uri().path_and_query().map(|p| p.as_str()).unwrap_or("/").to_string();
            let url = format!("https://{host}{pq}");
            Ok::<_, Infallible>(handle(st, req, host, url, true).await)
        }
    });
    hyper::server::conn::http1::Builder::new()
        .serve_connection(TokioIo::new(tls), svc)
        .await?;
    Ok(())
}

const HOP: &[&str] = &[
    "connection", "proxy-connection", "keep-alive", "transfer-encoding", "te", "trailer", "upgrade",
    "host", "content-length", "proxy-authorization",
];

fn gunzip_if_needed(headers: &mut HeaderMap, body: Bytes) -> Bytes {
    let gz = headers
        .get(hyper::header::CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("gzip"));
    if !gz {
        return body;
    }
    let mut out = Vec::new();
    if flate2::read::GzDecoder::new(&body[..]).read_to_end(&mut out).is_ok() {
        headers.remove(hyper::header::CONTENT_ENCODING);
        Bytes::from(out)
    } else {
        body
    }
}

struct Upstream {
    status: StatusCode,
    headers: HeaderMap,
    body: Bytes,
}

async fn forward(st: &State, method: &Method, url: &str, headers: &HeaderMap, body: Bytes) -> Result<Upstream> {
    let mut url = url.to_string();
    if let Some(t) = &st.test_upstream {
        // reqwest's resolve() keeps the URL's port, so point it at the test server.
        let mut u = url::Url::parse(&url)?;
        let _ = u.set_port(Some(t.addr.port()));
        url = u.to_string();
    }
    let mut rb = st.upstream.request(method.clone(), &url);
    for (k, v) in headers {
        if !HOP.contains(&k.as_str()) {
            rb = rb.header(k, v);
        }
    }
    let r = rb.body(body).send().await?;
    let status = r.status();
    let mut headers = r.headers().clone();
    let body = r.bytes().await?;
    for h in HOP {
        headers.remove(*h);
    }
    Ok(Upstream { status, headers, body })
}

fn build(status: StatusCode, headers: HeaderMap, body: Bytes) -> Response<Body> {
    let mut r = Response::new(Full::new(body));
    *r.status_mut() = status;
    *r.headers_mut() = headers;
    r
}

/// One request, inside a decrypted connection (or plain HTTP).
async fn handle(st: Arc<State>, req: Request<Incoming>, host: String, url: String, decrypted: bool) -> Response<Body> {
    let t0 = Instant::now();
    let method = req.method().clone();
    let pq = req.uri().path_and_query().map(|p| p.as_str().to_string()).unwrap_or_else(|| "/".into());
    let mut headers = req.headers().clone();
    let body = match req.into_body().collect().await {
        Ok(b) => b.to_bytes(),
        Err(_) => return respond(StatusCode::BAD_REQUEST, ""),
    };
    let req_bytes = body.len();
    let mut kind = "passthrough";
    let mut note = String::new();
    let mut req_preview = None;

    let result: Result<Response<Body>> = async {
        // 1. Local replacement files.
        if decrypted && host == ASSET_HOST && pq.starts_with(FILE_PREFIX) {
            let id: u64 = pq[FILE_PREFIX.len()..].split(['?', '/']).next().unwrap_or("").parse().unwrap_or(0);
            let path = match st.rules.read().get(&id) {
                Some(Action::File { path }) => Some(path.clone()),
                _ => None,
            };
            let Some(path) = path else { return Ok(respond(StatusCode::NOT_FOUND, "no such replacement")) };
            let data = tokio::fs::read(&path).await.with_context(|| format!("reading {path}"))?;
            st.inspector.bump(|s| s.served_files += 1);
            kind = "served-file";
            note = format!("asset {id} ← {path}");
            let mut h = HeaderMap::new();
            h.insert(hyper::header::CONTENT_TYPE, HeaderValue::from_static("application/octet-stream"));
            return Ok(build(StatusCode::OK, h, Bytes::from(data)));
        }

        // 2. Asset batch requests.
        if decrypted && host == ASSET_HOST && method == Method::POST && pq.to_ascii_lowercase().starts_with("/v1/assets/batch") {
            let raw = gunzip_if_needed(&mut headers, body.clone());
            req_preview = preview(&raw);
            let rewrite = { hooks::rewrite_batch_request(&raw, &st.rules.read()) };
            let Some(rw) = rewrite else {
                let up = forward(&st, &method, &url, &headers, body.clone()).await?;
                return Ok(build(up.status, up.headers, up.body));
            };
            headers.insert(hyper::header::ACCEPT_ENCODING, HeaderValue::from_static("identity"));
            let mut up = forward(&st, &method, &url, &headers, Bytes::from(rw.body)).await?;
            let mut out = gunzip_if_needed(&mut up.headers, up.body);
            if let Some(b) = hooks::rewrite_batch_response(&out, &rw.location_overrides) {
                out = Bytes::from(b);
            }
            st.inspector.bump(|s| {
                s.replaced += rw.replaced as u64;
                s.removed += rw.removed as u64;
            });
            kind = "modified";
            note = [("replaced", rw.replaced), ("removed", rw.removed), ("file", rw.files), ("link", rw.links)]
                .iter()
                .filter(|(_, n)| *n > 0)
                .map(|(w, n)| format!("{w} {n}"))
                .collect::<Vec<_>>()
                .join(", ");
            return Ok(build(up.status, up.headers, out));
        }

        // 3. Single-asset GETs.
        if decrypted && host == ASSET_HOST && method == Method::GET {
            if let Some(id) = hooks::single_asset_id(&pq) {
                let action = st.rules.read().get(&id).cloned();
                match action {
                    Some(Action::Remove) => {
                        st.inspector.bump(|s| s.removed += 1);
                        kind = "blocked";
                        note = format!("removed {id}");
                        return Ok(respond(StatusCode::NOT_FOUND, ""));
                    }
                    Some(Action::Replace { with_id }) => {
                        let new = format!("https://{host}{}", hooks::swap_single_asset_id(&pq, id, with_id));
                        let up = forward(&st, &method, &new, &headers, body.clone()).await?;
                        st.inspector.bump(|s| s.replaced += 1);
                        kind = "modified";
                        note = format!("{id} → {with_id}");
                        return Ok(build(up.status, up.headers, up.body));
                    }
                    Some(Action::File { path }) => {
                        let data = tokio::fs::read(&path).await.with_context(|| format!("reading {path}"))?;
                        st.inspector.bump(|s| s.served_files += 1);
                        kind = "served-file";
                        note = format!("asset {id} ← {path}");
                        return Ok(build(StatusCode::OK, HeaderMap::new(), Bytes::from(data)));
                    }
                    Some(Action::Link { url: to }) => {
                        // assetdelivery normally answers these with a redirect
                        // to the CDN, so the client already follows one.
                        st.inspector.bump(|s| s.replaced += 1);
                        kind = "modified";
                        note = format!("{id} → {to}");
                        let mut h = HeaderMap::new();
                        h.insert(hyper::header::LOCATION, HeaderValue::from_str(&to).context("link isn't a valid URL")?);
                        return Ok(build(StatusCode::FOUND, h, Bytes::new()));
                    }
                    None => {}
                }
            }
        }

        // 4. Live FastFlags.
        if decrypted && hooks::is_client_settings(&host, &pq) && !st.fflags.read().is_empty() {
            headers.insert(hyper::header::ACCEPT_ENCODING, HeaderValue::from_static("identity"));
            let mut up = forward(&st, &method, &url, &headers, body.clone()).await?;
            let raw = gunzip_if_needed(&mut up.headers, up.body.clone());
            let injected = { hooks::inject_fflags(&raw, &st.fflags.read()) };
            if let Some((b, n)) = injected {
                st.inspector.bump(|s| s.fflag_injections += n as u64);
                kind = "modified";
                note = format!("fastflags +{n}");
                return Ok(build(up.status, up.headers, Bytes::from(b)));
            }
            return Ok(build(up.status, up.headers, up.body));
        }

        // 5. Everything else on an intercepted host: untouched.
        let up = forward(&st, &method, &url, &headers, body.clone()).await?;
        Ok(build(up.status, up.headers, up.body))
    }
    .await;

    let resp = match result {
        Ok(r) => r,
        Err(e) => {
            kind = "error";
            note = format!("{e:#}");
            respond(StatusCode::BAD_GATEWAY, format!("Arcadia proxy: {e:#}"))
        }
    };
    let resp_bytes = hyper::body::Body::size_hint(resp.body()).exact().unwrap_or(0) as usize;
    let resp_preview = if kind == "modified" || kind == "error" {
        // Re-read the body we're about to send for the inspector.
        let b = resp.body().clone();
        b.collect().await.ok().and_then(|c| preview(&c.to_bytes()))
    } else {
        None
    };
    if req_preview.is_none() && kind != "passthrough" {
        req_preview = preview(&body);
    }
    st.inspector.record(Entry {
        id: 0,
        time_ms: now_ms(),
        method: method.to_string(),
        url,
        status: resp.status().as_u16(),
        req_bytes,
        resp_bytes,
        duration_ms: t0.elapsed().as_millis() as u64,
        kind: kind.into(),
        note,
        req_body: req_preview,
        resp_body: resp_preview,
    });
    resp
}
