use config_it::CollectPropMeta;

#[derive(CollectPropMeta)]
struct MyStruct {
    #[config_it(min = 3)]
    minimal: i32,
}

#[test]
fn fewew() {
    // println!("{}", hello_world());
}
