use std::{cell::RefCell, mem::take, rc::Rc};

use anyhow::anyhow;
use capture_it::capture;
use egui::Color32;
use egui_dock::NodeIndex;
use rpc_it::RetrievePayload;
use wasm_bindgen_futures::spawn_local;

use crate::{
    app::default,
    common::{
        handshake::{self, LoginRequest, LoginResult, SystemIntroduce},
        util::{remote_call, JsonPayload},
    },
    tabs,
};

type StateRenderFn = Box<dyn FnMut(&mut egui::Ui) -> anyhow::Result<Option<SessionStageActive>>>;

/* ---------------------------------------------------------------------------------------------- */
/*                                      CONSISTENT UI STATES                                      */
/* ---------------------------------------------------------------------------------------------- */
#[derive(Default, Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct UIState {}

/* ---------------------------------------------------------------------------------------------- */
/*                                     INSTANCE IMPLEMENTATION                                    */
/* ---------------------------------------------------------------------------------------------- */
pub struct Instance {
    rpc: rpc_it::Handle,
    ui_state: Rc<RefCell<UIState>>,

    state: SessionStage,

    tx_flush: flume::Sender<()>,
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
                replace_handshake_rendering(&render_fn, move |_| {
                    e.take().map(|x| Err(x)).unwrap_or(Ok(None))
                });
            }
        }));

        Self {
            rpc,
            ui_state: state,
            state: SessionStage::HandShake(render_fn),
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
                if stages
                    .last()
                    .map(|(s, d)| *s == stage && *d == desc)
                    .unwrap_or(false)
                {
                    // Skip adding when the stage duplicates ...
                } else {
                    stages.push((stage.to_string(), desc.to_string()));
                }

                replace_handshake_rendering(
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
        let sys_info: handshake::SystemIntroduce =
            remote_call(&rpc, handshake::SYSTEM_INTRODUCE, &"")
                .await?
                .result()?
                .json_payload()?;
        log::debug!("System information fetched: {sys_info:#?}");

        let mut id = String::default();
        let mut pw = String::default();
        add_stage("login", "setup");

        let login_result = loop {
            // Send request first. This naively assumes the server would not require authentication
            match remote_call(&rpc, handshake::LOGIN, &LoginRequest::new(id.clone(), &pw))
                .await?
                .result()
                .map(|x| x.json_payload::<LoginResult>())
            {
                Ok(Ok(result)) => {
                    break result;
                }

                Ok(Err(e)) => {
                    log::debug!("failed to parse json payload: {e:#}");
                }

                Err(e) => {
                    log::debug!("authentication failed, retrying ({e:#})");
                }
            }

            // Spawn auth_info modifier
            let (tx_auth_info, rx_auth_info) = oneshot::channel();

            replace_handshake_rendering(
                &render_fn,
                capture!([*id, *pw, *tx = Some(tx_auth_info)], move |ui| {
                    ui.colored_label(Color32::WHITE, "🔑 login");
                    ui.separator();

                    egui::Grid::new("grid").num_columns(2).show(ui, |ui| {
                        ui.label("id");
                        ui.text_edit_singleline(&mut id);

                        ui.end_row();

                        let drag = ui.horizontal(|ui| {
                            ui.label("password");
                            ui.button("👁")
                                .interact(egui::Sense::drag())
                                .on_hover_text("👁 click to show")
                                .dragged()
                        });

                        egui::TextEdit::singleline(&mut pw)
                            .password(drag.inner == false)
                            .show(ui);
                    });

                    ui.add_space(5.);

                    ui.with_layout(egui::Layout::top_down_justified(egui::Align::Center), |ui| {
                        if ui.button("\nLogin\n").clicked() {
                            let pair = (take(&mut id), take(&mut pw));
                            tx.take().map(|x| x.send(pair).unwrap());
                        }
                    });

                    Ok(None)
                }),
            );

            let Ok(s) = rx_auth_info.await else {
                anyhow::bail!("login canceled")
            };

            (id, pw) = s;
            add_stage("login", "logging in ...");
        };

        log::debug!("login successful. {login_result:#?}");
        add_stage("login successful", "starting ...");

        let session_context: Rc<_> = SessionContext {
            login_result,
            sys_info,
            rpc: rpc.clone(),
            ui_state: state.clone(),
        }
        .into();

        let result = SessionStageActive {
            session_context: session_context.clone(),
            ui_tree: egui_dock::Tree::new(vec![Box::new(SysView {
                context: session_context.clone(),
            })]),
        };

        replace_handshake_rendering(
            &render_fn,
            capture!([*result = Some(result)], move |_| { Ok(result.take()) }),
        );
        Ok(())
    }

    pub fn ui_update(
        &mut self,
        ctx: &egui::Context,
        _frame: &mut eframe::Frame,
    ) -> anyhow::Result<bool> {
        let _ = self.tx_flush.try_send(());

        match &mut self.state {
            SessionStage::HandShake(renderer) => {
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
                    self.state = SessionStage::Active(result);
                }
            }

            SessionStage::Active(SessionStageActive {
                ui_tree,
                session_context,
                ..
            }) => {
                if self.rpc.is_closed() {
                    anyhow::bail!("RPC has been closed");
                }

                let mut vwr = TabViewer {
                    session_context: session_context.clone(),
                    added_nodes: default(),
                    ui_ctx: ctx,
                };

                egui_dock::DockArea::new(ui_tree)
                    .style(
                        egui_dock::StyleBuilder::from_egui(&ctx.style())
                            .show_add_buttons(true)
                            .show_add_popup(true)
                            .show_close_buttons(true)
                            .show_context_menu(true)
                            .build(),
                    )
                    .show(ctx, &mut vwr)
            }
        }

        Ok(true)
    }
}

