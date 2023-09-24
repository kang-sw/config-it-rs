#[derive(config_it::Template, Clone)]
struct Foo {
    /// This is to
    /// Multi
    ///
    /// ANd
    #[config(default = 14, env_once = "DLOGIIO")]
    var: u32,

    #[config(default = (14, 8))]
    varg: (u32, u32),

    #[config(default = "hello", env = "ADLSOC")]
    vk: String,

    #[config(one_of=[1,3,4])]
    tew: u32,

    #[non_config_default_expr = "4"]
    _other: i32,
}
