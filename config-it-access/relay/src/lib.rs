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
    use axum::extract::State;
    use std::sync::Arc;

    pub type StateExtr = State<Arc<super::Context>>;

    pub mod sess {
        use super::StateExtr;
        use axum::extract::State;

        pub async fn login(State(this): StateExtr) {}
        pub async fn logout(State(this): StateExtr) {}
        pub async fn extend(State(this): StateExtr) {}
    }

    pub mod mgmt {
        use super::StateExtr;
        use axum::extract::State;

        pub async fn rule_list(State(this): StateExtr) {}
        pub async fn rule_update(State(this): StateExtr) {}
        pub async fn rule_get(State(this): StateExtr) {}
        pub async fn rule_delete(State(this): StateExtr) {}

        pub async fn user_list(State(this): StateExtr) {}
        pub async fn user_update(State(this): StateExtr) {}
        pub async fn user_get(State(this): StateExtr) {}
        pub async fn user_delete(State(this): StateExtr) {}
    }

    pub mod site {
        use super::StateExtr;
        use axum::extract::State;

        pub async fn list(State(this): StateExtr) {}
        pub async fn get_desc(State(this): StateExtr) {}
        pub async fn watch(State(this): StateExtr) {}
        pub async fn post_commit(State(this): StateExtr) {}
        pub async fn comment(State(this): StateExtr) {}
    }

    pub mod prov {
        use super::StateExtr;
        use axum::extract::State;

        pub async fn register(State(this): StateExtr) {}
        pub async fn publish(State(this): StateExtr) {}
    }
}
