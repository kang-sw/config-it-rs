use config_it::ConfigGroupData;
use config_it::Group;
use futures::executor::{self, block_on};
use std::{
    io::Write,
    process::{self, Command, Stdio},
    thread,
};

#[derive(Clone, ConfigGroupData, Default, Debug)]
pub struct MyStruct {
    #[config_it(min = 0, min = 3)]
    minimal: i32,

    #[config_it(default = 2, max = 3, one_of(1, 2, 3, 4, 5))]
    maximum: i32,

    #[config_it(default = "3@", one_of("a"))]
    data: String,

    #[config_it(default = 3112)]
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
fn config_set_valid_operations() {
    let async_op = async {
        let (storage, worker) = config_it::Storage::new();
        thread::spawn(move || block_on(worker));

        let mut group = storage
            .create_group::<MyStruct>(["hello", "world!"])
            .await
            .unwrap();

        assert!(
            storage
                .create_group::<MyStruct>(["hello", "world!"])
                .await
                .is_err(),
            "Assert key duplication handled correctly"
        );

        assert!(!group.update(), "Assert initial empty group update handles 0 correctly");
        assert!(!group.check_elem_update(&group.data));
        assert!(group.maximum == 2);
        assert!(group.minimal == 0);
        assert!(group.median == 3112);
        assert!(group.data == "3@");
        dbg!(group.__body);

        // TODO: Create json value
    };

    block_on(async_op);
}
