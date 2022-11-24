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

    #[config_it(one_of("a"))]
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

impl config_it::ConfigGroupData for MyStruct {
    fn prop_desc_table__() -> &'static std::collections::HashMap<usize, config_it::config::PropData>
    {
        use config_it::lazy_static;
        lazy_static! {
            static ref TABLE: std::sync::Arc<std::collections::HashMap<usize, config_it::config::PropData>> = {
                let mut s = std::collections::HashMap::new();
                {
                    type Type = i32;
                    let offset = unsafe {
                        let owner = 0 as *const MyStruct;
                        &(*owner).minimal as *const _ as *const u8 as usize
                    };
                    let identifier = "minimal";
                    let varname = "minimal";
                    let doc_string = "";
                    let index = 0usize as usize;
                    let init = config_it::entity::MetadataValInit::<Type> {
                        fn_validate: |_, _| -> Option<bool> { Some(true) },
                        v_default: Default::default(),
                        v_one_of: [].into(),
                        v_max: Some(3),
                        v_min: None,
                    };
                    let props = config_it::entity::MetadataProps {
                        description: doc_string,
                        varname,
                        disable_import: false,
                        disable_export: false,
                        hidden: false,
                    };
                    let meta =
                        config_it::entity::Metadata::create_for_base_type(identifier, init, props);
                    let prop_data = config_it::config::PropData {
                        index,
                        type_id: std::any::TypeId::of::<Type>(),
                        meta: std::sync::Arc::new(meta),
                    };
                    s.insert(offset, prop_data);
                }
                {
                    type Type = i32;
                    let offset = unsafe {
                        let owner = 0 as *const MyStruct;
                        &(*owner).maximum as *const _ as *const u8 as usize
                    };
                    let identifier = "maximum";
                    let varname = "maximum";
                    let doc_string = "";
                    let index = 1usize as usize;
                    let init = config_it::entity::MetadataValInit::<Type> {
                        fn_validate: |_, _| -> Option<bool> { Some(true) },
                        v_default: Default::default(),
                        v_one_of: [1.into(), 2.into(), 3.into(), 4.into(), 5.into()].into(),
                        v_max: None,
                        v_min: Some(3),
                    };
                    let props = config_it::entity::MetadataProps {
                        description: doc_string,
                        varname,
                        disable_import: false,
                        disable_export: false,
                        hidden: false,
                    };
                    let meta =
                        config_it::entity::Metadata::create_for_base_type(identifier, init, props);
                    let prop_data = config_it::config::PropData {
                        index,
                        type_id: std::any::TypeId::of::<Type>(),
                        meta: std::sync::Arc::new(meta),
                    };
                    s.insert(offset, prop_data);
                }
                {
                    type Type = String;
                    let offset = unsafe {
                        let owner = 0 as *const MyStruct;
                        &(*owner).data as *const _ as *const u8 as usize
                    };
                    let identifier = "data";
                    let varname = "data";
                    let doc_string = "";
                    let index = 2usize as usize;
                    let init = config_it::entity::MetadataValInit::<Type> {
                        fn_validate: |_, _| -> Option<bool> { Some(true) },
                        v_default: Default::default(),
                        v_one_of: ["a".into()].into(),
                        v_max: None,
                        v_min: None,
                    };
                    let props = config_it::entity::MetadataProps {
                        description: doc_string,
                        varname,
                        disable_import: false,
                        disable_export: false,
                        hidden: false,
                    };
                    let meta =
                        config_it::entity::Metadata::create_for_base_type(identifier, init, props);
                    let prop_data = config_it::config::PropData {
                        index,
                        type_id: std::any::TypeId::of::<Type>(),
                        meta: std::sync::Arc::new(meta),
                    };
                    s.insert(offset, prop_data);
                }
                {
                    type Type = i32;
                    let offset = unsafe {
                        let owner = 0 as *const MyStruct;
                        &(*owner).median as *const _ as *const u8 as usize
                    };
                    let identifier = "median";
                    let varname = "median";
                    let doc_string = "";
                    let index = 3usize as usize;
                    let init = config_it::entity::MetadataValInit::<Type> {
                        fn_validate: |_, _| -> Option<bool> { Some(true) },
                        v_default: Default::default(),
                        v_one_of: [].into(),
                        v_max: None,
                        v_min: None,
                    };
                    let props = config_it::entity::MetadataProps {
                        description: doc_string,
                        varname,
                        disable_import: false,
                        disable_export: false,
                        hidden: false,
                    };
                    let meta =
                        config_it::entity::Metadata::create_for_base_type(identifier, init, props);
                    let prop_data = config_it::config::PropData {
                        index,
                        type_id: std::any::TypeId::of::<Type>(),
                        meta: std::sync::Arc::new(meta),
                    };
                    s.insert(offset, prop_data);
                }
                std::sync::Arc::new(s)
            };
        }
        &*TABLE
    }
    fn elem_at_mut__(&mut self, index: usize) -> &mut dyn std::any::Any {
        match index {
            1usize => &mut self.minimal,
            2usize => &mut self.maximum,
            3usize => &mut self.data,
            4usize => &mut self.median,
            _ => unreachable!(),
        }
    }
}
