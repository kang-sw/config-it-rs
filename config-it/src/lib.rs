//!
//! A crate for asynchronous centralized configuration management.
//!
//! # Usage
//!
//! You can define 'Template' which defines set of grouped properties,
//!  which should be updated at once. Only `config_it` decorated properties will be counted
//!  as part of given group and will be managed.
//!
//! `Template` must implement `Clone` and `Default` to be treated as valid config
//!  group data.
//!
//! You should create `Storage` to create config group instances(`Group<T:Template>`).
//!  `Storage` is the primary actor for centralized configuration management. Config groups can
//!  be instantiated based on storage instance.
//!
//! # Example usage
//!
//! ```
//! /// This is a 'Template' struct, which is minimal unit of
//! /// instantiation. Put required properties to configure
//! /// your program.
//! ///
//! /// All 'Template' classes must be 'Clone'able, and
//! /// 'Default'able.
//! ///
//! /// (Trying to finding way to remove 'Default' constraint.
//! ///  However, 'Clone' will always be required.)
//! #[derive(config_it::Template, Clone, Default)]
//! struct MyConfig {
//!     /// If you expose any field as config property, the
//!     /// field must be marked with `config_it` attribute.
//!     #[config_it]
//!     string_field: String,
//!
//!     /// You can also specify default value, or min/max
//!     /// constraints for this field.
//!     #[config_it(default = 3, min = 1, max = 5)]
//!     int_field: i32,
//!
//!     /// This field will be aliased as 'alias'.
//!     ///
//!     /// > **Warning** Don't use `~(tilde)` characters in
//!     /// > alias name. In current implementation, `~` is
//!     /// > used to indicate group object in archive
//!     /// > representation during serialization.
//!     #[config_it(alias = "alias")]
//!     non_alias: f32,
//!
//!     /// Only specified set of values are allowed for
//!     /// this field, however, default field can be
//!     /// excluded from this set.
//!     #[config_it(default = "default", one_of("a", "b", "c"))]
//!     one_of_field: String,
//!
//!     /// Any 'serde' compatible type can be used as config field.
//!     #[config_it]
//!     c_string_type: Box<std::ffi::CStr>,
//!
//!     /// This will find value from environment variable
//!     /// `MY_ENV_VAR`. Currently, only values that can be
//!     /// `TryParse`d from `str` are supported.
//!     ///
//!     /// Environment variables are imported when the
//!     /// group is firstly instantiated.
//!     /// i.e. call to `Storage::create_group`
//!     #[config_it(env = "MY_ARRAY_VAR")]
//!     env_var: i64,
//!
//!     /// Complicated default value are represented as expression.
//!     #[config_it(default_expr = "[1,2,3,4,5].into()")]
//!     array_init: Vec<i32>,
//!
//!     /// This field is not part of config_it system.
//!     _not_part_of: (),
//!
//!     /// This field won't be imported or exported from
//!     /// archiving operation
//!     #[config_it(no_import, no_export)]
//!     no_imp_exp: Vec<f64>,
//!
//!     /// `transient` flag is equivalent to `no_import` and
//!     /// `no_export` flags.
//!     #[config_it(transient)]
//!     no_imp_exp_2: Vec<f64>,
//! }
//!
//! // USAGE ///////////////////////////////////////////////////////////////////////////////////////
//!
//! // 1. Storage
//! //
//! // Storage is basic and most important class to drive
//! // the whole config_it system. Before you can use any
//! // of the features, you must create a storage instance.
//! let (storage, driver_task) = config_it::create_storage();
//!
//! // `[config_it::create_storage]` returns a tuple of
//! // `(Storage, Task)`. `Storage` is the handle to the
//! // storage, and `Task` is the driver task that must
//! // be spawned to drive the storage operations(actor).
//! // You can spawn the task using any async runtime.
//! //
//! // Basically, config_it is designed to be used with
//! // async runtime, we're run this example under async
//! // environment.
//! let mut local = futures::executor::LocalPool::new();
//! let spawn = local.spawner();
//!
//! // Storage driver task must be running somewhere.
//! use futures::task::SpawnExt;
//! spawn.spawn(driver_task).unwrap();
//!
//! // before starting this, let's set environment variable to see if it works.
//! std::env::set_var("MY_ARRAY_VAR", "123");
//!
//! // Let's get into async
//! local.run_until(async {
//!     // 2. Groups and Templates
//!     //
//!     // A group is an instance of a template. You can
//!     // create multiple groups from a single template.
//!     // Each group has its own set of properties, and
//!     // can be configured independently.
//!     //
//!     // When instantiating a group, you must provide a
//!     // path to the group. Path is a list of short string
//!     // tokens, which is used to identify the group. You
//!     // can use any string as path, but it's recommended
//!     // to use a short string, which does not contain any
//!     // special characters. (Since it usually encoded as a
//!     //  key of a key-value store of some kind of data
//!     //  serialization formats, such as JSON, YAML, etc.)
//!     let path = &["path", "to", "my", "group"];
//!     let mut group = storage.create_group::<MyConfig>(path).await.unwrap();
//!
//!     // Note, duplicated path name is not allowed.
//!     assert!(storage.create_group::<MyConfig>(path).await.is_err());
//!
//!     // `update()` call to group, will check for asynchronously
//!     // queued updates, and apply changes to the group instance.
//!     // Since this is the first call to update,
//!     //
//!     // You can understand `update()` as clearing dirty flag.
//!     assert!(group.update() == true);
//!
//!     // After `update()`, as long as there's no new update,
//!     // `update()` will return false.
//!     assert!(group.update() == false);
//!
//!     // Every individual properties has their own dirty flag.
//!     assert!(true == group.check_elem_update(&group.array_init));
//!     assert!(true == group.check_elem_update(&group.c_string_type));
//!     assert!(true == group.check_elem_update(&group.env_var));
//!     assert!(true == group.check_elem_update(&group.no_imp_exp));
//!     assert!(true == group.check_elem_update(&group.no_imp_exp_2));
//!     assert!(true == group.check_elem_update(&group.non_alias));
//!     assert!(true == group.check_elem_update(&group.int_field));
//!     assert!(true == group.check_elem_update(&group.one_of_field));
//!     assert!(true == group.check_elem_update(&group.string_field));
//!
//!     assert!(false == group.check_elem_update(&group.array_init));
//!     assert!(false == group.check_elem_update(&group.c_string_type));
//!     assert!(false == group.check_elem_update(&group.env_var));
//!     assert!(false == group.check_elem_update(&group.no_imp_exp));
//!     assert!(false == group.check_elem_update(&group.no_imp_exp_2));
//!     assert!(false == group.check_elem_update(&group.int_field));
//!     assert!(false == group.check_elem_update(&group.non_alias));
//!     assert!(false == group.check_elem_update(&group.one_of_field));
//!     assert!(false == group.check_elem_update(&group.string_field));
//!
//!     // Any field that wasn't marked as 'config_it' attribute will not be part of
//!     // config_it system.
//!
//!     // // Invoking next line will panic:
//!     // group.check_elem_update(&group.nothing_here);
//!
//!     // 3. Properties
//!     //
//!     // You can access each field of the group instance in common deref manner.
//!     assert!(group.string_field == "");
//!     assert!(group.array_init == &[1, 2, 3, 4, 5]);
//!     assert!(group.env_var == 123);
//!
//!     // 4. Importing and Exporting
//!     //
//!     // You can export the whole storage using 'Export' method.
//!     // (currently, there is no way to export a specific group
//!     //  instance. To separate groups into different archiving
//!     //  categories, you can use multiple storage instances)
//!     let archive = storage.export(Default::default()).await.unwrap();
//!
//!     // `config_it::Archive` implements `serde::Serialize` and
//!     // `serde::Deserialize`. You can use it to serialize/
//!     //  deserialize the whole storage.
//!     let yaml = serde_yaml::to_string(&archive).unwrap();
//!     let json = serde_json::to_string_pretty(&archive).unwrap();
//!     // println!("{}", yaml);
//!     // OUTPUT:
//!     //
//!     //  ~path: # all path tokens of group hierarchy are prefixed with '~'
//!     //    ~to: # (in near future, this will be made customizable)
//!     //      ~my:
//!     //        ~group:
//!     //          alias: 0.0
//!     //          array_init:
//!     //          - 1
//!     //          - 2
//!     //          - 3
//!     //          - 4
//!     //          - 5
//!     //          c_string_type: []
//!     //          env_var: 0
//!     //          int_field: 3
//!     //          one_of_field: default
//!     //          string_field: ''
//!     //
//!
//!     // println!("{}", json);
//!     // OUTPUT:
//!     // {
//!     //   "~path": {
//!     //     "~to": {
//!     //       "~my": {
//!     //         "~group": {
//!     //           "alias": 0.0,
//!     //           "array_init": [
//!     //             1,
//!     //             2,
//!     //             3,
//!     //             4,
//!     //             5
//!     //           ],
//!     //           "c_string_type": [],
//!     //           "env_var": 0,
//!     //           "int_field": 3,
//!     //           "one_of_field": "default",
//!     //           "string_field": ""
//!     //         }
//!     //       }
//!     //     }
//!     //   }
//!     // }
//!
//!     // Importing is similar to exporting. You can import a
//!     // whole storage from an archive. For this, you should
//!     // create a new archive. Archive can be created using serde either.
//!     use indoc::indoc;
//!     let yaml = indoc!(r##"
//!         ~path:
//!            ~to:
//!                ~my:
//!                    ~group:
//!                        alias: 3.14
//!                        array_init:
//!                        - 1
//!                        - 145
//!                        int_field: 3 # If there's no change, it won't be updated. This behavior can be overridden by import options.
//!                        env_var: 59
//!                        one_of_field: "hello" # This is not in the 'one_of' list...
//!         "##);
//!
//!     println!("{}", yaml);
//!
//!     let archive: config_it::Archive = serde_yaml::from_str(yaml).unwrap();
//!     storage.import(archive, Default::default()).await.unwrap();
//!     storage.fence().await; // Since import operation is asynchronous, you must fence
//!                             // to make sure all changes are applied.
//!
//!     // Now, let's check if the changes are applied.
//!     assert!(group.update() == true);
//!
//!     // Data update is regardless of the individual properties' dirty flag control.
//!     // Data is modified only when `group.update()` is called.
//!     assert!(group.non_alias == 3.14); // That was aliased property
//!     assert!(group.array_init == [1, 145]);
//!     assert!(group.env_var == 59);
//!     assert!(group.int_field == 3); // No change
//!     assert!(group.one_of_field == "default"); // Not in the 'one_of' list. no change.
//!
//!     // Only updated properties' dirty flag will be set.
//!     assert!(true == group.check_elem_update(&group.non_alias));
//!     assert!(true == group.check_elem_update(&group.array_init));
//!     assert!(true == group.check_elem_update(&group.env_var));
//!
//!     // Since this property had no change, dirty flag was not set.
//!     assert!(false == group.check_elem_update(&group.int_field));
//!
//!     // Since this property was not in the 'one_of' list, it was ignored.
//!     assert!(false == group.check_elem_update(&group.one_of_field));
//!
//!     // These were simply not in the list.
//!     assert!(false == group.check_elem_update(&group.c_string_type));
//!     assert!(false == group.check_elem_update(&group.no_imp_exp));
//!     assert!(false == group.check_elem_update(&group.no_imp_exp_2));
//!     assert!(false == group.check_elem_update(&group.string_field));
//!
//!     // 5. Other features
//!
//!     // 5.1. Watch update
//!     // When group is possible to updated, you can be notified
//!     // through asynchronous channel. This is useful when you
//!     // want to immediately response to any configuration updates.
//!     let mut monitor = group.watch_update();
//!     assert!(false == monitor.try_recv().is_ok());
//!
//!     let archive: config_it::Archive = serde_yaml::from_str(yaml).unwrap();
//!     storage
//!         .import(
//!             archive,
//!             config_it::ImportOptions {
//!                 apply_as_patch: false, // This will force all properties to be updated.
//!                 ..Default::default()
//!             },
//!         )
//!         .await
//!         .unwrap();
//!
//!     assert!(true == monitor.recv().await.is_ok());
//!     assert!(group.update());
//!
//!     // 5.2. Commit
//!     // Any property value changes on group is usually local,
//!     // however, if you want to
//!     // archive those changes, you can commit it.
//!     group.int_field = 15111; // This does not affected by
//!                                 // constraint and visible from export,
//!                                 // however, in next time you import
//!                                 // it from exported archive,
//!                                 // its constraint will be applied.
//!
//!     // If you set the second boolean parameter 'true', it will
//!     // be notified to 'monitor'
//!     group.commit_elem(&group.int_field, false);
//!     let archive = storage.export(Default::default()).await.unwrap();
//!
//!     assert!(
//!         archive.find_path(path.iter().map(|x| *x)).unwrap().values["int_field"]
//!             .as_i64()
//!             .unwrap()
//!             == 15111
//!     );
//!
//!     // As the maximum value of 'int_field' is 5, in next import, it will be 5.
//!     storage
//!         .import(
//!             archive,
//!             config_it::ImportOptions {
//!                 // Since we create patch from archive content ...
//!                 // Need to forcibly invalidate all
//!                 apply_as_patch: false,
//!                 ..Default::default()
//!             },
//!         )
//!         .await
//!         .unwrap();
//!     storage.fence().await;
//!
//!     assert!(group.update());
//!     assert!(group.int_field == 5);
//!
//!     // 5.3. Monitor
//!     //
//!     // All events to update storage can be monitored though
//!     // this channel.
//!     //
//!     // As this is advanced topic, and currently its design
//!     // is not finalized, just give a look for fun and don't
//!     // use it in production.
//!     let _ch = storage.monitor_open_replication_channel().await;
//! });
//! ```
//!
pub mod archive;
pub mod config;
pub mod core;
pub mod entity;
pub mod storage;

pub use compact_str::CompactString;

pub use archive::Archive;
pub use archive::CategoryRule as ArchiveCategoryRule;
pub use config::Group;
pub use config::Template;
pub use storage::ExportOptions;
pub use storage::ImportOptions;
pub use storage::Storage;

pub use storage::create as create_storage;

/// Required by `config_it::Template` macro.
pub use lazy_static::lazy_static;

/// Primary macro for defining configuration group template.
pub use macros::Template;
