use egui::*;

pub trait Tab {
    fn ui(&mut self, _ctx: &Context, ui: &mut Ui);
    fn title(&mut self) -> WidgetText;

    fn context_menu(&mut self, _ui: &mut Ui) {}
    fn on_tab_button(&mut self, _response: &Response) {}
    fn on_close(&mut self) -> bool {
        false
    }
    fn pending_close(&mut self) -> bool {
        false
    }
}
