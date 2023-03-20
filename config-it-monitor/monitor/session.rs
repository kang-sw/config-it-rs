use std::{net::SocketAddr, sync::Arc};

use anyhow::anyhow;
use rpc_it::{Inbound, RetrieveRoute};

use crate::common::{handshake, util::reply_as};

#[derive(typed_builder::TypedBuilder)]
pub(crate) struct Desc {
    context: Arc<crate::server::Context>,
    remote: SocketAddr,
    rpc: rpc_it::Handle,
}

struct Context {
    access_level: crate::common::AuthLevel,
    tx_control: flume::Sender<Control>,
    rx_control: flume::Receiver<Control>,
}

enum Control {}

impl Desc {
    #[tracing::instrument(skip(self), fields(remote = %self.remote))]
    pub async fn execute(mut self) {
        log::info!("executing new session for {}", self.remote);
        let context = match self.__handshake().await {
            Err(e) => {
                log::error!("error during handshake: {e}");
                return;
            }
            Ok(x) => x,
        };

        self.__loop(context).await;
    }

    #[tracing::instrument(skip(self), fields(remote = %self.remote))]
    async fn __handshake(&mut self) -> anyhow::Result<Context> {
        loop {
            let req = self
                .rpc
                .recv_inbound()
                .await
                .ok_or_else(|| anyhow!("disconnected"))?;

            let req = match req {
                Inbound::Request(req) => req,
                Inbound::Notify(noti) => {
                    log::warn!("unexpected notification during handshake: {:?}", noti.route_str());
                    continue;
                }
            };

            let route_str = req.route_str().unwrap_or("<invalid-str>");
            log::debug!("received route during handshake: {:?}", route_str);

            match route_str {
                handshake::LOGIN => {
                    let collect_all_storage = || {
                        self.context
                            .table
                            .0
                            .iter()
                            .map(|(key, val)| handshake::StorageDesc {
                                key: key.to_string(),
                                require_auth: val.access_keys.is_empty() == false,
                            })
                            .collect()
                    };

                    // TODO: Implement valid authntication reply:
                    // - when auth is invalid -> set error
                    let auth_level = crate::common::AuthLevel::Admin;
                    reply_as(
                        req,
                        &handshake::LoginResult {
                            auth_level,
                            storages: collect_all_storage(),
                        },
                    )
                    .await?;

                    // return with context. this flush is mandatory!
                    self.rpc.flush().await?;
                    let (tx, rx) = flume::unbounded();
                    break Ok(Context {
                        access_level: auth_level,
                        rx_control: rx,
                        tx_control: tx,
                    });
                }

                handshake::SYSTEM_INTRODUCE => {
                    reply_as(req, &self.context.sys_info).await?;
                }

                handshake::HELLO => {
                    req.reply([b"world"]).await?;
                }

                unknown => {
                    log::warn!("unknown route during handshake: {:?}", unknown);
                    continue;
                }
            }

            self.rpc.flush().await?;
        }
    }

    async fn __loop(mut self, mut ctx: Context) {
        loop {
            tokio::select! {
                msg = self.rpc.recv_inbound() => {
                    if let Some(msg) = msg {
                        if let Err(e) = self.__reply(&mut ctx, msg).await {
                            log::warn!("error during handling reply: {e}");
                        }
                    } else {
                        log::info!("disconnected");
                        break;
                    }
                }
                msg = ctx.rx_control.recv_async() => {
                    if let Ok(msg) = msg {
                        if let Err(e) = self.__control(&mut ctx, msg).await {
                            log::warn!("error during handling control: {e}");
                        }
                    } else {
                        log::info!("disconnected");
                        break;
                    }
                }
            }
        }
    }

    async fn __reply(&mut self, ctx: &mut Context, inbound: rpc_it::Inbound) -> anyhow::Result<()> {
        match inbound {
            rpc_it::Inbound::Request(req) => {
                let route_str = req.route_str().unwrap_or("<invalid-str>");

                match route_str {
                    unknown => {
                        log::warn!("unknown route name: {unknown}");
                    }
                }
            }

            rpc_it::Inbound::Notify(noti) => {}
        }

        Ok(())
    }

    async fn __control(&mut self, ctx: &mut Context, control: Control) -> anyhow::Result<()> {
        Ok(())
    }
}
