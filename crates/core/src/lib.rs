//! Arcadia core.
//!
//! * [`library`]  – detects installed games across launchers and launches them.
//! * [`roblox`]   – a Bloxstrap/Froststrap-style Roblox bootstrapper.
//! * [`addons`]   – sandboxed storage for marketplace addons, and their backends.
//! * [`proxy`]    – a Fleasion-style loopback HTTPS proxy that swaps Roblox assets
//!                  and overrides FastFlags before they reach the client.
//!
//! The crate has no GUI dependencies so every piece can be tested headless.

pub mod addons;
pub mod library;
pub mod minecraft;
pub mod paths;
pub mod proxy;
pub mod registry;
pub mod roblox;
pub mod settings;

pub use settings::Settings;
