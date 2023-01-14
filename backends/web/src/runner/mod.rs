use std::{
    num::NonZeroUsize,
    time::{SystemTime, UNIX_EPOCH},
};

use futures::join;
use gethostname::gethostname;
use serde::Serialize;
use tokio::sync::mpsc;

mod api;

pub(super) struct Runner {}

#[derive(Serialize, Debug, Clone)]
struct SystemInfo {
    alias: String,
    description: String,
    epoch: u64,
    hostname: String,
    num_cores: usize,
}

enum Directive {}

type Handle = mpsc::UnboundedSender<Directive>;

impl Runner {
    pub async fn exec(desc: super::Builder) {
        let sys_info = SystemInfo {
            alias: desc.name,
            description: desc.description,
            epoch: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            hostname: gethostname().to_string_lossy().into(),
            num_cores: std::thread::available_parallelism()
                .unwrap_or(NonZeroUsize::new(1).unwrap())
                .into(),
        };

        // Make a close signal factory, and propagate it over system.
        let (tx_close, rx_close) = async_channel::bounded(1);
        let close_signal_task = async move {
            if let Some(sig) = desc.stop_signal {
                sig.await.ok();
                tx_close.try_send(()).ok();
            } else {
                // Just await forever. This lets tx_close instance alive, which let recv() call
                //  to rx_close never return, thus all other components which subscribe to
                //  rx_close will never be dropped.
                futures::future::pending::<()>().await;
            }
        };
        let create_close_signal = || {
            let rx_close = rx_close.clone();
            async move {
                rx_close.recv().await.ok();
            }
        };

        // Create routes for service API and websocket creation
        let (tx, mut rx) = mpsc::unbounded_channel();
        let router_task = api::ApiRouter::run(sys_info, desc.bind_port, tx, create_close_signal());

        // TODO: Create directive runner
        let runner_task = async {};

        // TODO: Create configuration / trace / terminal handlers

        join!(close_signal_task, router_task, runner_task);
    }
}
