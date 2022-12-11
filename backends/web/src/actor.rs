use std::{num::NonZeroUsize, sync::Arc, time::SystemTime};

use config_it::CompactString;
use tokio::sync::oneshot;

pub(super) struct Context {
    cached_sys_info: Arc<SystemInfo>,
    storages: Vec<config_it::Storage>,
    command_stream: Option<async_channel::Sender<String>>,
    terminal_stream: Option<async_channel::Receiver<String>>,
}

pub(super) enum Directive {
    GetSystemInfo(oneshot::Sender<Arc<SystemInfo>>),
}

#[derive(Debug, serde::Serialize)]
pub struct SystemInfo {
    alias: CompactString,
    description: String,
    epoch: u64,
    version: String,
    hostname: String,
    num_cores: usize,
}

impl Context {
    pub fn new(init: super::Builder) -> Self {
        Self {
            cached_sys_info: SystemInfo {
                alias: init.app_name.into(),
                description: init.description.clone(),
                epoch: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                version: env!("CARGO_PKG_VERSION").into(),
                hostname: whoami::hostname(),
                num_cores: std::thread::available_parallelism()
                    .unwrap_or(NonZeroUsize::new(1).unwrap())
                    .into(),
            }
            .into(),
            storages: init.storage,
            command_stream: init.command_stream,
            terminal_stream: init.terminal_stream,
        }
    }

    ///
    /// Run event loop
    ///
    pub async fn run(mut self, rx: async_channel::Receiver<Directive>) {
        log::debug!("Starting monitor event loop ...");

        while let Ok(directive) = rx.recv().await {
            match directive {
                Directive::GetSystemInfo(reply) => {
                    let _ = reply.send(self.cached_sys_info.clone());
                }
            }
        }

        log::debug!("Monitor event loop stopped.");
    }
}
