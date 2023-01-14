use std::{
    num::NonZeroUsize,
    time::{SystemTime, UNIX_EPOCH},
};

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

        // Create routes for service API and websocket creation
        let (tx, rx) = mpsc::unbounded_channel();
        let router_task = api::ApiRouter::run(sys_info, tx);

        // TODO: Create configuration / trace / terminal handlers

        tokio::select! {
            _ = router_task => {}
        };
    }
}
