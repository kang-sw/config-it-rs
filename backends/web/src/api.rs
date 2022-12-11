use axum::{routing::get, Extension};
use tokio::sync::mpsc;

use super::actor;

pub(super) fn build(tx: mpsc::UnboundedSender<actor::Directive>) -> axum::Router {
    // TODO: Add router handler for index.html
    axum::Router::new()
        .route("/api/system_info", get(api::system_info))
        .layer(Extension(tx))
}

mod api {
    use crate::actor::{self};
    use axum::{http::StatusCode, Extension};

    type TxExtension = Extension<async_channel::Sender<actor::Directive>>;
    type Result<T> = std::result::Result<T, StatusCode>;

    pub(super) async fn system_info<'a>(Extension(tx): TxExtension) -> Result<String> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        let _ = tx.send(actor::Directive::GetSystemInfo(reply_tx));

        let sys_info = reply_rx
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(serde_json::to_string(&*sys_info).unwrap())
    }
}
