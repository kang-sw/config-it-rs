use std::{net::SocketAddr, sync::Arc};

#[derive(typed_builder::TypedBuilder)]
pub(crate) struct Session {
    context: Arc<crate::server::Context>,
    remote: SocketAddr,
    rpc: rpc_it::Handle,

    #[builder(default, setter(skip))]
    moa: u32,
}

impl Session {
    pub fn execute(self) {}
}
