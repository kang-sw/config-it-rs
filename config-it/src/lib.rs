//!
//! A crate for asynchronous centralized configuration management.
//!
//! # Usage
//!
//! You can define 'ConfigGroupData' which defines set of grouped properties,
//!  which should be updated at once. Only `config_it` decorated properties will be counted
//!  as part of given group and will be managed.
//!
//! `ConfigGroupData` must implement `Clone` and `Default` to be treated as valid config
//!  group data.
//!
//! You should create `Storage` to create config group instances(`Group<T:ConfigGroupData>`).
//!  `Storage` is the primary actor for centralized configuration management. Config groups can
//!  be instantiated based on storage instance.
//!
//! # Example usage
//!
//! ```
//! use futures::executor::{self, block_on};
//!
//! ///
//! /// Define set of properties using derive macro
//! ///
//! /// `Debug` implementation is optional.
//! ///
//! #[derive(Clone, config_it::ConfigGroupData, Default, Debug)]
//! pub struct MyStruct {
//!     /// This docstring will be contained as metadata of given element
//!     #[config_it(min = -35)]
//!     minimal: i32,
//!
//!     #[config_it(default = 2, max = 3)]
//!     maximum: i32,
//!
//!     #[config_it(default = "3@", one_of("ab3", "go04"))]
//!     data: String,
//!
//!     #[config_it(default = 3112, one_of(1, 2, 3, 4, 5))]
//!     median: i32,
//!
//!     /// This property won't be counted as config group's property
//!     transient: f32,
//! }
//!
//! // Second tuple parameter is 'actor', which asynchronously drives all multithreaded
//! //  operations safely. If you utilize async executors, for example, `tokio`, then you can
//! //  simply spawn the whole storage operations as light-weight async task.
//! let (storage, async_worker) = config_it::Storage::new();
//! std::thread::spawn(move || block_on(async_worker)); // tokio::task::spawn(async_worker)
//!
//! // Since most of storage operations are run through actor, it is usually recommended to
//! //  use config_it inside async context.
//! let job = async move {
//!     // You can instantiate group within storage with `prefix` words.
//!     // Each words indicates hierarchy.
//!     let mut group: config_it::Group<_> =
//!         storage.create_group::<MyStruct>(["Main"]).await.unwrap();
//!
//!     // "Main" - `group`
//!     //   +-> "Sub" - `subgroup`
//!     let subgroup = storage
//!         .create_group::<MyStruct>(["Main", "Sub"])
//!         .await
//!         .unwrap();
//!
//!     // Prefix duplication is not allowed.
//!     storage
//!         .create_group::<MyStruct>(["Main", "Sub"])
//!         .await
//!         .unwrap_err();
//!
//!     // Group can fetch and apply changes from storage, using `update` method.
//!     // This operation clears dirty flag of config group, thus next update call won't
//!     //  return true until any update is committed to storage.
//!     assert!(group.update());
//!     assert!(!group.update());
//!
//!     // Individual properties of config group has their own dirty flag, thus after update,
//!     //  you can check if which element has changed. This operation consumes dirty flag
//!     //  either.
//!     assert!(group.check_elem_update(&group.data));
//!     assert!(!group.check_elem_update(&group.data));
//!
//!     // TODO: Write this ...
//!
//!     // As `Group<T>` implements `Deref` and `DerefMut` traits, you can access its values
//!     //  in simple manner.
//!     assert!(&group.data == "3@");
//!
//!     // You can modify `ConfigGroupData`, however, this change won't be visible to
//!     //  storage until you publish this change.
//!     group.data = "other_value".into();
//!
//!     // You can publish your changes to storage.
//!     // Second boolean parameter indicates whether you want to propagate this change or not.
//!     //  If you check 'true' here, then this change will
//!     group.commit_elem(&group.data, false);
//! };
//!
//! block_on(job);
//!
//! ```
//!
pub mod archive;
pub mod config;
pub mod core;
pub mod entity;
pub mod monitor;
pub mod storage;

pub use smartstring::alias::CompactString;

pub use archive::Archive;
pub use config::ConfigGroupData;
pub use config::Group;
pub use monitor::StorageMonitor;
pub use storage::Storage;

pub use lazy_static::lazy_static;
pub use macros::ConfigGroupData;

#[cfg(test)]
mod ttt {
    mod config_it {
        pub use crate::*;
    }

    #[test]
    fn doctest() {
        use futures::executor::block_on;

        ///
        /// Define set of properties using derive macro
        ///
        /// `Debug` implementation is optional.
        ///
        #[derive(Clone, config_it::ConfigGroupData, Default, Debug)]
        pub struct MyStruct {
            /// This docstring will be contained as metadata of given element
            #[config_it(min = -35)]
            minimal: i32,

            #[config_it(default = 2, max = 3)]
            maximum: i32,

            #[config_it(default = "3@", one_of("ab3", "go04"))]
            data: String,

            #[config_it(default = 3112, one_of(1, 2, 3, 4, 5))]
            median: i32,

            /// This property won't be counted as config group's property
            #[allow(unused)]
            transient: f32,
        }

        // Second tuple parameter is 'actor', which asynchronously drives all multithreaded
        //  operations safely. If you utilize async executors, for example, `tokio`, then you can
        //  simply spawn the whole storage operations as light-weight async task.
        let (storage, async_worker) = config_it::Storage::new();
        std::thread::spawn(move || block_on(async_worker)); // tokio::task::spawn(async_worker)

        // Since most of storage operations are run through actor, it is usually recommended to
        //  use config_it inside async context.
        let job = async move {
            // You can instantiate group within storage with `prefix` words.
            // Each words indicates hierarchy.
            let mut group: config_it::Group<_> =
                storage.create_group::<MyStruct>(["Main"]).await.unwrap();

            // "Main" - `group`
            //   +-> "Sub" - `subgroup`
            #[allow(unused)]
            let subgroup = storage
                .create_group::<MyStruct>(["Main", "Sub"])
                .await
                .unwrap();

            // Prefix duplication is not allowed.
            storage
                .create_group::<MyStruct>(["Main", "Sub"])
                .await
                .unwrap_err();

            // Group can fetch and apply changes from storage, using `update` method.
            // This operation clears dirty flag of config group, thus next update call won't
            //  return true until any update is committed to storage.
            assert!(group.update());
            assert!(!group.update());

            // Individual properties of config group has their own dirty flag, thus after update,
            //  you can check if which element has changed. This operation consumes dirty flag
            //  either.
            assert!(group.check_elem_update(&group.data));
            assert!(!group.check_elem_update(&group.data));

            // TODO: Write this ...

            // As `Group<T>` implements `Deref` and `DerefMut` traits, you can access its values
            //  in simple manner.
            assert!(&group.data == "3@");

            // You can modify `ConfigGroupData`, however, this change won't be visible to
            //  storage until you publish this change.
            group.data = "other_value".into();

            // You can publish your changes to storage.
            // Second boolean parameter indicates whether you want to propagate this change or not.
            //  If you check 'true' here, then this change will
            group.commit_elem(&group.data, false);
        };

        block_on(job);
    }
}
