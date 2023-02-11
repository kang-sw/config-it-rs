mod public_path;

use std::net::{IpAddr, Ipv4Addr};

use async_channel::Sender;
use config_it::CompactString;
use tokio::sync::oneshot;

use crate::{misc::default, runner::Runner};

use self::public_path::PublicPath;

///
/// Serves multiple config storages on specified access point.
///
/// TODO: Authentication method
/// TODO: Public file path
///
pub struct Builder {
    pub storages: Vec<config_it::Storage>,
    pub bind_addr: IpAddr,
    pub bind_port: u16,

    /// Path to public files
    pub public_path: PublicPath,

    /// Name of this system. This will be displayed in the web interface.
    pub name: String,

    /// Description of this system. This will be displayed in the web interface.
    pub description: String,

    /// A string channel for accepting commands from remote side client.
    pub tx_cmd: Option<Sender<CompactString>>,

    /// A oneshot signal that can be used to stop the service.
    pub stop_signal: Option<oneshot::Receiver<()>>,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            storages: default(),
            bind_port: 15572,
            bind_addr: Ipv4Addr::UNSPECIFIED.into(),
            name: "default".into(),
            description: default(),
            public_path: default(),
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

    pub fn with_command_source(mut self, tx: Sender<CompactString>) -> Self {
        self.tx_cmd = Some(tx);
        self
    }

    pub fn with_stop_signal(mut self, rx: oneshot::Receiver<()>) -> Self {
        self.stop_signal = Some(rx);
        self
    }

    /// Creates a log sink that can be used to redirect log outputs to the service.
    ///
    /// > **Warning**: Do not use this method with `with_trace_sink` method when log is
    ///   redirected as trace event.
    pub fn create_log_sink(&mut self) -> Box<dyn Fn(&log::Record)> {
        todo!("Create a log sink, associate it with the service.");
    }

    pub async fn run(self) {
        Runner::exec(self).await;
    }
}
