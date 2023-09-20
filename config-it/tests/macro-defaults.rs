#[derive(config_it::Template, Clone)]
struct Foo {
    /// This is to
    /// Multi
    ///
    /// ANd
    #[config(default = 14, desc = "This is a var")]
    var: u32,
}
