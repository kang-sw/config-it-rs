#![allow(unused_imports)]

use config_it::Archive;
use config_it::Group;
use config_it::Template;
use futures::executor::{self, block_on};
use serde_json::json;
use std::time::Duration;
use std::{
    io::Write,
    process::{self, Command, Stdio},
    thread,
};

#[derive(Clone, config_it::Template, Debug)]
pub struct MyStruct {
    #[config_it(min = -35)]
    minimal: i32,

    #[config_it(default = 2, max = 3)]
    maximum: i32,

    #[config_it(default = 24, max = 3, editor=ColorRgba255)]
    maximum_va: usize,

    #[config_it(default = "3@", one_of = ["ab3", "go04"])]
    data: String,

    #[config_it(default = 3112, one_of = [1, 2, 3, 4, 5])]
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

    #[non_config_default_expr = "[1,2,3,4]"]
    this_is_invisible_default: [i32; 4],

    #[non_config_default_expr = r#"Into::into("pewpew")"#]
    this_is_invisible_default2: String,

    #[config]
    my_type: MyType,
}

#[derive(Default, Clone, serde::Serialize, serde::Deserialize, Debug)]
struct MyType {
    a: i32,
    b: i32,
}

#[test]
fn config_set_valid_operations() {
    let async_op = async {
        let storage = config_it::create_storage();

        std::env::set_var("MY_ARRAY_VAR", "14141");
        assert_eq!(std::env::var("MY_ARRAY_VAR").unwrap(), "14141");

        let mut group = storage.create::<MyStruct>(["hello", "world!"]).unwrap();

        assert!(
            storage.create::<MyStruct>(["hello", "world!"]).is_err(),
            "Assert key duplication handled correctly"
        );

        #[cfg(feature = "jsonschema")]
        {
            assert!(group.property_info(&group.maximum).schema.is_some());
            assert!(group.property_info(&group.my_type).schema.is_none());
        }

        let mut brd = group.watch_update();
        assert!(brd.try_recv().is_ok());
        assert!(brd.try_recv().is_err());

        dbg!(module_path!());
        dbg!(MyStruct::template_name());

        assert_eq!(group.maximum, 2, "Default value correctly applied");
        assert_eq!(group.minimal, 0);
        assert_eq!(group.median, 3112);
        assert_eq!(group.noimp, 124);
        assert_eq!(group.noexp, 242);
        assert_eq!(group.my_value, 0);
        assert_eq!(group.array, [1, 2, 3, 4, 5]);
        assert_eq!(group.array_env, 14141);
        assert_eq!(group.data, "3@");
        assert_eq!(group.this_is_invisible_default, [1, 2, 3, 4]);
        assert_eq!(group.this_is_invisible_default2, "pewpew");

        assert!(group.update(), "Initial update always returns true.");
        assert!(!group.update(), "Now dirty flag cleared");
        assert!(group.consume_update(&group.data), "Initial check returns true");
        assert!(!group.consume_update(&group.data), "Now dirty flag is cleared");
        assert!(!group.consume_update(&group.data), "Now dirty flag is cleared");
        assert!(group.consume_update(&group.median), "Now dirty flag is cleared");
        assert!(group.consume_update(&group.noimp), "Now dirty flag is cleared");
        assert!(!group.consume_update(&group.median), "Now dirty flag is cleared");

        dbg!(&*group);

        let json = json!({
            "~hello": {
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

        config_it::archive::with_category_rule(
            config_it::ArchiveCategoryRule::Wrap("C_AT", "GO_RY"),
            || {
                let str = serde_json::to_string_pretty(&arch).unwrap();
                println!("{}", str);
                assert!(serde_json::from_str::<Archive>(&str).is_ok());
            },
        );

        dbg!(&arch);

        assert!(brd.try_recv().is_err());
        storage.import(arch);

        thread::sleep(Duration::from_millis(100));

        assert!(brd.try_recv().is_ok());
        assert!(!group.consume_update(&group.data), "Before 'update()' call, nothing changes.");
        assert!(group.update(), "Config successfully imported.");

        dbg!(&*group);

        let meta = group.property_info(&group.my_value);
        dbg!((meta.name, &*meta));

        assert!(!group.update(), "Re-request handled correctly.");
        assert!(group.consume_update(&group.data), "Updated configs correctly applied.");
        assert!(group.consume_update(&group.maximum));
        assert!(group.consume_update(&group.minimal));
        assert!(group.consume_update(&group.noexp));
        assert!(!group.consume_update(&group.noimp), "No-import property correctly excluded");
        assert!(!group.consume_update(&group.median), "Unspecified update correctly excluded.");

        let dumped = storage.exporter().collect();
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
