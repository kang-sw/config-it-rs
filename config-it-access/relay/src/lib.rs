use std::{
    net::SocketAddr,
    sync::{Arc, Weak},
};

use anyhow::Context as _Context;
use bitflags::bitflags;
use compact_str::CompactString;
use dashmap::DashMap;
use indexmap::IndexSet;
use serde::{Deserialize, Serialize};
use sqlx::{
    query,
    sqlite::{self},
    Executor, Row,
};
use tokio::task;
use tracing::{debug, info, warn};
use uuid::Uuid;

pub(crate) type AMutex<T> = tokio::sync::Mutex<T>;
pub(crate) type ARwLock<T> = tokio::sync::RwLock<T>;

bitflags! {
    /// TODO: Find way to programatically share this list with typescript ...
    /// Otherwise, type all manually ?
    #[derive(Clone, Copy, Debug, Default)]
    #[repr(transparent)]
    pub struct Authority: u32 {
        /// Can access administrative actions
        /// - Assign user 'administrative' roles / authorities
        /// - Create 'Admin' access rules
        const ADMIN = 0x01;

        /// Can add/remove/update user.
        const EDIT_USER_LIST = 0x02;

        /// Can edit user authority, except for administrative roles ...
        const ASSIGN_USER_AUTH = 0x04;

        /// Can assign user to a role, for non-administrative ...
        const ASSIGN_USER_ROLE = 0x08;

        /// Can duplicate current user's role/authority
        const DUPLICATE_SELF = 0x10;

        /// Can set notification hooks
        const SET_SITE_HOOK = 0x20;

        /// Can access site's log
        const ACCESS_SITE_LOG = 0x40;

        /// Can access site's configuration history
        const ACCESS_SITE_HISTORY = 0x80;

        /// Can modify site's configuration
        const MODIFY_SITE_CONFIG = 0x100;
    }
}

impl Authority {
    pub fn administrative() -> Self {
        Self::default()
    }
}

pub struct AppContext {
    conf: AppConfig,

    // TODO: Online providers management
    db_sys: sqlx::SqlitePool,

    /// All logged in users
    user_sessions: DashMap<Uuid, SessionContext>,

    /// User instances
    user_info_table: DashMap<CompactString, Weak<UserContextLock>>,
}

struct SessionContext {
    // TODO: 'Extender' channel to session expiration task
    // TODO: List of currently 'Accessible' sites.
    /// User information.
    user: Arc<UserContextLock>,

    /// A handle to expire this session.
    h_expire_task: Option<task::JoinHandle<()>>,

    /// Remote address of this session
    remote: SocketAddr,
}

/// Shared user informatino between logon sessions
struct UserContext {
    /// Weak instance to self
    weak_self: Weak<ARwLock<Self>>,

    /// User ID of this session
    id: CompactString,

    /// Password hash
    passwd: String,

    /// Authority level
    authority: Authority,

    /// Alias
    alias: CompactString,

    /// Live set of logon sessions
    connections: IndexSet<Uuid>,

    /// Weak reference to app context. This is used on struct drop
    weak_app: Weak<AppContext>,
}

/// User information, which is locked
type UserContextLock = ARwLock<UserContext>;

impl Drop for UserContext {
    fn drop(&mut self) {
        if let Some(app) = self.weak_app.upgrade() {
            app.user_info_table.remove(&self.id);
        }
    }
}

pub async fn create_state(first_user: Option<(&str, &str)>) -> api::AppState {
    // Read configuration
    let conf = async {
        let file = tokio::fs::read("config.toml").await.context("config file not exist")?;
        let file_str = std::str::from_utf8(&file).context("config file is not valid utf-8")?;
        Ok::<_, anyhow::Error>(toml::from_str(file_str).context("failed to parse config file")?)
    }
    .await;

    let conf = match conf {
        Ok(x) => x,
        Err(e) => {
            warn!(%e, "reading config failed");
            let conf = AppConfig::default();
            let conf_str = toml::to_string_pretty(&conf).unwrap();
            tokio::fs::write("config.toml", conf_str).await.unwrap();

            info!("Default config file created, please edit it and restart the server.");
            std::process::exit(1);
        }
    };

    // prepare directory to store site-specific files
    std::fs::create_dir("sites").ok();

    let db_sys = sqlite::SqlitePool::connect_with(
        sqlite::SqliteConnectOptions::new().filename("db-sys.sqlite").create_if_missing(true),
    )
    .await
    .unwrap();

    db_sys.execute(include_str!("./ddl/Sys.ddl")).await.unwrap();
    let qry = "SELECT COUNT(*) FROM User";
    let n_user = query(qry).fetch_one(&db_sys).await.unwrap().get::<i64, _>(0);

    debug!(num_registered_users = n_user);

    if let Some((id, pw)) = first_user.filter(|_| n_user == 0) {
        info!(
            id,
            password = (0..pw.len()).map(|_| '*').collect::<String>(),
            "No user found, creating first user..."
        );

        let pw = sha256::digest(pw);
        db_sys
            .execute(
                query("INSERT INTO User(id, passwd, alias, authority) VALUES(?, ?, ?, ?)")
                    .bind(id)
                    .bind(pw)
                    .bind("Administrator")
                    .bind(Authority::all().bits()),
            )
            .await
            .expect("First user creation failed");
    }

    Arc::new(AppContext {
        db_sys,
        conf,
        user_sessions: Default::default(),
        user_info_table: Default::default(),
    })
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
struct AppConfig {
    /// Session expiration time in seconds
    session_time_seconds: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self { session_time_seconds: 2400 }
    }
}

pub mod api;
pub mod apitool;
