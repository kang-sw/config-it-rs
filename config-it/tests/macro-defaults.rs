#[derive(config_it::Template)]
struct Foo {
    #[config(default = 14)]
    var: u32,
}
