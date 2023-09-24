use config_it::{commit_elem, consume_update, mark_dirty, Storage};
use futures::executor::block_on;

/// This is a 'Template' struct, which is minimal unit of
/// instantiation. Put required properties to configure
/// your program.
///
/// All 'Template' classes must be 'Clone'able, and
/// 'Default'able.
///
/// (Trying to finding way to remove 'Default' constraint.
///  However, 'Clone' will always be required.)
#[derive(config_it::Template, Clone)]
struct MyConfig {
    /// If you expose any field as config property, the
    /// field must be marked with `config_it` attribute.
    #[config_it]
    string_field: String,

    /// You can also specify default value, or min/max
    /// constraints for this field.
    #[config_it(default = 3, min = 1, max = 5)]
    int_field: i32,

    /// This field will be aliased as 'alias'.
    ///
    /// > **Warning** Don't use `~(tilde)` characters in
    /// > alias name. In current implementation, `~` is
    /// > used to indicate group object in archive
    /// > representation during serialization.
    #[config(alias = "alias")]
    non_alias: f32,

    /// Only specified set of values are allowed for
    /// this field, however, default field can be
    /// excluded from this set.
    #[config_it(default = "default", one_of = ["a", "b", "c"])]
    one_of_field: String,

    /// Any 'serde' compatible type can be used as config field.
    #[config_it]
    c_string_type: Box<std::ffi::CStr>,

    /// This will find value from environment variable
    /// `MY_ENV_VAR`. Currently, only values that can be
    /// `TryParse`d from `str` are supported.
    ///
    /// Environment variables are imported when the
    /// group is firstly instantiated.
    /// i.e. call to `Storage::create_group`
    #[config_it(env = "MY_ARRAY_VAR")]
    env_var: i64,

    /// Complicated default value are represented as expression.
    #[config_it(default = [1,2,3,4,5])]
    array_init: Vec<i32>,

    /// This field is not part of config_it system.
    _not_part_of: (),

    /// This field won't be imported or exported from
    /// archiving operation
    #[config_it(no_import, no_export)]
    no_imp_exp: Vec<f64>,

    /// `transient` flag is equivalent to `no_import` and
    /// `no_export` flags.
    #[config_it(transient)]
    no_imp_exp_2: Vec<f64>,

    /// Alternative attribute is allowed.
    #[config]
    another_attr: i32,

    /// If any non-default-able but excluded field exists, you can provide
    /// your own default value to make this template default-constructible.
    #[non_config_default_expr = "std::num::NonZeroUsize::new(1).unwrap()"]
    _nonzero_var: std::num::NonZeroUsize,
}

