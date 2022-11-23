use config_it::ConfigGroupData;
use std::{
    io::Write,
    process::{self, Command, Stdio},
};

#[derive(ConfigGroupData)]
pub struct MyStruct<T> {
    #[config_it(min = 3)]
    minimal: i32,

    my_arg: T,
}

#[test]
fn fewew() {
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
    println!("STDOUT: {}\n", std::str::from_utf8(&content.stdout).unwrap());
}
