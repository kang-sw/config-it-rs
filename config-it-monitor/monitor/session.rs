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
}

impl Desc {
    #[tracing::instrument(skip(self), fields(remote = %self.remote))]
    pub async fn execute(mut self) {
        log::info!("executing new session for {}", self.remote);
        if let Err(e) = self.__handshake().await {
            log::error!("error during handshake: {e}");
            return;
        }

        self.__loop().await;
    }

    #[tracing::instrument(skip(self), fields(remote = %self.remote))]
    async fn __handshake(&mut self) -> anyhow::Result<()> {
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
                    reply_as(
                        req,
                        &handshake::LoginResult {
                            auth_level: crate::common::AuthLevel::Admin,
                        },
                    )
                    .await?;

                    // TODO: Implement valid authntication reply:
                    // - when auth is invalid -> set error

                    break Ok(());
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

    async fn __loop(mut self) {
        // TODO: implement me!
        log::error!("we're just expiring this connection ...")
    }
}
