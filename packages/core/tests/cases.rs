use config_it::config::storage::GroupFindError;

#[test]
fn concepts() {
    use config_it::Template;

    // Every config template must implement `Clone` trait.
    #[derive(Template, Clone)]
    struct Profile {
        /// Doc comment will be used as description for the property. This will be included in
        /// the config property's metadata.
        #[config]
        pub name: String,

        #[config(max = 250)]
        pub age: u32,

        #[config(default = "unspecified", one_of("left", "right", "up", "down"))]
        pub position: String,
    }

    // Before doing anything with your config template, you should create storage instance.
    // Storage is the primary actor for centralized configuration management.
    let storage = config_it::create_storage();

    // Assume that you have a config file with following content:
    // (Note that all 'group's are prefixed with '~'(this is configurable) to be distinguished
    //  from properties)
    let content = serde_json::json!({
        "~profile": {
            "~scorch": {
                "name": "Scorch",
                "age": 25,
                "position": "left"
            },
            "~john": {
                "name": "John",
                "age": 30,
                "position": "invalid-value-here"
            }
        }
    });

    let archive = serde_json::from_value(content).unwrap();

    // You can import config file into storage.
    storage.import(archive);

    // A `Template` can be instantiated as `Group<T:Template>` type.
    let mut scorch = storage.create::<Profile>(["profile", "scorch"]).unwrap();
    let mut john = storage.create::<Profile>(["profile", "john"]).unwrap();

    // Before calling 'update' method on group, every property remain in default.
    assert_eq!(scorch.name, "");
    assert_eq!(scorch.age, 0);
    assert_eq!(scorch.position, "unspecified");

    // Calling 'update' method will update the property to the value in archive.
    assert!(scorch.update() == true);
    assert!(john.update() == true);

    // You can check dirty flag of individual property.
    assert!(scorch.consume_update(&scorch.name) == true);
    assert!(scorch.consume_update(&scorch.name) == false);

    // Now the property values are updated.
    assert_eq!(scorch.name, "Scorch");
    assert_eq!(scorch.age, 25);
    assert_eq!(scorch.position, "left");
    assert_eq!(john.name, "John");
    assert_eq!(john.age, 30);
    assert_eq!(john.position, "unspecified", "invalid value is ignored");
}

#[test]
fn serde_struct() {
    #[derive(config_it::Template, Clone)]
    struct Outer {
        #[config(default_expr = "Inner{name:Default::default(),age:0}")]
        inner: Inner,
    }

    #[derive(serde::Serialize, serde::Deserialize, Clone)]
    struct Inner {
        name: String,
        age: u32,
    }

    let storage = config_it::create_storage();
    let mut outer = storage.create::<Outer>(["outer"]).unwrap();
    outer.inner.name = "John".to_owned();
    outer.inner.age = 30;

    outer.commit_elem(&outer.inner, false);
    let archive = storage.exporter().collect();

    let dump = serde_json::to_string(&archive).unwrap();
    assert_eq!(dump, r#"{"~outer":{"inner":{"age":30,"name":"John"}}}"#);
}

#[test]
fn find_or() {
    #[derive(config_it::Template, Clone)]
    struct TemplateA {
        #[config(default = "unspecified")]
        name: String,
    }

    #[derive(config_it::Template, Clone)]
    struct TemplateB {
        #[config(default = "unspecified")]
        name_b: String,
    }

    let storage = config_it::create_storage();
    let path = || ["a"];
    assert!(matches!(storage.find::<TemplateA>(path()), Err(GroupFindError::PathNotFound)));

    let mut a = storage.create::<TemplateA>(path()).unwrap();
    assert!(a.update() == true);
    assert!(a.consume_update(&a.name) == true);
    assert!(a.name == "unspecified");
    a.name.clear();
    a.name.push_str("John");
    a.commit_elem(&a.name, false);

    assert!(storage.create::<TemplateA>(path()).is_err());
    let mut a2 = storage.find_or_create::<TemplateA>(path()).unwrap();
    assert!(a2.update() == true);
    assert!(a2.consume_update(&a2.name) == true);
    assert!(a2.name == "John");

    assert!(matches!(storage.find::<TemplateB>(path()), Err(GroupFindError::MismatchedTypeID)));

    drop(storage);
}
