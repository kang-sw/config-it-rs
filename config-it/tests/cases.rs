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
    let (storage, runner) = config_it::create_storage();

    // To run the storage, you should spawn a task with runner (the second return value)
    std::thread::spawn(move || futures::executor::block_on(runner));

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

    // It is recommended to manipulate config group within async context.
    futures::executor::block_on(async {
        // You can import config file into storage.
        storage.import(archive, Default::default()).await.unwrap();

        // As the import operation simply transmits request to the actor, you should wait
        // for the actual import job to be done.
        storage.fence().await;

        // A `Template` can be instantiated as `Group<T:Template>` type.
        let mut scorch = storage
            .create_group::<Profile>(["profile", "scorch"])
            .await
            .unwrap();
        let mut john = storage
            .create_group::<Profile>(["profile", "john"])
            .await
            .unwrap();

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

        storage.close().unwrap();
    });
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

    let (storage, runner) = config_it::create_storage();
    let task = async {
        let mut outer = storage.create_group::<Outer>(["outer"]).await.unwrap();
        outer.inner.name = "John".to_owned();
        outer.inner.age = 30;

        outer.commit_elem(&outer.inner, false);
        let archive = storage.export(Default::default()).await.unwrap();

        let dump = serde_json::to_string(&archive).unwrap();
        assert_eq!(dump, r#"{"~outer":{"inner":{"age":30,"name":"John"}}}"#);

        storage.close().unwrap();
    };

    futures::executor::block_on(async {
        futures::join!(runner, task);
    });
}
