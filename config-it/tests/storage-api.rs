#[derive(Clone)]
struct MyType {
    _x: i32,
}

impl config_it::Template for MyType {
    fn prop_desc_table__() -> &'static std::collections::HashMap<usize, config_it::config::PropData>
    {
        todo!()
    }

    fn template_name() -> (&'static str, &'static str) {
        todo!()
    }

    fn default_config() -> Self {
        todo!()
    }

    fn fill_default(&mut self) {
        todo!()
    }

    fn elem_at_mut__(&mut self, index: usize) -> &mut dyn std::any::Any {
        todo!()
    }
}

pub fn pewpew() {
    let st = config_it::Storage::default();
    let mut g: config_it::Group<MyType> = st.create(["hello"]).unwrap();
}
