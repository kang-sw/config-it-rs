//!
//!
//! # Usage
//!
//! ```ignore
//! let storage_1 = Storage::new();
//! let storage_2 = Storage::new();
//!
//! // Register the tracing layer (optional)
//! let (sink, source) = config_it_dashboard::tracing::layer();
//! tracing_subscriber::registry()
//!     .with(sink)
//!     // Do some other required stuff ...
//!     .init();
//!
//! // Start the dashboard server
//! config_it_dashboard::Builder::default()
//!     .http([127,0,0,1], 8080)
//! 	// If tracing source is present, the log tab will be enabled in dashboard
//!     .tracing_source(source)
//! 	// Authentication can be configured.
//!     .authentication("admin", "1234", Some("auth.txt"))
//! 	// HTTPS can be configured
//!     .https([0,0,0,0], 440, "cert.pem", "key.pem")
//!     // config_it_dashboard utilizes `dioxus-liveview` crate to render dashboard in server side.
//!     // Since dioxus is totally modularized, you can nest your own app into dashboard. User
//!     // widgets will be added to top-level navigation of dashboard.
//!     .with_user_widget_factory(MyHardwareMonitor::new())
//! 	// Multiple user widgets can be added.
//!     .with_user_widget_factory(MySystemMonitor::new())
//! 	// Every storage registration must be unique. (Unique name + Unique ID)
//!     .storage("first storage", storage_1.clone(), |x| x)
//!     .storage("second storage", storage_2.clone(), |x| {
//!         x.with_category_prefix("**").with_archive_path("config/second.json")
//!     })
//!     // Server will be disposed when shutdown signal is received. Dashboard clients can be
//!     // informed when this signal is received.
//!     .with_shutdown(async { std::future::pending::<()>().await })
//!     .serve()
//!     .await;
//! ```

// TODO: Storage monitor implementation

mod web {}

pub mod tracing {}
