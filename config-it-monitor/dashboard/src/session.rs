use std::{cell::RefCell, rc::Rc};

use anyhow::anyhow;
use capture_it::capture;
use egui::Color32;
use wasm_bindgen_futures::spawn_local;

use crate::common::{
    handshake::{self, LoginResult, SystemIntroduce},
    util::remote_call,
};

#[derive(Default, Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct UIState {}

pub struct Instance {
    rpc: rpc_it::Handle,
    ui_state: Rc<RefCell<UIState>>,

    state: State,

    tx_flush: flume::Sender<()>,
}

enum State {
    HandShake(Rc<RefCell<StateRenderFn>>),
    Active(HandshakeResult),
}

struct HandshakeResult {
    sys_info: SystemIntroduce,
    login_result: LoginResult,
}

type StateRenderFn = Box<dyn FnMut(&mut egui::Ui) -> anyhow::Result<Option<HandshakeResult>>>;

fn new_state<T: 'static + FnMut(&mut egui::Ui) -> anyhow::Result<Option<HandshakeResult>>>(
    s: &RefCell<StateRenderFn>,
    f: T,
) {
    *s.borrow_mut() = Box::new(f);
}

impl Instance {
    pub fn new(rpc: rpc_it::Handle, state: Rc<RefCell<UIState>>) -> Self {
        let func: StateRenderFn = Box::new(|ui| {
            ui.label("handshaking ...");
            Ok(None)
        });

        let render_fn = Rc::new(RefCell::new(func));

        // Spawn periodic flush task
        let w_rpc = rpc_it::Handle::downgrade(&rpc);
        let (tx_flush, flush_receiver) = flume::bounded(1);
        spawn_local(async move {
            loop {
                let Ok(_) = flush_receiver.recv_async().await else {
                    break;
                };

                if let Some(rpc) = w_rpc.upgrade() {
                    if let Err(e) = rpc.flush().await {
                        log::error!("failed to flush rpc: {}", e);
                        break;
                    }
                } else {
                    break;
                }
            }
        });

        spawn_local(capture!([rpc, state, render_fn], async move {
            if let Err(e) = Self::__handshake(rpc, state, render_fn.clone()).await {
                let mut e = Some(e);
                new_state(&render_fn, move |_| e.take().map(|x| Err(x)).unwrap_or(Ok(None)));
            }
        }));

        Self {
            rpc,
            ui_state: state,
            state: State::HandShake(render_fn),
            tx_flush,
        }
    }

    async fn __handshake(
        rpc: rpc_it::Handle,
        state: Rc<RefCell<UIState>>,
        render_fn: Rc<RefCell<StateRenderFn>>,
    ) -> anyhow::Result<()> {
        let mut add_stage =
            capture!([render_fn, *stages = Vec::default()], move |stage: &str, desc: &str| {
                stages.push((stage.to_string(), desc.to_string()));

                new_state(
                    &render_fn,
                    capture!([stages], move |ui| {
                        ui.colored_label(Color32::WHITE, "✴ handshaking in progress ...");
                        ui.separator();

                        for (idx, (stage, desc)) in stages.iter().enumerate() {
                            let is_last = idx == stages.len() - 1;

                            ui.horizontal(|ui| {
                                ui.label("▪");

                                ui.colored_label(
                                    is_last.then(|| Color32::YELLOW).unwrap_or(Color32::GREEN),
                                    stage.as_str(),
                                );

                                ui.label(desc.as_str());
                                if !is_last {
                                    ui.colored_label(Color32::GREEN, "✔ done!");
                                } else {
                                    ui.spinner();
                                }
                            });
                        }

                        Ok(None)
                    }),
                );
            });

        add_stage("hello", "sending request");
        let rep = rpc.request(handshake::HELLO, &["hello"]).await?;

        add_stage("hello", "waiting reply");
        let rep = rep.await.ok_or_else(|| anyhow!("RPC closed"))?.result()?;

        if rep.payload() != b"world" {
            return Err(anyhow!("unexpected reply: {:?}", std::str::from_utf8(rep.payload())));
        }

        add_stage("system info", "fetching");
        let rep: handshake::SystemIntroduce =
            remote_call(&rpc, handshake::SYSTEM_INTRODUCE, &"").await?;
        log::debug!("System information fetched: {rep:#?}");

        // TODO: Logging in ...

        std::future::pending().await
    }

    pub fn ui_update(
        &mut self,
        ctx: &egui::Context,
        frame: &mut eframe::Frame,
    ) -> anyhow::Result<bool> {
        let _ = self.tx_flush.try_send(());

        match &mut self.state {
            State::HandShake(renderer) => {
                let inner = egui::Window::new("Handshaking...")
                    .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                    .title_bar(false)
                    .show(ctx, |ui| (renderer.borrow_mut())(ui))
                    .unwrap()
                    .inner
                    .unwrap();

                let next = match inner {
                    Ok(Some(result)) => Some(result),

                    Ok(None) => None,

                    Err(e) => {
                        return Err(e);
                    }
                };

                if let Some(result) = next {
                    self.state = State::Active(result);
                }
            }

            State::Active(_) => {
                // TODO: do nothing
            }
        }

        Ok(true)
    }
}
