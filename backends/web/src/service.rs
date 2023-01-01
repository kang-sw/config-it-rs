use std::net::{IpAddr, Ipv4Addr};

use async_channel::{Receiver, Sender};
use config_it::CompactString;
use futures::Future;
use tokio::sync::oneshot;

use crate::runner::Runner;

///
/// Serves multiple config storages on specified access point.
///
/// TODO: Authentication method
/// TODO: Public file path
///
pub struct Service {
    storages: Vec<config_it::Storage>,
    bind_addr: IpAddr,
    bind_port: u16,

    /// Name of this system. This will be displayed in the web interface.
    name: String,

    /// Description of this system. This will be displayed in the web interface.
    description: String,

    /// A string channel that can be used to log messages
    rx_log: Option<Receiver<CompactString>>,

    /// A string channel for accepting commands from remote side client.
    tx_cmd: Option<Sender<CompactString>>,

    /// A oneshot signal that can be used to stop the service.
    stop_signal: Option<oneshot::Receiver<()>>,
}

fn default<T: Default>() -> T {
    T::default()
}

impl Service {
    pub fn new() -> Self {
        Self {
            storages: default(),
            bind_port: 15572,
            bind_addr: Ipv4Addr::UNSPECIFIED.into(),
            name: "default".into(),
            description: default(),
            rx_log: None,
            tx_cmd: None,
            stop_signal: None,
        }
    }

    pub fn add_storage(mut self, storage: config_it::Storage) -> Self {
        self.storages.push(storage);
        self
    }

    pub fn with_bind_addr(mut self, addr: IpAddr) -> Self {
        self.bind_addr = addr;
        self
    }

    pub fn with_bind_port(mut self, port: u16) -> Self {
        self.bind_port = port;
        self
    }

    pub fn with_name(mut self, name: String) -> Self {
        self.name = name;
        self
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = description;
        self
    }

    pub fn with_log_sink(mut self, rx: Receiver<CompactString>) -> Self {
        self.rx_log = Some(rx);
        self
    }

    pub fn with_command_source(mut self, tx: Sender<CompactString>) -> Self {
        self.tx_cmd = Some(tx);
        self
    }

    pub fn with_stop_signal(mut self, rx: oneshot::Receiver<()>) -> Self {
        self.stop_signal = Some(rx);
        self
    }

    pub async fn run(self) {
        Runner::exec(self).await;
    }
}
