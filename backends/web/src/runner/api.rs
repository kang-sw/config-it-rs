use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use axum::{Extension, Router};
use futures::Future;

use super::Handle;

pub(super) struct ApiRouter {
    sys_info: super::SystemInfo,
    chan: Handle,
}

impl ApiRouter {
    pub async fn run(
        sys_info: super::SystemInfo,
        port: u16,
        chan: super::Handle,
        shutdown: impl Future<Output = ()>,
    ) {
        let this = Arc::new(Self { sys_info, chan });
        let router = Router::new().layer(Extension(this.clone()));

        axum::Server::bind(&SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port))
            .serve(router.into_make_service())
            .with_graceful_shutdown(shutdown)
            .await;
    }
}
