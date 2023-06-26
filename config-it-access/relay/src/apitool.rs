use axum::http::StatusCode;
use axum_extra::extract::CookieJar;
use tracing::debug;
use uuid::Uuid;

pub const COOKIE_SESSION_ID: &str = "Session-ID";

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

pub trait CookieRetrieveSessionId {
    fn retrieve_uuid(&self) -> Option<Uuid>;
}

impl CookieRetrieveSessionId for CookieJar {
    fn retrieve_uuid(&self) -> Option<Uuid> {
        self.get(COOKIE_SESSION_ID).and_then(|x| Uuid::parse_str(x.value()).ok())
    }
}
