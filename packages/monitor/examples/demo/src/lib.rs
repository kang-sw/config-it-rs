mod demo_cfg {
    #[derive(config_it::Template, Clone)]
    pub struct DemoConfig1 {}
}

#[derive(Default)]
pub struct App {}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        todo!()
    }
}
