use tokio::sync::mpsc;

mod api;

pub(super) struct Runner {}

struct SystemInfo {
    alias: String,
    description: String,
    epoch: u64,
    hostname: String,
    num_cores: usize,
}

enum Directive {}

type DirectiveChannel = mpsc::UnboundedSender<Directive>;

impl Runner {
    pub async fn exec(desc: super::Service) {
        // TODO: Cache system information

        // TODO: Create routes for service API and websocket creation

        // TODO: Create
    }
}
