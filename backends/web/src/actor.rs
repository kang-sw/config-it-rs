use std::{num::NonZeroUsize, sync::Arc, time::SystemTime};

use config_it::CompactString;
use tokio::sync::{mpsc, oneshot};
use tokio::task;

pub(super) struct Context {
    sys_info: Arc<SystemInfo>,
    storages: Vec<config_it::Storage>,
    command_stream: Option<async_channel::Sender<String>>,
}

pub(super) enum Directive {
    Shutdown,

    GetSystemInfo(oneshot::Sender<Arc<SystemInfo>>),

    AppendLine(String),
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
    pub async fn launch(init: super::Builder) -> mpsc::UnboundedSender<Directive> {
        let (tx, rx) = mpsc::unbounded_channel();

        // Spawn terminal handler task if `init` has terminal_stream
        if let Some(terminal_stream) = init.terminal_stream {
            let tx = tx.clone();
            task::spawn(async move {
                while let Ok(line) = terminal_stream.recv().await {
                    let _ = tx.send(Directive::AppendLine(line));
                }

                log::debug!("Terminal stream closed.");
            });
        }

        // Spawn close signal handler task if `init` has close_signal
        if let Some(close_signal) = init.close_signal {
            let tx = tx.clone();
            task::spawn(async move {
                if let Ok(_) = close_signal.await {
                    let _ = tx.send(Directive::Shutdown);
                }
            });
        }

        // Spawn context
        let context = Self {
            sys_info: SystemInfo {
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
        };

        task::spawn(context.run_(rx));

        tx
    }

    ///
    /// Run event loop
    ///
    async fn run_(mut self, mut rx: mpsc::UnboundedReceiver<Directive>) {
        log::debug!("Starting monitor event loop ...");

        while let Some(directive) = rx.recv().await {
            match directive {
                Directive::GetSystemInfo(reply) => {
                    let _ = reply.send(self.sys_info.clone());
                }

                Directive::Shutdown => {
                    log::debug!("Shutdown directive received, stopping monitor event loop ...");
                    break;
                }

                Directive::AppendLine(line) => {
                    todo!("Append line to circular buffer, and publish to active clients.")
                }
            }
        }

        log::debug!("Monitor event loop stopped.");
    }
}
