use config_it::ConfigGroupData;
use std::{
    io::Write,
    process::{self, Command, Stdio},
};

#[derive(config_it::ConfigGroupData, Clone, Default)]
pub struct MyStruct {
    #[config_it(min = 3)]
    minimal: i32,

    #[config_it(max = 3, one_of(1, 2, 3, 4, 5))]
    maximum: i32,

    #[config_it(one_of(("h".to_string())))]
    data: String,

    #[config_it(default = 3)]
    median: i32,

    my_invisible: f32,
}

#[test]
fn fewew() {
    let s = MyStruct::default();

    let echo = process::Command::new("rustfmt")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    echo.stdin.unwrap().write_all(hello().as_bytes()).unwrap();

    let stdout_fmt = Command::new("rustfmt")
        .stdin(echo.stdout.unwrap())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let content = stdout_fmt.wait_with_output().unwrap();
    println!("\n\n{}\n", std::str::from_utf8(&content.stdout).unwrap());
}
