//!
//! # Config-it web control backend
//!
//! This library provides a class which serves a web accessible control dashboard for existing
//! config-it storage instance.
//!
//!

// NOTE: Intentionally declare all modules as private, and exposes desired number of public APIs.
mod misc;
mod runner;
mod service;
pub mod trace;

pub use service::Service;
