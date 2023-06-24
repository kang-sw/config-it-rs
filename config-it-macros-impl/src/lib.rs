mod utils;

#[test]
fn test_macro() {
    use syn::parse2;

    use crate::utils::parsing::decompose_input;

    let raw = r###"
        #[derive(Debug, config_it::Template, Clone, Serialize)]
        pub struct MyStruct<T, Y> {
          #[config_it(default=34, min=0, max=154,  env="MY_ENV_VAR_NAME", no_import, no_export, hidden)]
          my_var : i32,

          pub my_var_2 : f32,

          /// My elels dsa 1
          /// My elels dsa 2
          /// My elels dsa 3
          /// My elels dsa 4
          #[config_it()]
          pub my_var_emp : f32,

          #[config_it(no_import, access(guest, user))]
          pub my_var_4 : f32,

          ///
          /// Hello, world!
          ///
          #[config_it(one_of(1,2,3,4))]
          pub my_var_3 : f64
        }
    "###;

    let d = parse2::<syn::DeriveInput>(raw.parse().unwrap()).unwrap();
    for _attr in d.attrs.iter() {
        println!("OutermostAttr: {:#?}", _attr.tokens)
    }

    // println!("{}", test_input(raw.parse().unwrap()).to_string());
    let _r = decompose_input(parse2(raw.parse().unwrap()).unwrap()).unwrap();
    // println!("{}", generate(r).unwrap());
}