/* ---------------------------------------------------------------------------------------------- */
/*                                   AVAILABLE RENDERING STAGES                                   */
/* ---------------------------------------------------------------------------------------------- */
enum SessionStage {
    HandShake(Rc<RefCell<StateRenderFn>>),
    Active(SessionStageActive),
}

struct SessionStageActive {
    session_context: Rc<SessionContext>,
    ui_tree: egui_dock::Tree<Box<dyn tabs::Tab>>,
}

fn replace_handshake_rendering<
    T: 'static + FnMut(&mut egui::Ui) -> anyhow::Result<Option<SessionStageActive>>,
>(
    s: &RefCell<StateRenderFn>,
    f: T,
) {
    *s.borrow_mut() = Box::new(f);
}

struct SessionContext {
    ui_state: Rc<RefCell<UIState>>,
    rpc: rpc_it::Handle,

    sys_info: SystemIntroduce,
    login_result: LoginResult,
    // TODO: list of storages, which are being watched
    // TODO:
}

/* ---------------------------------------------------------------------------------------------- */
/*                                    TAB VIEWER IMPLEMENTATION                                   */
/* ---------------------------------------------------------------------------------------------- */
struct TabViewer<'a> {
    ui_ctx: &'a egui::Context,
    added_nodes: Vec<(NodeIndex, Box<dyn tabs::Tab>)>,
    session_context: Rc<SessionContext>,
}

impl<'a> egui_dock::TabViewer for TabViewer<'a> {
    type Tab = Box<dyn tabs::Tab>;

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        tab.ui(self.ui_ctx, ui)
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.title()
    }

    fn context_menu(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        tab.context_menu(ui)
    }

    fn on_tab_button(&mut self, tab: &mut Self::Tab, response: &egui::Response) {
        tab.on_tab_button(response)
    }

    fn on_close(&mut self, tab: &mut Self::Tab) -> bool {
        tab.on_close()
    }

    fn add_popup(&mut self, _ui: &mut egui::Ui, _node: NodeIndex) {
        // TODO: add new storage view
        // TODO: storage view / file view / log view / system view
    }

    fn force_close(&mut self, tab: &mut Self::Tab) -> bool {
        tab.pending_close()
    }

    fn inner_margin_override(&self, style: &egui_dock::Style) -> egui::Margin {
        style.default_inner_margin
    }
}

struct SysView {
    context: Rc<SessionContext>,
}

impl tabs::Tab for SysView {
    fn ui(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.label("todo!(system view here)");
    }

    fn title(&mut self) -> egui::WidgetText {
        egui::RichText::new("placeholder").into()
    }
}
