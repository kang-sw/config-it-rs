//! Provides generic UI support
//!
//! - Storage subscription & groups·items management
//! - JsonSchema retrieval from every items
//! - Edition history support.

mod subscriber;

#[cfg(feature = "egui")]
mod egui {}

#[cfg(any())]
mod dioxus {}
