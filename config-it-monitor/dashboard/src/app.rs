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
                    egui::Window::new("Connecting")
                        .id("conn-msg".into())
                        .show(ctx, |ui| {
                            ui.horizontal(|ui| {
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
                egui::Window::new("Connection Broken")
                    .id("conn-msg".into())
                    .show(ctx, |ui| {
                        ui.label("Failed to connect to server");
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
    use pharos::{Observable, ObserveConfig};
    use ws_stream_wasm::WsMeta;

    use super::{spawn_task, ConnectionTask};

    pub fn connect() -> ConnectionTask {
        let (tx, rx) = oneshot::channel::<anyhow::Result<rpc_it::Handle>>();
        spawn_task(async move {
            let remote = web_sys::window().unwrap().location().host().unwrap();
            let url = format!("ws://{}/ws", remote);
            log::debug!("connecting to: {url}");

            let (mut ws_meta, _ws_io) = match WsMeta::connect(url.as_str(), None).await {
                Ok(ws) => ws,

                Err(e) => {
                    log::error!("Failed to connect: {}", e);
                    drop(tx.send(Err(e.into())));
                    return;
                }
            };

            let Ok(observer) = ws_meta.observe(ObserveConfig::default()).await else {
                log::error!("Failed to connect: observe failed");
                drop(tx.send(Err(anyhow::anyhow!("observe failed"))));
                return;
            };

            log::debug!("connection successfully established. creating rpc instance ...");

            // TODO: create RPC instance using websocket stream.
        });

        rx
    }
}

pub fn spawn_task(fut: impl std::future::Future<Output = ()> + 'static) {
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(fut);

    #[cfg(not(target_arch = "wasm32"))]
    tokio::task::spawn_local(fut)
}
