use std::sync::Arc;

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
use tracing::{debug, info};
use uuid::Uuid;

pub(crate) type AMutex<T> = tokio::sync::Mutex<T>;

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

pub struct Context {
    // TODO: Online providers management
    db_sys: sqlx::SqlitePool,

    sessions: DashMap<Uuid, SessionCache>,
    id_sess_map: DashMap<String, IndexSet<Uuid>>,
}

pub struct SessionCache {
    // TODO: 'Extender' channel to session expiration task
    // TODO: List of currently 'Accessible' sites.
    /// User ID of this session
    user_id: CompactString,
}

pub async fn create_state(first_user: Option<(&str, &str)>) -> api::AppState {
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

    debug!(num_user = n_user);

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

    Arc::new(Context { db_sys, id_sess_map: Default::default(), sessions: Default::default() })
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct AppConfig {}

pub mod apitool {
    use axum::http::StatusCode;
    use tracing::debug;

    pub trait ToStatusErr {
        fn map_status<V, E>(self, code: StatusCode) -> Result<V, StatusCode>
        where
            Self: Into<Result<V, E>>,
            E: std::fmt::Display,
        {
            self.into().map_err(|e| {
                debug!(%e);
                code
            })
        }
    }

    impl<V, E> ToStatusErr for Result<V, E> where E: std::fmt::Display {}
}

pub mod api {
    use axum::{
        extract::State,
        http::Request,
        middleware::{self, Next},
        response::Response,
        Router,
    };
    use axum_extra::extract::CookieJar;
    use capture_it::capture;
    use std::sync::Arc;

    pub type AppStateExtract = State<Arc<super::Context>>;
    pub type AppState = Arc<super::Context>;

    pub fn configure_api(state: AppState) -> Router<AppState> {
        use axum::routing as method;
        let gen_middleware_auth = capture!([state], move || {
            middleware::from_fn_with_state(state.clone(), mdl_authorization)
        });

        Router::new()
            .route("/api/login", method::post(sess::login))
            .nest(
                "/api/sess",
                Router::new()
                    .route("/logout", method::post(sess::logout))
                    .route("/extend", method::post(sess::extend))
                    .layer(gen_middleware_auth()),
            )
            .nest(
                "/api/mgmt",
                Router::new()
                    .route("/rule", method::get(mgmt::rule_list))
                    .route("/rule/:name", method::post(mgmt::rule_update))
                    .route("/rule/:name", method::get(mgmt::rule_get))
                    .route("/rule/:name", method::delete(mgmt::rule_delete))
                    .route("/role", method::get(mgmt::role_list))
                    .route("/role/:name", method::post(mgmt::role_update))
                    .route("/role/:name", method::get(mgmt::role_get))
                    .route("/role/:name", method::delete(mgmt::role_delete))
                    .route("/user", method::get(mgmt::user_list))
                    .route("/user/:name", method::post(mgmt::user_update))
                    .route("/user/:name", method::get(mgmt::user_get))
                    .route("/user/:name", method::delete(mgmt::user_delete))
                    .route("/prov_key", method::get(mgmt::prov_key_list))
                    .route("/prov_key/:name", method::post(mgmt::prov_key_add))
                    .route("/prov_key/:name", method::get(mgmt::prov_key_get))
                    .route("/prov_key/:name", method::delete(mgmt::prov_key_delete))
                    .layer(gen_middleware_auth()),
            )
            .nest(
                "/api/site",
                Router::new()
                    .route("/all", method::get(site::list))
                    .route("/info/:name", method::get(site::get_desc))
                    .route("/watch/:name", method::get(site::watch))
                    .route("/commit/:name", method::post(site::post_commit))
                    .route("/comment/:name", method::post(site::comment))
                    .route("/log/:name", method::get(site::fetch_old_log))
                    .layer(gen_middleware_auth()),
            )
            .route("/api/prov-login", method::post(|| async {})) // TODO: Authenticate with pre-registered 'APIKEY'
            .nest("/api/prov", Router::new()) // TODO: API for providers
    }

    async fn mdl_authorization<B>(jar: CookieJar, req: Request<B>, next: Next<B>) -> Response {
        next.run(req).await
    }

    async fn token_authorization<B>(jar: CookieJar, req: Request<B>, next: Next<B>) -> Response {
        next.run(req).await
    }

    pub mod sess {
        use std::{net::SocketAddr, time::SystemTime};

        use crate::{apitool::ToStatusErr, Authority};

        use super::AppStateExtract;
        use axum::{
            extract::{ConnectInfo, Path, State},
            headers::{authorization, Authorization},
            http::StatusCode,
            response::IntoResponse,
            Json, TypedHeader,
        };
        use axum_extra::extract::CookieJar;
        use compact_str::CompactString;
        use serde::Serialize;
        use sqlx::{query, query_as};
        use tracing::{debug, info};

        #[derive(Serialize, ts_rs::TS)]
        #[ts(export)]
        struct LoginReply {
            expire_utc_ms: u64,
            user_alias: String,
            authority: u32,
        }

        #[tracing::instrument(skip(this, auth, jar), fields(id = auth.username()))]
        pub async fn login(
            ConnectInfo(addr): ConnectInfo<SocketAddr>,
            State(this): AppStateExtract,
            TypedHeader(auth): TypedHeader<Authorization<authorization::Basic>>,
            jar: CookieJar,
        ) -> Result<impl IntoResponse, StatusCode> {
            debug!(%addr, id = auth.username(), "logging in ...");
            dbg!(auth.password());

            let (alias, authority): (String, u32) = query_as(concat!(
                "SELECT alias, authority FROM User",
                " WHERE id = ? AND passwd = ?"
            ))
            .bind(auth.username())
            .bind(auth.password())
            .fetch_optional(&this.db_sys)
            .await
            .expect("invalid query")
            .ok_or(StatusCode::UNAUTHORIZED)?;

            // TODO: Create new UUID and session instance.
            // TODO: Add to session <-> id mapping
            let authority = Authority::from_bits(authority).unwrap();
            info!(id = auth.username(), alias, ?authority, "login successful");

            let ts_now =
                SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis();

            // TODO: Parameterize expire time
            // TODO: Return session-id as cookie
            Ok(Json(LoginReply {
                expire_utc_ms: (ts_now + 30 * 60 * 1000) as _, // 2 hr per session
                user_alias: alias,
                authority: authority.bits(),
            }))
        }

        pub async fn logout(State(this): AppStateExtract) {
            tracing::info!("call me!?");
        }
        pub async fn extend(State(this): AppStateExtract) {}
    }

    pub mod mgmt {
        use super::AppStateExtract;
        use axum::extract::State;

        pub async fn rule_list(State(this): AppStateExtract) {}
        pub async fn rule_update(State(this): AppStateExtract) {}
        pub async fn rule_get(State(this): AppStateExtract) {}
        pub async fn rule_delete(State(this): AppStateExtract) {}

        pub async fn role_list(State(this): AppStateExtract) {}
        pub async fn role_update(State(this): AppStateExtract) {}
        pub async fn role_get(State(this): AppStateExtract) {}
        pub async fn role_delete(State(this): AppStateExtract) {}

        pub async fn user_list(State(this): AppStateExtract) {}
        pub async fn user_update(State(this): AppStateExtract) {}
        pub async fn user_get(State(this): AppStateExtract) {}
        pub async fn user_delete(State(this): AppStateExtract) {}

        pub async fn prov_key_list(State(this): AppStateExtract) {}
        pub async fn prov_key_add(State(this): AppStateExtract) {}
        pub async fn prov_key_get(State(this): AppStateExtract) {}
        pub async fn prov_key_delete(State(this): AppStateExtract) {}
    }

    pub mod site {
        use super::AppStateExtract;
        use axum::extract::State;

        pub async fn list(State(this): AppStateExtract) {}
        pub async fn get_desc(State(this): AppStateExtract) {}
        pub async fn watch(State(this): AppStateExtract) {}
        pub async fn post_commit(State(this): AppStateExtract) {}
        pub async fn comment(State(this): AppStateExtract) {}
        pub async fn fetch_old_log(State(this): AppStateExtract) {}
    }

    pub mod prov {
        use super::AppStateExtract;
        use axum::extract::State;

        pub async fn register(State(this): AppStateExtract) {}
        pub async fn publish(State(this): AppStateExtract) {}
    }
}
