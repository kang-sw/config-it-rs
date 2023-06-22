pub struct Context {}

pub mod auth {
    #[derive(serde::Serialize, serde::Deserialize)]
    pub enum UserAccessLevel {
        User,
        Manager,
        Admin,
    }
}

pub mod api {
    use axum::{
        extract::State,
        http::Request,
        middleware::{self, Next},
        response::Response,
    };
    use std::sync::Arc;

    pub type AppStateExtract = State<Arc<super::Context>>;
    pub type AppState = Arc<super::Context>;

    pub fn configure_api(state: AppState) -> axum::Router<AppState> {
        use axum::routing as method;

        axum::Router::new()
            .route("/sess/login", method::post(sess::login))
            .route("/sess/logout", method::post(sess::logout))
            .route("/sess/extend", method::post(sess::extend))
            .nest(
                "/mgmt",
                axum::Router::new()
                    .route("/rule", method::get(mgmt::rule_list))
                    .route("/rule", method::post(mgmt::rule_update))
                    .route("/rule/:name", method::get(mgmt::rule_get))
                    .route("/rule/:name", method::delete(mgmt::rule_delete))
                    .route("/user", method::get(mgmt::user_list))
                    .route("/user", method::post(mgmt::user_update))
                    .route("/user/:name", method::get(mgmt::user_get))
                    .route("/user/:name", method::delete(mgmt::user_delete))
                    .layer(middleware::from_fn_with_state(state.clone(), mdl_authorization)),
            )
            .route("/sites", method::get(site::list))
            .route("/site/info/:name", method::get(site::get_desc))
            .route("/site/watch/:name", method::get(site::watch))
            .route("/site/commit/:name", method::post(site::post_commit))
            .route("/site/comment/:name", method::post(site::comment))
            .route("/site/log/:name", method::get(site::fetch_old_log))
    }

    async fn mdl_authorization<B>(req: Request<B>, next: Next<B>) -> Response {
        next.run(req).await
    }

    pub mod sess {
        use super::AppStateExtract;
        use axum::extract::State;

        pub async fn login(State(this): AppStateExtract) {}
        pub async fn logout(State(this): AppStateExtract) {}
        pub async fn extend(State(this): AppStateExtract) {}
    }

    pub mod mgmt {
        use super::AppStateExtract;
        use axum::extract::State;

        pub async fn rule_list(State(this): AppStateExtract) {}
        pub async fn rule_update(State(this): AppStateExtract) {}
        pub async fn rule_get(State(this): AppStateExtract) {}
        pub async fn rule_delete(State(this): AppStateExtract) {}

        pub async fn user_list(State(this): AppStateExtract) {}
        pub async fn user_update(State(this): AppStateExtract) {}
        pub async fn user_get(State(this): AppStateExtract) {}
        pub async fn user_delete(State(this): AppStateExtract) {}
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
