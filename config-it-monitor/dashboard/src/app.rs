use egui::Color32;
use instant::{Duration, Instant};

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
    #[serde(skip)]
    state: AppState,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            state: AppState::Uninit,
        }
    }
}

enum AppState {
    Uninit,

    Connecting(Instant, ConnectionTask),

    Active { rpc: rpc_it::Handle },

    Broken(Instant),
}

type ConnectionTask = oneshot::Receiver<anyhow::Result<rpc_it::Handle>>;

impl TemplateApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }
}

impl eframe::App for TemplateApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        const MAX_WAIT: Duration = std::time::Duration::from_secs(5);

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Edit", |_| {});
                ui.menu_button("View", |_| {});
            })
        });

        match &mut self.state {
            #[cfg(target_arch = "wasm32")]
            AppState::Uninit => {
                self.state = AppState::Connecting(Instant::now(), ws_wasm32::connect());
            }

            AppState::Connecting(epoch, task) => match task.try_recv() {
                Ok(Ok(rpc)) => self.state = AppState::Active { rpc },

                Ok(Err(err)) => {
                    log::error!("Failed to connect: {}", err);
                    self.state = AppState::Broken(Instant::now());
                }

                Err(oneshot::TryRecvError::Empty) => {
                    egui::Window::new("conn-msg")
                        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                        .title_bar(false)
                        .show(ctx, |ui| {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("connecting...");
                                let rate = (epoch.elapsed().as_secs_f32() * 255. / 5.) as _;
                                ui.colored_label(
                                    Color32::from_rgb(rate, 255 - rate, 0),
                                    format!("{:.2}s", epoch.elapsed().as_secs_f32()),
                                );
                            })
                        });

                    if epoch.elapsed() > MAX_WAIT {
                        log::warn!("failed to connect in time");
                        self.state = AppState::Broken(Instant::now());
                    }

                    ctx.request_repaint();
                }

                Err(oneshot::TryRecvError::Disconnected) => {
                    log::error!("Failed to connect: channel closed");
                    self.state = AppState::Broken(Instant::now());
                }
            },

            AppState::Active { rpc } => {
                log::debug!("UNIMPLEMENTED");
            }

            AppState::Broken(when) => {
                egui::Window::new("conn-msg")
                    .title_bar(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.colored_label(Color32::RED, "Failed to connect to server");
                        ui.horizontal(|ui| {
                            ui.label("retrying in ");
                            ui.colored_label(
                                Color32::GREEN,
                                format!(
                                    "{:.2}s",
                                    (MAX_WAIT.saturating_sub(when.elapsed())).as_secs_f32()
                                ),
                            )
                        })
                    });

                if when.elapsed() > MAX_WAIT {
                    self.state = AppState::Uninit;
                }

                ctx.request_repaint();
            }
        }
    }

    #[cfg(any())]
    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let Self { label, value } = self;

        // Examples of how to create different panels and windows.
        // Pick whichever suits you.
        // Tip: a good default choice is to just keep the `CentralPanel`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        _frame.close();
                    }
                });
            });
        });

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading("Side Panel");

            ui.horizontal(|ui| {
                ui.label("Write something: ");
                ui.text_edit_singleline(label);
            });

            ui.add(egui::Slider::new(value, 0.0..=10.0).text("value"));
            if ui.button("Increment").clicked() {
                *value += 1.0;
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label("powered by ");
                    ui.hyperlink_to("egui", "https://github.com/emilk/egui");
                    ui.label(" and ");
                    ui.hyperlink_to(
                        "eframe",
                        "https://github.com/emilk/egui/tree/master/crates/eframe",
                    );
                    ui.label(".");
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's

            ui.heading("eframe template");
            ui.hyperlink("https://github.com/emilk/eframe_template");
            ui.add(egui::github_link_file!(
                "https://github.com/emilk/eframe_template/blob/master/",
                "Source code."
            ));
            egui::warn_if_debug_build(ui);
        });

        egui::Window::new("Window").show(ctx, |ui| {
            ui.label("Windows can be moved by dragging them.");
            ui.label("They are automatically sized based on contents.");
            ui.label("You can turn on resizing and scrolling if you like.");
            ui.label("You would normally choose either panels OR windows.");
        });
    }
}

#[cfg(target_arch = "wasm32")]
mod ws_wasm32 {
    use std::{pin::Pin, task::Poll};

    use futures::{
        stream::{SplitSink, SplitStream},
        SinkExt, StreamExt,
    };
    use rpc_it::{AsyncFrameRead, AsyncFrameWrite};
    use ws_stream_wasm::{WsMessage, WsMeta, WsStream};

