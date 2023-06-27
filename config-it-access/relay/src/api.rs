use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Router,
};
use axum_extra::extract::CookieJar;
use capture_it::capture;
use std::sync::Arc;
use tracing::info;

use crate::apitool::CookieRetrieveSessionId;

pub type AppStateExtract = State<Arc<super::AppContext>>;
pub type AppState = Arc<super::AppContext>;

pub fn configure_api(state: AppState) -> Router<AppState> {
    use axum::routing as method;
    let gen_middleware_auth = capture!([state], move || {
        middleware::from_fn_with_state(state.clone(), authenticate_session)
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

async fn authenticate_session<B>(
    State(this): AppStateExtract,
    jar: CookieJar,
    req: Request<B>,
    next: Next<B>,
) -> Result<impl IntoResponse, StatusCode> {
    let uuid = jar.retrieve_uuid().ok_or(StatusCode::BAD_REQUEST)?;
    if this.user_sessions.contains_key(&uuid) == false {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(req).await)
}

async fn authenticate_provider<B>(req: Request<B>, next: Next<B>) -> Response {
    // TODO: retrive JWT, authenticate it.
    next.run(req).await
}

pub mod sess {
    use std::{
        net::SocketAddr,
        ops::Not,
        sync::Arc,
        time::{Duration, SystemTime},
    };

    use crate::{
        apitool::{CookieRetrieveSessionId, ToStatusErr, COOKIE_SESSION_ID},
        ARwLock, Authority, SessionCache,
    };

    use super::AppStateExtract;
    use anyhow::{anyhow, Context};
    use axum::{
        extract::{ConnectInfo, State},
        headers::{authorization, Authorization},
        http::StatusCode,
        response::IntoResponse,
        Json, TypedHeader,
    };
    use axum_extra::extract::{cookie::Cookie, CookieJar};

    use compact_str::ToCompactString;
    use indexmap::indexset;
    use serde::Serialize;
    use sqlx::query_as;
    use tokio::task::{self};
    use tracing::{debug, info, warn};
    use uuid::Uuid;

    #[derive(Serialize, ts_rs::TS)]
    #[ts(export)]
    struct LoginReply {
        expire_utc_ms: u64,
        user_alias: String,
        authority: u32,
    }

    #[tracing::instrument(skip(this, auth, jar), fields(id = auth.username(), %remote_addr))]
    pub async fn login(
        ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
        State(this): AppStateExtract,
        TypedHeader(auth): TypedHeader<Authorization<authorization::Basic>>,
        mut jar: CookieJar,
    ) -> Result<impl IntoResponse, StatusCode> {
        debug!("Logging in ...");

        let (alias, authority): (String, u32) =
            query_as(concat!("SELECT alias, authority FROM User", " WHERE id = ? AND passwd = ?"))
                .bind(auth.username())
                .bind(auth.password())
                .fetch_optional(&this.db_sys)
                .await
                .expect("invalid query")
                .ok_or(StatusCode::UNAUTHORIZED)?;

        let authority = Authority::from_bits(authority).unwrap();
        let new_session_uuid = {
            let uuid = Uuid::new_v4();
            let mut new_obj_anchor = None;
            let cache = this
                .user_info_table
                .entry(auth.username().into())
                .or_insert_with(|| {
                    debug!("Creating new user info");

                    let instance = Arc::new_cyclic(|weak_self| {
                        ARwLock::new(crate::SharedUserInfoCache {
                            weak_self: weak_self.clone(),
                            id: auth.username().into(),
                            authority,
                            alias: alias.to_compact_string(),
                            connections: indexset! {uuid},
                            weak_app: Arc::downgrade(&this),
                        })
                    });

                    let rval = Arc::downgrade(&instance);
                    new_obj_anchor = Some(instance);
                    rval
                })
                .upgrade()
                .ok_or_else(|| {
                    warn!("Edge case: entry removal occurred during lookup!");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            if new_obj_anchor.is_none() {
                debug!("Inserting user info to existing session ...");

                let n = {
                    let mut user = cache.write().await;
                    user.connections.insert(uuid);
                    user.connections.len()
                };

                debug!(num_connection = n, "User info inserted");
            }

            let expiration = this
                .clone()
                .create_session_timeout_task(uuid, None)
                .await
                .expect("failing this is logic error!");

            let new_sess = SessionCache {
                h_expire_task: expiration.into(),
                remote: remote_addr,
                user: cache.clone(),
            };

            assert!(this.user_sessions.insert(uuid, new_sess).is_none(), "UUID duplication!");
            uuid
        };

        if let Some(uuid_prev) = jar.retrieve_uuid() {
            debug!(%uuid_prev, "UUID found from cookie jar, removing ...");
            this.expire_session(uuid_prev).ok();
        }

        '_return_auth: {
            jar = jar.add(Cookie::new(COOKIE_SESSION_ID, new_session_uuid.to_string()));
            info!(
                n_total_sess = this.user_sessions.len(),
                id = auth.username(),
                alias,
                ?authority,
                "login successful"
            );

            Ok((
                jar,
                Json(LoginReply {
                    expire_utc_ms: this.session_expire_utc().as_millis() as _,
                    user_alias: alias,
                    authority: authority.bits(),
                }),
            ))
        }
    }

    pub async fn logout(State(this): AppStateExtract, jar: CookieJar) -> Result<(), StatusCode> {
        let uuid = jar.retrieve_uuid().ok_or(StatusCode::BAD_REQUEST)?;
        this.expire_session(uuid).map_status(StatusCode::NOT_MODIFIED)?;

        Ok(())
    }

    pub async fn extend(
        State(this): AppStateExtract,
        jar: CookieJar,
    ) -> Result<Json<u64>, StatusCode> {
        let uuid = jar.retrieve_uuid().ok_or(StatusCode::BAD_REQUEST)?;
        let task_previous = {
            let mut arg = this.user_sessions.get_mut(&uuid).ok_or(StatusCode::NOT_FOUND)?;
            arg.h_expire_task.take()
        };

        // NOTE: This unwrap prevents spawning multiple expiration task spawning
        let Some(task_previous) = task_previous else {
            warn!(%uuid, "it seems multiple concurrent extension is happening here ...");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        };

        let naive_new_expiration = this.session_expire_utc();
        let new_task = this
            .clone()
            .create_session_timeout_task(uuid, Some(task_previous))
            .await
            .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

        '_task_replacement: {
            // NOTE: Race condition occurs here ... has logged out during extension
            let mut arg = this.user_sessions.get_mut(&uuid).ok_or(StatusCode::CONFLICT)?;
            assert!(arg.h_expire_task.replace(new_task).is_none(), "logic error!");
        }

        Ok(Json(naive_new_expiration.as_millis() as _))
    }

    impl crate::AppContext {
        fn session_expire_utc(&self) -> Duration {
            let ts = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
            ts + Duration::from_secs(self.conf.session_time_seconds as _)
        }

        /// Returns [`None`] if the session is already expired.
        ///
        /// This is pretty edge case, but it can happen when task is failed to abort. In other
        /// words, when the task successfully expired the session.
        async fn create_session_timeout_task(
            self: Arc<Self>,
            session_id: Uuid,
            task: Option<task::JoinHandle<()>>,
        ) -> Option<task::JoinHandle<()>> {
            if let Some(task) = task {
                task.abort();

                let err = task.await.err()?;
                if err.is_cancelled() == false {
                    warn!(%err, "session expiration task panicked");
                    return None;
                }
            }

            Some(task::spawn(async move {
                tokio::time::sleep(Duration::from_secs(self.conf.session_time_seconds as _)).await;
                if let Err(e) = self.expire_session(session_id) {
                    warn!(%e, r#type = "timeout", "failed to expire session");
                }
            }))
        }

        fn expire_session(&self, session_id: Uuid) -> anyhow::Result<()> {
            let (_, cache) =
                self.user_sessions.remove(&session_id).context("session is already expired!")?;

            if let Some(task) = cache.h_expire_task {
                task.abort();
            }

            // NOTE: If `user_sessions` removal performed, following logic must be executed.

            task::spawn(async move {
                let mut user = cache.user.write().await;
                assert!(user.connections.remove(&session_id), "logic error");

                info!(
                    %user.id,
                    addr = %cache.remote,
                    session_left_for_this_id = user.connections.len(),
                    "session expired"
                );
            });

            Ok(())
        }
    }
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
