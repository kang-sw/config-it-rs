use std::rc::Rc;

use chrono::TimeZone;
use egui::Color32;
use instant::Duration;

use crate::tabs;

use super::SessionContext;

#[derive(derive_new::new)]
pub(super) struct SysView {
    context: Rc<SessionContext>,

    #[new(default)]
    editing_auth: String,
}

macro_rules! rich {
($str:expr) => {
    RichText::new($str)
};

($color:expr, $str:expr) => {
    RichText::new($str).color($color)
};

($color:expr, f, $($format:tt)+) => {
    RichText::new(format!($($format)*)).color($color)
};
}

impl tabs::Tab for SysView {
    fn ui(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        use egui::*;

        /* ----------------------------------- Sys Info Viewer ---------------------------------- */

        CollapsingHeader::new(rich!(Color32::WHITE, "🖧 System Information").heading())
            .default_open(true)
            .show(ui, |ui| {
                egui::Grid::new("grid")
                    .num_columns(2)
                    .striped(true)
                    .spacing([40., 4.])
                    .show(ui, |ui| {
                        macro_rules! tag {
                        ($ui:ident, label, $str:literal) => {
                            $ui.label(RichText::new($str));
                        };

                        ($ui:ident, value, $color:expr, $($format:tt)*) => {
                            $ui.label(
                                RichText::new(format!($($format)*)).color($color).monospace(),
                            );
                        };
                    }

                        let sys_info = &self.context.sys_info;
                        tag!(ui, label, "System Name");
                        ui.horizontal(|ui| {
                            tag!(ui, value, Color32::GREEN, "{}", sys_info.system_name);
                            ui.label("@");
                            tag!(ui, value, Color32::GRAY, "{}", sys_info.desktop_name);
                        });
                        ui.end_row();

                        tag!(ui, label, "Monitor Version");
                        tag!(ui, value, Color32::WHITE, "{}", sys_info.monitor_version);
                        ui.end_row();

                        tag!(ui, label, "System Description");
                        tag!(ui, value, Color32::WHITE, "{}", sys_info.system_description);
                        ui.end_row();

                        tag!(ui, label, "Uptime");
                        let now = chrono::Utc::now();
                        let epoch = chrono::Utc
                            .timestamp_millis_opt((sys_info.epoch_utc * 1000) as _)
                            .single()
                            .unwrap_or(now);

                        let offset = now - epoch;
                        if offset.num_seconds() > 0 {
                            ui.horizontal(|ui| {
                                tag!(
                                    ui,
                                    value,
                                    Color32::LIGHT_BLUE,
                                    "{}",
                                    humantime::format_duration(Duration::from_secs(
                                        offset.num_seconds() as _
                                    ))
                                );

                                let epoch = chrono::Local.from_utc_datetime(&epoch.naive_utc());
                                tag!(ui, value, Color32::GRAY, "({})", epoch.to_rfc2822());
                            });
                        }
                        ui.end_row();

                        tag!(ui, label, "Number of Cores");
                        tag!(ui, value, Color32::WHITE, "{}", sys_info.num_cores);
                        ui.end_row();
                    });

                ui.separator();
            });

        /* ------------------------------------ System Status ----------------------------------- */

        // TODO: system status graph -> memory usage / cpu usage / thread count -> history

        /* ----------------------------------- Storage Viewer ----------------------------------- */

        ui.collapsing(rich!(Color32::WHITE, "⛃ Storages").heading(), |ui| {
            ui.horizontal_wrapped(|ui| {
                for storage in &self.context.login_result.storages {
                    let key_text = if storage.require_auth {
                        rich!(Color32::LIGHT_YELLOW, storage.key.clone() + "  🔑")
                    } else {
                        rich!(Color32::WHITE, &storage.key)
                    };

                    let tracking = &self.context.tracking_storages;
                    let already_tracked = tracking.borrow().contains(&storage.key);
                    let key_text = if already_tracked {
                        key_text.color(Color32::GREEN)
                    } else {
                        key_text
                    };

                    ui.menu_button(key_text.monospace().size(13.), |ui| {
                        if !already_tracked {
                            let connect_button = if storage.require_auth {
                                ui.label("Enter passphrase");
                                ui.horizontal(|ui| {
                                    ui.label("🔑");
                                    TextEdit::singleline(&mut self.editing_auth)
                                        .password(true)
                                        .show(ui);
                                });

                                ui.separator();
                                ui.button("Try Connect")
                            } else {
                                ui.button("Connect")
                            };

                            if connect_button.clicked() {
                                // TODO: Create asynchronous request to connect to storage ...
                                // TODO: Once connection ops done,
                                tracking.borrow_mut().insert(storage.key.clone());
                            }
                        } else {
                            if ui.button("Untrack Storage").clicked() {
                                // TODO: Send asynchronous task to disconnect from storage ...
                                tracking.borrow_mut().remove(&storage.key);
                            }
                        }
                    });
                }
            });
        });
    }

    fn title(&mut self) -> egui::WidgetText {
        egui::RichText::new("🏠 Home")
            .color(Color32::WHITE)
            .text_style(egui::TextStyle::Monospace)
            .into()
    }
}
