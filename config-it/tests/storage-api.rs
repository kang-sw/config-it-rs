use config_it::beacon::group::Property;

#[derive(Clone)]
struct MyType {
    _x: i32,
}

impl config_it::Template for MyType {
    fn prop_at_offset__(offset: usize) -> Option<&'static Property> {
        todo!()
    }

    fn props__() -> &'static [Property] {
        todo!()
    }

    fn template_name() -> (&'static str, &'static str) {
        todo!()
    }

    fn default_config() -> Self {
        todo!()
    }

    fn elem_at_mut__(&mut self, index: usize) -> &mut dyn std::any::Any {
        todo!()
    }
}

pub fn pewpew() {
    let st = config_it::Storage::default();
    let g: config_it::Group<MyType> = st.create(["hello"]).unwrap();
    st.find::<MyType>(["hads", "hads"]).ok();
}