#[test]
fn test_use_case() {
    // USAGE ///////////////////////////////////////////////////////////////////////////////////////

    // 1. Storage
    //
    // Storage is basic and most important class to drive
    // the whole config_it system. Before you can use any
    // of the features, you must create a storage instance.
    let storage = config_it::Storage::default();

    // `[config_it::create_storage]` returns a tuple of
    // `(Storage, Task)`. `Storage` is the handle to the
    // storage, and `Task` is the driver task that must
    // be spawned to drive the storage operations(actor).
    // You can spawn the task using any async runtime.
    //
    // Basically, config_it is designed to be used with
    // async runtime, we're run this example under async
    // environment.
    let mut local = futures::executor::LocalPool::new();

    // before starting this, let's set environment variable to see if it works.
    std::env::set_var("MY_ARRAY_VAR", "123");

    // Let's get into async
    local.run_until(async {
        // 2. Groups and Templates
        //
        // A group is an instance of a template. You can
        // create multiple groups from a single template.
        // Each group has its own set of properties, and
        // can be configured independently.
        //
        // When instantiating a group, you must provide a
        // path to the group. Path is a list of short string
        // tokens, which is used to identify the group. You
        // can use any string as path, but it's recommended
        // to use a short string, which does not contain any
        // special characters. (Since it usually encoded as a
        //  key of a key-value store of some kind of data
        //  serialization formats, such as JSON, YAML, etc.)
        let path = &["path", "to", "my", "group"];
        let mut group = storage.create::<MyConfig>(path).unwrap();

        // Note, duplicated path name is not allowed.
        assert!(storage.create::<MyConfig>(path).is_err());

        // `update()` call to group, will check for asynchronously
        // queued updates, and apply changes to the group instance.
        // Since this is the first call to update,
        //
        // You can understand `update()` as clearing dirty flag.
        assert!(group.update());

        // After `update()`, as long as there's no new update,
        // `update()` will return false.
        assert!(!group.update());

        // Every individual properties has their own dirty flag.
        assert!(group.consume_update(&group.array_init));
        assert!(group.consume_update(&group.c_string_type));
        assert!(group.consume_update(&group.env_var));
        assert!(group.consume_update(&group.no_imp_exp));
        assert!(group.consume_update(&group.no_imp_exp_2));
        assert!(group.consume_update(&group.non_alias));
        assert!(group.consume_update(&group.int_field));
        assert!(group.consume_update(&group.one_of_field));
        assert!(group.consume_update(&group.string_field));

        assert!(!group.consume_update(&group.array_init));
        assert!(!group.consume_update(&group.c_string_type));
        assert!(!group.consume_update(&group.env_var));
        assert!(!group.consume_update(&group.no_imp_exp));
        assert!(!group.consume_update(&group.no_imp_exp_2));
        assert!(!group.consume_update(&group.int_field));
        assert!(!group.consume_update(&group.non_alias));
        assert!(!group.consume_update(&group.one_of_field));
        assert!(!group.consume_update(&group.string_field));

        {
            mark_dirty!(group, c_string_type, one_of_field, env_var);
            assert!(group.consume_update(&group.c_string_type));
            assert!(group.consume_update(&group.one_of_field));
            assert!(group.consume_update(&group.env_var));

            mark_dirty!(group, c_string_type, one_of_field, env_var);
            assert_eq!(
                consume_update!(group, [c_string_type, one_of_field, env_var]),
                [true, true, true],
            );
            assert_eq!(
                consume_update!(group, [c_string_type, one_of_field, env_var]),
                [false, false, false],
            );

            assert!(!group.consume_update(&group.c_string_type));
            assert!(!group.consume_update(&group.one_of_field));
            assert!(!group.consume_update(&group.env_var));

            mark_dirty!(group, c_string_type, one_of_field, env_var);
            assert!(consume_update!(group, c_string_type, one_of_field, env_var));

            assert!(!group.consume_update(&group.c_string_type));
            assert!(!group.consume_update(&group.one_of_field));
            assert!(!group.consume_update(&group.env_var));

            let mut watchdog = group.watch_update();
            assert!(watchdog.try_recv().is_ok());
            assert!(watchdog.try_recv().is_err());
            commit_elem!(group, notify(c_string_type));
            assert!(watchdog.recv().await.is_ok());

            assert!(group.update());
            assert!(group.consume_update(&group.c_string_type));

            mark_dirty!(group, c_string_type, one_of_field, env_var);
            let up = consume_update!(group, ((c_string_type, one_of_field, env_var)));
            assert!(up.c_string_type && up.env_var && up.one_of_field);
        }

        // Any field that wasn't marked as 'config_it' attribute will not be part of
        // config_it system.

        // // Invoking next line will panic:
        // group.clear_flag(&group.nothing_here);

        // 3. Properties
        //
        // You can access each field of the group instance in common deref manner.
        assert!(group.string_field.is_empty());
        assert!(group.array_init == &[1, 2, 3, 4, 5]);
        assert!(group.env_var == 123);

        // 4. Importing and Exporting
        //
        // You can export the whole storage using 'Export' method.
        // (currently, there is no way to export a specific group
        //  instance. To separate groups into different archiving
        //  categories, you can use multiple storage instances)
        let archive = storage.exporter().collect();

        // `config_it::Archive` implements `serde::Serialize` and
        // `serde::Deserialize`. You can use it to serialize/
        //  deserialize the whole storage.
        let _yaml = serde_yaml::to_string(&archive).unwrap();
        let _json = serde_json::to_string_pretty(&archive).unwrap();
        // println!("{}", yaml);
        // OUTPUT:
        //
        //  ~path: # all path tokens of group hierarchy are prefixed with '~'
        //    ~to: # (in near future, this will be made customizable)
        //      ~my:
        //        ~group:
        //          alias: 0.0
        //          array_init:
        //          - 1
        //          - 2
        //          - 3
        //          - 4
        //          - 5
        //          c_string_type: []
        //          env_var: 0
        //          int_field: 3
        //          one_of_field: default
        //          string_field: ''
        //

        // println!("{}", json);
        // OUTPUT:
        // {
        //   "~path": {
        //     "~to": {
        //       "~my": {
        //         "~group": {
        //           "alias": 0.0,
        //           "array_init": [
        //             1,
        //             2,
        //             3,
        //             4,
        //             5
        //           ],
        //           "c_string_type": [],
        //           "env_var": 0,
        //           "int_field": 3,
        //           "one_of_field": "default",
        //           "string_field": ""
        //         }
        //       }
        //     }
        //   }
        // }

        // Importing is similar to exporting. You can import a
        // whole storage from an archive. For this, you should
        // create a new archive. Archive can be created using serde either.
        let yaml = r#"
~path:
    ~to:
        ~my:
            ~group:
                alias: 3.14
                array_init:
                - 1
                - 145
                int_field: 3 # If there's no change, it won't be updated.
                                # This behavior can be overridden by import options.
                env_var: 59
                one_of_field: "hello" # This is not in the 'one_of' list..."#;

        let archive: config_it::Archive = serde_yaml::from_str(yaml).unwrap();
        storage.import(archive);

        // Now, let's check if the changes are applied.
        assert!(group.update());

        // Data update is regardless of the individual properties' dirty flag control.
        // Data is modified only when `group.update()` is called.
        assert_eq!(group.non_alias, 3.14); // That was aliased property
        assert_eq!(group.array_init, [1, 145]);
        assert_eq!(group.env_var, 59);
        assert_eq!(group.int_field, 3); // No change
        assert_eq!(group.one_of_field, "default"); // Not in the 'one_of' list. no change.

        // Only updated properties' dirty flag will be set.
        assert!(group.consume_update(&group.non_alias));
        assert!(group.consume_update(&group.array_init));
        assert!(group.consume_update(&group.env_var));

        // Since this property had no change, dirty flag was not set.
        assert!(!group.consume_update(&group.int_field));

        // Since this property was not in the 'one_of' list, it was ignored.
        assert!(!group.consume_update(&group.one_of_field));

        // These were simply not in the list.
        assert!(!group.consume_update(&group.c_string_type));
        assert!(!group.consume_update(&group.no_imp_exp));
        assert!(!group.consume_update(&group.no_imp_exp_2));
        assert!(!group.consume_update(&group.string_field));

        // 5. Other features

        // 5.1. Watch update
        // When group is possible to updated, you can be notified
        // through asynchronous channel. This is useful when you
        // want to immediately response to any configuration updates.
        let mut monitor = group.watch_update();
        assert!(monitor.try_recv().is_ok());
        assert!(monitor.try_recv().is_err());

        let archive: config_it::Archive = serde_yaml::from_str(yaml).unwrap();
        storage.import(archive).apply_as_patch(false);

        assert!(monitor.recv().await.is_ok());
        assert!(group.update());

        // 5.2. Commit
        // Any property value changes on group is usually local,
        // however, if you want to
        // archive those changes, you can commit it.
        group.int_field = 15111; // This does not affected by
                                 // constraint and visible from export,
                                 // however, in next time you import
                                 // it from exported archive,
                                 // its constraint will be applied.

        // If you set the second boolean parameter 'true', it will
        // be notified to 'monitor'
        group.commit_elem(&group.int_field, false);
        let archive = storage.exporter().collect();

        println!("pretty_json: {}", serde_yaml::to_string(&archive).unwrap());

        assert!(
            archive
                .find_path(path.iter().copied())
                .unwrap()
                .get_value("int_field")
                .unwrap()
                .as_i64()
                .unwrap()
                == 15111
        );

        // As the maximum value of 'int_field' is 5, in next import, it will be 5.
        storage.import(archive).apply_as_patch(false);

        assert!(group.update());
        assert!(group.int_field == 5);
    });
}

#[test]
fn test_multithread() {
    const N_THREAD: usize = 256;

    let (tx_ready, rx_ready) = std::sync::mpsc::channel();
    let (tx_exit, rx_exit) = tokio::sync::watch::channel(false);
    let mut handles = Vec::with_capacity(N_THREAD);

    let storage = config_it::create_storage();

    for _ in 0..N_THREAD {
        let tx_noti = tx_ready.clone();
        let mut rx_exit = rx_exit.clone();
        let storage: Storage = storage.clone();
        handles.push(std::thread::spawn(move || {
            let path = ["path", "to", "my", "storage"];
            let elem = storage.find_or_create::<MyConfig>(path);
            let elem = elem.expect("must not fail under this condition!");

            tx_noti.send(()).ok();
            block_on(rx_exit.changed()).expect("may not fail");

            assert!(*rx_exit.borrow_and_update());
            drop(elem);
        }))
    }

    for _ in 0..N_THREAD {
        rx_ready.recv().ok();
    }

    tx_exit.send(true).ok();
}
