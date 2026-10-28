//! Fleasion-style asset proxy.
//!
//! Roblox Player is started with `HTTPS_PROXY=http://127.0.0.1:<port>` (the
//! "env proxy" approach — no hosts-file edits, no admin rights, other apps are
//! unaffected). For the few Roblox API hosts in [`hooks::INTERCEPT_HOSTS`] the
//! proxy terminates TLS with a certificate from Arcadia's local CA (which the
//! bootstrapper adds to that client's `ssl/cacert.pem`), applies asset rules
//! and FastFlag overrides, and forwards upstream. Every other connection is a
//! blind tunnel.

pub mod ca;
pub mod hooks;
pub mod inspector;
pub mod rules;
mod server;

pub use server::{Proxy, ProxyConfig, TestUpstream};