    use super::{spawn_task, ConnectionTask};

    pub fn connect() -> ConnectionTask {
        let (tx_reply, rx) = oneshot::channel::<anyhow::Result<rpc_it::Handle>>();
        spawn_task(async move {
            let remote = web_sys::window().unwrap().location().host().unwrap();
            let url = format!("ws://{}/ws", remote);
            log::debug!("connecting to: {url}");

            let (ws_meta, ws_io) = match WsMeta::connect(url.as_str(), None).await {
                Ok(ws) => ws,

                Err(e) => {
                    log::error!("Failed to connect: {}", e);
                    let _ = tx_reply.send(Err(e.into()));
                    return;
                }
            };

            let (tx, rx) = ws_io.split();

            log::debug!("connection successfully established. creating rpc instance ...");

            // TODO: create RPC instance using websocket stream.
            let (handle, t1, t2) = rpc_it::InitInfo::builder()
                .write(Box::new(Sink { ws: tx }))
                .read(Box::new(Source::new(rx)))
                .build()
                .start();

            spawn_task(async move {
                let (e1, e2) = futures::join!(t1, t2);
                if let Err(e) = e1 {
                    log::error!("rpc task 1 exited with error: {}", e);
                }
                if let Err(e) = e2 {
                    log::error!("rpc task 2 exited with error: {}", e);
                }
                drop(ws_meta)
            });

            let _ = tx_reply.send(Ok(handle));
        });

        rx
    }

    /* -------------------------------------- Sink Adapter -------------------------------------- */
    struct Sink {
        ws: SplitSink<WsStream, WsMessage>,
    }

    impl AsyncFrameWrite for Sink {
        fn poll_start_write(
            mut self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            _frame_size: usize,
        ) -> Poll<std::io::Result<()>> {
            self.ws.poll_ready_unpin(cx).map_err(map_err)
        }

        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            bufs: &[std::io::IoSlice<'_>],
        ) -> Poll<std::io::Result<usize>> {
            let num_all_bytes = bufs.iter().map(|b| b.len()).sum();
            let mut buf = Vec::with_capacity(num_all_bytes);

            for b in bufs {
                buf.extend_from_slice(b);
            }

            let msg = WsMessage::Binary(buf);
            self.ws.start_send_unpin(msg).map_err(map_err)?;

            Poll::Ready(Ok(num_all_bytes))
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> Poll<std::io::Result<()>> {
            self.ws.poll_flush_unpin(cx).map_err(map_err)
        }
    }

    /* ------------------------------------- Source Adapter ------------------------------------- */
    struct Source {
        ws: SplitStream<WsStream>,
        inbound: Option<WsMessage>,

        // cursor for front-post inbound message
        head_cursor: usize,
    }

    impl Source {
        fn new(ws: SplitStream<WsStream>) -> Self {
            Self {
                ws,
                inbound: None,
                head_cursor: 0,
            }
        }
    }

    impl AsyncFrameRead for Source {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &mut [u8],
        ) -> Poll<std::io::Result<usize>> {
            let mut this = &mut *self;

            loop {
                match this.inbound.take() {
                    Some(WsMessage::Binary(head)) => {
                        let head = &head[this.head_cursor..];
                        let len = std::cmp::min(head.len(), buf.len());

                        buf[..len].copy_from_slice(&head[..len]);
                        this.head_cursor += len;

                        if this.head_cursor == head.len() {
                            this.head_cursor = 0;
                        } else {
                            // partially consumed ... put it back
                            this.inbound = Some(WsMessage::Binary(head.to_vec()));
                        }

                        break Poll::Ready(Ok(len));
                    }

                    Some(WsMessage::Text(msg)) => {
                        log::warn!("unexpected text message: {msg}");
                    }

                    None => match this.ws.poll_next_unpin(cx) {
                        Poll::Ready(Some(msg)) => {
                            this.inbound = Some(msg);
                        }
                        Poll::Ready(None) => {
                            break Poll::Ready(Ok(0));
                        }
                        Poll::Pending => {
                            break Poll::Pending;
                        }
                    },
                }
            }
        }
    }

    /* ------------------------------------- Utility Method ------------------------------------- */
    fn map_err(e: impl std::error::Error + Send + Sync + 'static) -> std::io::Error {
        std::io::Error::new(std::io::ErrorKind::Other, e)
    }
}

pub fn spawn_task(fut: impl std::future::Future<Output = ()> + 'static) {
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(fut);

    #[cfg(not(target_arch = "wasm32"))]
    tokio::task::spawn_local(fut)
}

pub fn default<T: Default>() -> T {
    Default::default()
}
