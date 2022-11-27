use config_it::ConfigGroupData;
use config_it::Group;
use futures::executor::{self, block_on};
use std::{
    io::Write,
    process::{self, Command, Stdio},
    thread,
};

#[derive(Clone, ConfigGroupData, Default)]
pub struct MyStruct {
    #[config_it(min = 0, min = 3)]
    minimal: i32,

    #[config_it(max = 3, one_of(1, 2, 3, 4, 5))]
    maximum: i32,

    #[config_it(one_of("a"))]
    data: String,

    #[config_it(default = 3)]
    median: i32,

    my_invisible: f32,
}

#[cfg(none)]
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

// #[cfg(none)]
#[test]
fn create_config_set() {
    let (storage, worker) = config_it::Storage::new();
    thread::spawn(move || block_on(worker));

    let group = storage.create_group::<MyStruct>(["hello", "world!"]);
    let mut group: Group<_> = block_on(group).unwrap();

    let group_2 = storage.create_group::<MyStruct>(["hello", "world!"]);
    assert!(block_on(group_2).is_err());
    assert!(!group.update());
    assert!(!group.check_elem_update(&group.data));

    // TODO: Try load value from json
}
