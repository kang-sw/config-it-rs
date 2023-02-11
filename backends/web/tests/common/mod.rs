use config_it::Template;

#[derive(Template, Clone, Default)]
pub struct ExampleConfig {
    #[config_it]
    pub field_i32: i32,

    #[config_it]
    pub field_u32: u32,
}
