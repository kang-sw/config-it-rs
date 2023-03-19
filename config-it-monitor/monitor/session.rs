use std::{net::SocketAddr, sync::Arc};

use anyhow::anyhow;
use rpc_it::{Inbound, RetrieveRoute};

use crate::common::{handshake, util::reply_as};

#[derive(typed_builder::TypedBuilder)]
pub(crate) struct Session {
    context: Arc<crate::server::Context>,
    remote: SocketAddr,
    rpc: rpc_it::Handle,

    #[builder(default, setter(skip))]
    moa: u32,
}

impl Session {
    pub async fn execute(mut self) {
        log::info!("executing new session for {}", self.remote);
        if let Err(e) = self.__handshake().await {
            log::error!("error during handshake: {e}");
        }

        std::future::pending().await
    }

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

            match req.route_str().unwrap_or("<invalid-str>") {
                handshake::LOGIN => {
                    reply_as(
                        req,
                        &handshake::LoginResult {
                            auth_level: crate::common::AuthLevel::Admin,
                        },
                    )
                    .await?;

                    // TODO: Implement valid authntication reply:
                    // - when auth is invalid -> retry

                    break Ok(());
                }

                handshake::SYSTEM_INTRODUCE => {}

                unknown => {
                    log::warn!("unknown route during handshake: {:?}", unknown);
                    continue;
                }
            }
        }
    }
}
