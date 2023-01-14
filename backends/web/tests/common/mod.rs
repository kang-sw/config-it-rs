use config_it::ConfigGroupData;

#[derive(ConfigGroupData, Clone, Default)]
pub struct ExampleConfig {
    #[config_it]
    pub field_i32: i32,

    #[config_it]
    pub field_u32: u32,
}
