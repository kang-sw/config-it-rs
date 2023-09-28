//! # Config-it Beacon
//!
//! **Beacon** exposes application's management point to **Harbor**.
//!
//! ```ignore
//! // Assume you're in async context
//!
//! let beacon = config_it_beacon::Builder::with_remote("https://harbor.example.com")
//!     .with_app_path("fruit-farm/grape/vision-monitor")
//!     .with_fixed_app_key(std::env::var("APP_KEY").unwrap()) // random gen on every instance
//!     .with_app_version(env!("CARGO_PKG_VERSION"))
//!     .with_latitude_longitude(37.564, 127.001)
//!     .with_tags(["prod", "asia"])
//!     .launch()
//!     .await
//!     .unwrap();
//!
//! let storage = config_it::Storage::new();
//! beacon
//!     .add_storage("Primary", storage.clone())
//!     .with_admin_only(true);
//!
//! // ...
//!
//! // Can be updated at any time.
//! beacon.update_latitude_longitude(34.564, 123.001);
//! beacon.update_tags(["prod", "asia", "korea"]);
//! beacon.add_tags(["korea"]);
//! beacon.remove_tags(["asia"]);
//!
//!
//! ```
