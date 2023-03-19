use config_it::CompactString;
use futures::{future::BoxFuture, FutureExt};
use sha2::Digest;
use std::collections::HashMap;

use crate::common::handshake::SystemIntroduce;

pub type Hash512 = [u8; 64];

/* ---------------------------------------- Storage Desc ---------------------------------------- */
/// Table of storage decsriptions. The key is registered name of given storage.
#[derive(Debug, Default)]
pub struct StorageTable(pub(crate) HashMap<CompactString, StorageDesc>);

#[derive(custom_debug::Debug)]
pub(crate) struct StorageDesc {
    /// Handle to storage
    #[debug(skip)]
    pub(crate) handle: config_it::Storage,

    /// Access keys required to read/write to this storage. The value represents whether
    /// write access is granted or not for given key.
    #[debug(skip)]
    pub(crate) access_keys: HashMap<Hash512, bool>,
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

/* --------------------------------- Server Initialization Info --------------------------------- */
#[derive(typed_builder::TypedBuilder)]
pub struct Service {
    /// Storage contents
    table: StorageTable,

    /// Name of this system.
    #[builder(default = "<unknown>".into(), setter(transform=|s:impl Into<String>| s.into()))]
    system_name: String,

    #[builder(default, setter(transform=|s:impl Into<String>| s.into()))]
    system_desc: String,

    /// Computer name should be hidden?
    #[builder(default = false)]
    hide_host_name: bool,

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
    pub table: StorageTable,
    pub sys_info: SystemIntroduce,
    // TODO: Tracing subscriber replication
}

mod driver {
    use std::{net::SocketAddr, sync::Arc, time::SystemTime};

    use crate::common::handshake::SystemIntroduce;

    use super::{Context, Service};
    use axum::{
        extract::{ConnectInfo, State},
        response::IntoResponse,
        routing::get,
    };

    #[derive(Debug, thiserror::Error)]
    pub enum Error {
        #[error("failed to parse address")]
        AddrParseError(#[from] std::net::AddrParseError),

        #[error("error during server operation")]
        ServiceError(#[from] axum::Error),
    }

    impl Service {
        /// Start the service. This function will (asynchronosuly) block until the service
        /// is closed. May return error if any of provided arguments is invalid.
        ///
        /// This requires `tokio` runtime which was configured with 'enable_all()' to be running.
        pub async fn serve(self) -> Result<(), Error> {
            // - Collect system information
            let sys_info = SystemIntroduce {
                system_name: self.system_name,
                monitor_version: env!("CARGO_PKG_VERSION").into(),
                system_description: self.system_desc,
                desktop_name: if self.hide_host_name {
                    "<hidden>".into()
                } else {
                    hostname::get()
                        .ok()
                        .and_then(|s| s.into_string().ok())
                        .unwrap_or_else(|| "<unknown>".into())
                },
                num_cores: std::thread::available_parallelism()
                    .map(|x| x.get())
                    .unwrap_or(1),
                epoch_utc: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map(|x| x.as_secs())
                    .unwrap_or(0),
            };
            log::debug!("System info constructed: {sys_info:#?}");

            // - create axum router -> routes for /index.html and /api/ ...
            let context = Arc::new(Context {
                table: self.table,
                sys_info,
            });

            let route = axum::Router::default()
                .route("/", get(|| async { "Hello, world!" }))
                .route("/ws", get(Self::__on_ws))
                .with_state(context);

            axum::Server::bind(
                &format!("{}:{}", self.bind_ip.unwrap_or_else(|| "0.0.0.0".into()), self.bind_port)
                    .parse()?,
            )
            .serve(route.into_make_service_with_connect_info::<SocketAddr>())
            .with_graceful_shutdown(async {
                if let Some(fut) = self.close_signal {
                    fut.await
                } else {
                    std::future::pending().await
                }
            })
            .await
            .map_err(|e| axum::Error::new(e))?;

            Ok(())
        }

        /// Create a new tracing subscriber which can be linked to the service.
        pub fn create_tracing_subscriber(&mut self) -> crate::trace::Subscriber {
            todo!()
        }

        async fn __on_ws(
            State(ctx): State<Arc<Context>>,
            ws: axum::extract::WebSocketUpgrade,
            ConnectInfo(addr): ConnectInfo<SocketAddr>,
        ) -> impl IntoResponse {
            log::debug!("new remote websocket upgrade request: {addr}");

            ws.on_upgrade(move |ws| {
                crate::session::Desc::builder()
                    .context(ctx)
                    .remote(addr)
                    .rpc(super::ws_adapt::create_rpc_from(ws))
                    .build()
                    .execute()
            })
        }
    }
}

mod ws_adapt {
    use std::{pin::Pin, task::Poll};

