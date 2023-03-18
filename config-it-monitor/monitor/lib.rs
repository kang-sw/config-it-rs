//!
//!
//! ## TODOs
//!
//! - [ ]
//!

pub mod server {
    use config_it::CompactString;
    use futures::{future::BoxFuture, FutureExt};
    use sha2::Digest;
    use std::collections::HashMap;

    pub type Hash512 = [u8; 64];

    /* ----------------------------------- Storage Descriptor ----------------------------------- */
    /// Table of storage decsriptions. The key is registered name of given storage.
    #[derive(Debug, Default)]
    pub struct StorageTable(HashMap<CompactString, StorageDesc>);

    #[derive(custom_debug::Debug)]
    struct StorageDesc {
        /// Handle to storage
        #[debug(skip)]
        handle: config_it::Storage,

        /// Access keys required to read/write to this storage. The value represents whether
        /// write access is granted or not for given key.
        #[debug(skip)]
        access_keys: HashMap<Hash512, bool>,
    }

    pub struct StorageDescBuilderProxy {
        owner: StorageTable,
        key: CompactString,
        desc: StorageDesc,
    }

    impl StorageTable {
        pub fn entry(
            self,
            name: impl Into<CompactString>,
            storage: config_it::Storage,
        ) -> StorageDescBuilderProxy {
            StorageDescBuilderProxy {
                owner: self,
                key: name.into(),
                desc: StorageDesc {
                    handle: storage,
                    access_keys: Default::default(),
                },
            }
        }
    }

    impl StorageDescBuilderProxy {
        pub fn add_access_key_raw(
            mut self,
            key: Hash512,
            grant_write: bool,
        ) -> StorageDescBuilderProxy {
            self.desc.access_keys.insert(key, grant_write);
            self
        }

        pub fn add_access_key<'a>(
            self,
            passphrase: impl AsRef<[u8]> + 'a,
            grant_write: bool,
        ) -> StorageDescBuilderProxy {
            let mut sha = sha2::Sha512::new();
            sha.update(passphrase);
            let key = sha.finalize();
            self.add_access_key_raw(key.into(), grant_write)
        }

        pub fn submit(mut self) -> StorageTable {
            self.owner.0.insert(self.key, self.desc);
            self.owner
        }
    }

    /* ------------------------------- Server Initialization Info ------------------------------- */
    #[derive(typed_builder::TypedBuilder)]
    pub struct Service {
        /// Storage contents
        table: StorageTable,

        /// The port to bind to. An axum router will be created and bound to this port.
        bind_port: u16,

        /// The IP address to bind on. If not specified, it will defaults to IPv4 ANY.
        #[builder(default, setter(transform=|s:&str|Some(s.into())))]
        bind_ip: Option<CompactString>,

        /// If this future is provided, the server will be closed when this future resolves.
        #[builder(default, setter(transform=|f:impl std::future::Future<Output=()> + Send + 'static| Some(f.boxed())))]
        close_signal: Option<BoxFuture<'static, ()>>,
        // TODO: authentication modeling?
    }

    pub(crate) struct Context {
        table: StorageTable,
    }

    mod driver {
        use std::{net::SocketAddr, sync::Arc};

        use axum::{
            extract::{ConnectInfo, State},
            response::IntoResponse,
            routing::get,
        };

        use super::{Context, Service};

        #[derive(Debug, thiserror::Error)]
        pub enum Error {
            #[error("failed to parse address")]
            AddrParseError(#[from] std::net::AddrParseError),

            #[error("error during server operation")]
            ServiceError(#[from] axum::Error),
        }

        impl Service {
            pub async fn serve(self) -> Result<(), Error> {
                // - create axum router -> routes for /index.html and /api/ ...
                let context = Arc::new(Context { table: self.table });
                let route = axum::Router::default()
                    .route("/", get(|| async { "Hello, world!" }))
                    .route("/ws", get(Self::__on_ws))
                    .with_state(context);

                axum::Server::bind(
                    &format!(
                        "{}:{}",
                        self.bind_ip.unwrap_or_else(|| "0.0.0.0".into()),
                        self.bind_port
                    )
                    .parse()?,
                )
                .serve(route.into_make_service_with_connect_info::<SocketAddr>())
                .with_graceful_shutdown(async {
                    if let Some(fut) = self.close_signal {
                        fut.await;
                    } else {
                        std::future::pending::<()>().await;
                    }
                })
                .await
                .map_err(|e| axum::Error::new(e))?;

                Ok(())
            }

            async fn __on_ws(
                State(ctx): State<Arc<Context>>,
                ws: axum::extract::WebSocketUpgrade,
                ConnectInfo(addr): ConnectInfo<SocketAddr>,
            ) -> impl IntoResponse {
                "todo!()"
            }
        }
    }
}

mod session {
    use std::{net::SocketAddr, sync::Arc};

    #[derive(typed_builder::TypedBuilder)]
    pub(crate) struct SessionInfo {
        context: Arc<crate::server::Context>,
        remote: SocketAddr,
        rpc: rpc_it::Handle,
    }
}

pub mod trace {
    use tracing::span;

    pub struct Subscriber {}

    impl tracing::Subscriber for Subscriber {
        fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
            todo!()
        }

        fn new_span(&self, span: &span::Attributes<'_>) -> span::Id {
            todo!()
        }

        fn record(&self, span: &span::Id, values: &span::Record<'_>) {
            todo!()
        }

        fn record_follows_from(&self, span: &span::Id, follows: &span::Id) {
            todo!()
        }

        fn event(&self, event: &tracing::Event<'_>) {
            todo!()
        }

        fn enter(&self, span: &span::Id) {
            todo!()
        }

        fn exit(&self, span: &span::Id) {
            todo!()
        }
    }
}
