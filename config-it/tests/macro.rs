#![allow(unused_imports)]

use config_it::archive::Archive;
use config_it::ConfigGroupData;
use config_it::Group;
use futures::executor::{self, block_on};
use serde_json::json;
use std::time::Duration;
use std::{
    io::Write,
    process::{self, Command, Stdio},
    thread,
};

#[derive(Clone, ConfigGroupData, Default, Debug)]
pub struct MyStruct {
    #[config_it(min = -35)]
    minimal: i32,

    #[config_it(default = 2, max = 3)]
    maximum: i32,

    #[config_it(default = 24, max = 3)]
    maximum_va: usize,

    #[config_it(default = "3@", one_of("ab3", "go04"))]
    data: String,

    #[config_it(default = 3112, one_of(1, 2, 3, 4, 5))]
    median: i32,

    /// Complicated default value expressions should be wrapped within quotes, and assigned into
    /// `default_expr` attribute.
    #[config_it(default_expr = "[1,2,3,4,5]")]
    array: [i32; 5],

    #[config_it(env = "MY_ARRAY_VAR")]
    array_env: i64,

    #[config_it(default = 124, no_import)]
    noimp: i32,

    #[config_it(default = 242, no_export)]
    noexp: i32,

    #[config_it]
    my_value: i32,

    #[config_it(alias = "mvmv")]
    aliased_to_mvmv: i32,

    #[allow(unused)]
    my_invisible: f32,
}

#[cfg(any())]
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

#[test]
fn config_set_valid_operations() {
    let async_op = async {
        let (storage, worker) = config_it::create_storage();
        thread::spawn(move || block_on(worker));

        std::env::set_var("MY_ARRAY_VAR", "14141");
        assert_eq!(std::env::var("MY_ARRAY_VAR").unwrap(), "14141");

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

        let mut brd = group.watch_update();
        assert!(brd.try_recv().is_err());

        assert_eq!(group.maximum, 2, "Default value correctly applied");
        assert_eq!(group.minimal, 0);
        assert_eq!(group.median, 3112);
        assert_eq!(group.noimp, 124);
        assert_eq!(group.noexp, 242);
        assert_eq!(group.my_value, 0);
        assert_eq!(group.array, [1, 2, 3, 4, 5]);
        assert_eq!(group.array_env, 14141);
        assert_eq!(group.data, "3@");

        assert!(group.update(), "Initial update always returns true.");
        assert!(!group.update(), "Now dirty flag cleared");
        assert!(group.check_elem_update(&group.data), "Initial check returns true");
        assert!(!group.check_elem_update(&group.data), "Now dirty flag is cleared");
        assert!(!group.check_elem_update(&group.data), "Now dirty flag is cleared");
        assert!(group.check_elem_update(&group.median), "Now dirty flag is cleared");
        assert!(group.check_elem_update(&group.noimp), "Now dirty flag is cleared");
        assert!(!group.check_elem_update(&group.median), "Now dirty flag is cleared");

        dbg!(&group.__body);

        let json = json!({
            "hello": {
                "~world!": {
                    "data": "ab3",
                    "maximum": 98,
                    "minimal": -1929,
                    "noimp": 932,
                    "noexp": 884,
                    "mvmv": 415,
                }
            }
        });

        let arch = serde_json::from_str::<Archive>(&json.to_string()).unwrap();

        dbg!(&arch);

        assert!(brd.try_recv().is_err());
        let _ = storage.import(arch, Default::default()).await;

        thread::sleep(Duration::from_millis(100));

        assert!(brd.try_recv().is_ok());
        assert!(!group.check_elem_update(&group.data), "Before 'update()' call, nothing changes.");
        assert!(group.update(), "Config successfully imported.");

        dbg!(&group.__body);

        let meta = group.get_metadata(&group.my_value);
        dbg!((meta.name, &meta.props));

        assert!(!group.update(), "Re-request handled correctly.");
        assert!(group.check_elem_update(&group.data), "Updated configs correctly applied.");
        assert!(group.check_elem_update(&group.maximum));
        assert!(group.check_elem_update(&group.minimal));
        assert!(group.check_elem_update(&group.noexp));
        assert!(!group.check_elem_update(&group.noimp), "No-import property correctly excluded");
        assert!(!group.check_elem_update(&group.median), "Unspecified update correctly excluded.");

        let dumped = storage.export(Default::default()).await.unwrap();
        let dumped = serde_json::to_string_pretty(&dumped).unwrap();
        println!("{}", dumped);

        assert_eq!(group.maximum, 3, "Value validation");
        assert_eq!(group.minimal, -35, "Lower limit");
        assert_eq!(group.median, 3112, "Untouched element exclude");
        assert_eq!(group.noimp, 124, "No-import element exclude");
        assert_eq!(group.aliased_to_mvmv, 415, "Alias");
        assert_eq!(group.noexp, 884, "No-export element include");
        assert_eq!(group.data, "ab3", "String argument update");

        let _: &dyn Send = &group;
    };

    block_on(async_op);
}
