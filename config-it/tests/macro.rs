use config_it::lazy_static;
use config_it::ConfigGroupData;
use std::{
    io::Write,
    process::{self, Command, Stdio},
};

#[derive(ConfigGroupData, Clone)]
pub struct MyStruct<T> {
    #[config_it(min = 3)]
    minimal: i32,

    my_arg: T,
}

impl<T> ConfigGroupData for MyStruct<T> {
    fn prop_desc_table__() -> &'static HashMap<usize, PropData> {
        lazy_static! {
            static ref TABLE : Arc < HashMap < usize, PropData >> =
            {
                let mut s = HashMap :: new() ;
                {
                    type Type = i32 ; let offset = unsafe
                    {
                        let owner = 0 as * const MyStruct ; & (* owner)."minimal" as
                        * const _ as * const u8 as usize ;
                    } ; let identifier = "minimal" ; let varname = "minimal" ;
                    let doc_string = "" ; let index = 0i32 ; let init =
                    MetadataValInit :: < Type >
                    {
                        fn_validate : | _, _ | -> Option < bool > { Some(true) },
                        v_default : default_value, v_one_of : [], v_max : 3, v_min
                        :,
                    } ; let props = MetadataProps
                    {
                        description : doc_string, varname, disable_import : false,
                        disable_export : false, hidden : false,
                    } ; let meta = Metadata ::
                    create_for_base_type(identifier, init, props) ; let
                    prop_data = PropData
                    {
                        index, type_id : TypeId :: of :: < Type > (), meta : Arc ::
                        new(meta),
                    } ; s.insert(offset, prop_data) ;
                } Arc :: new(s)
            } ;
        }
        &*TABLE
    }

    fn elem_at_mut__(&mut self, index: usize) -> &mut dyn std::any::Any {
        todo!()
    }
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