    use axum::extract::ws::{Message, WebSocket};
    use futures::{
        stream::{SplitSink, SplitStream},
        SinkExt, StreamExt,
    };
    use rpc_it::{AsyncFrameRead, AsyncFrameWrite};

    pub fn create_rpc_from(ws: WebSocket) -> rpc_it::Handle {
        let (tx, rx) = ws.split();
        let (handle, t1, t2) = rpc_it::InitInfo::builder()
            .write(Box::new(Sink { ws: tx }))
            .read(Box::new(Source::new(rx)))
            .build()
            .start();

        tokio::spawn(t1);
        tokio::spawn(t2);

        handle
    }

    /* ---------------------------------------- Sink Impl --------------------------------------- */
    struct Sink {
        ws: SplitSink<WebSocket, Message>,
    }

    impl AsyncFrameWrite for Sink {
        fn poll_start_write(
            mut self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            _frame_size: usize,
        ) -> Poll<std::io::Result<()>> {
            self.ws.poll_ready_unpin(cx).map_err(map_err)
        }

        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            bufs: &[std::io::IoSlice<'_>],
        ) -> Poll<std::io::Result<usize>> {
            let num_all_bytes = bufs.iter().map(|b| b.len()).sum();
            let mut buf = Vec::with_capacity(num_all_bytes);

            for b in bufs {
                buf.extend_from_slice(b);
            }

            let msg = Message::Binary(buf);
            self.ws.start_send_unpin(msg).map_err(map_err)?;

            Poll::Ready(Ok(num_all_bytes))
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> Poll<std::io::Result<()>> {
            self.ws.poll_flush_unpin(cx).map_err(map_err)
        }
    }

    /* --------------------------------------- Source Impl -------------------------------------- */
    struct Source {
        ws: SplitStream<WebSocket>,
        inbound: Option<Message>,

        // cursor for front-post inbound message
        head_cursor: usize,
    }

    impl Source {
        fn new(ws: SplitStream<WebSocket>) -> Self {
            Self {
                ws,
                inbound: None,
                head_cursor: 0,
            }
        }
    }

    impl AsyncFrameRead for Source {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &mut [u8],
        ) -> Poll<std::io::Result<usize>> {
            let mut this = &mut *self;

            loop {
                match this.inbound.take() {
                    Some(Message::Binary(head_vec)) => {
                        let head = &head_vec[this.head_cursor..];
                        let len = std::cmp::min(head.len(), buf.len());

                        debug_assert!(head.len() > 0, "should not be empty");
                        debug_assert!(len > 0, "should not be empty");

                        buf[..len].copy_from_slice(&head[..len]);
                        this.head_cursor += len;

                        if this.head_cursor == head_vec.len() {
                            this.head_cursor = 0;
                        } else {
                            // partially consumed ... put it back
                            this.inbound = Some(Message::Binary(head_vec));
                        }

                        break Poll::Ready(Ok(len));
                    }

                    Some(Message::Close(_)) => {
                        break Poll::Ready(Ok(0));
                    }

                    Some(_) => {}

                    None => match this.ws.poll_next_unpin(cx) {
                        Poll::Ready(Some(Ok(msg))) => {
                            this.inbound = Some(msg);
                        }
                        Poll::Ready(Some(Err(e))) => {
                            break Poll::Ready(Err(map_err(e)));
                        }
                        Poll::Ready(None) => {
                            break Poll::Ready(Ok(0));
                        }
                        Poll::Pending => {
                            break Poll::Pending;
                        }
                    },
                }
            }
        }
    }

    /* ------------------------------------- Utility Method ------------------------------------- */
    fn map_err(e: impl std::error::Error + Send + Sync + 'static) -> std::io::Error {
        std::io::Error::new(std::io::ErrorKind::Other, e)
    }
}
