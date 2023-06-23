use std::sync::Arc;

use compact_str::CompactString;
use dashmap::DashMap;
use parking_lot::Mutex;
use uuid::Uuid;

pub(crate) type AMutex<T> = tokio::sync::Mutex<T>;

pub struct Context {
    // TODO: Online providers management
    db: Mutex<rusqlite::Connection>,
    sessions: DashMap<Uuid, Arc<SessionCache>>,
    id_sess_table: DashMap<CompactString, Uuid>,
}

pub struct SessionCache {
    // TODO: 'Extender' channel to session expiration task
    // TODO: List of currently 'Accessible' sites.
    /// User ID of this session
    user_id: CompactString,
}

pub fn create_state() -> api::AppState {
    let db = rusqlite::Connection::open("db.sqlite").expect("Failed to open database");
    let state = Arc::new(Context {
        db: Mutex::new(db),
        sessions: DashMap::new(),
        id_sess_table: DashMap::new(),
    });

    state
}

pub mod api {
    use axum::{
        extract::State,
        http::Request,
        middleware::{self, Next},
        response::Response,
        Router,
    };
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
            .route("/api/login/:id", method::post(sess::login))
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
    }

    async fn mdl_authorization<B>(req: Request<B>, next: Next<B>) -> Response {
        next.run(req).await
    }

    pub mod sess {
        use super::AppStateExtract;
        use axum::{
            extract::{Path, State},
            headers::{authorization, Authorization},
            TypedHeader,
        };
        use axum_extra::extract::CookieJar;

        pub async fn login(
            State(this): AppStateExtract,
            TypedHeader(auth): TypedHeader<Authorization<authorization::Basic>>,
            jar: CookieJar,
            Path(id): Path<String>,
        ) {
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
