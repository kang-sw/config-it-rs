//!
//! A crate for asynchronous centralized configuration management.
//!
//! # Usage
//!
//! You can define 'Template' which defines set of grouped properties, which should be updated at
//!  once. Only `config_it` decorated properties will be counted as part of given group and will be
//!  managed.
//!
//! `Template` must implement `Clone` trait.
//!
//! You should create `Storage` to create config group instances(`Group<T:Template>`). `Storage` is
//!  the primary actor for centralized configuration management. Config groups can be instantiated
//!  based on storage instance.
//!
//! Any property implements `serde::Serialize`, `serde::DeserializeOwned` can be used as
//! configuration property.
//!
//! # Attributes
//!
//! Any field decorated with attribute `#[config_it]` or `#[config]` will be treated as
//! configuration property. You can specify additional constraints for the property by adding
//! additional attributes inside parenthesis.
//!
//! - `default = <value>`
//!     - Specify default value for the property. as the <value> expression is converted into field
//!       type using `.try_into().unwrap()` expression, you should specify un-fallible expression
//!       here. This is to support convenient value elevation from `&str` to `String`, or similar
//!       owning conversions.
//! - `default_expr = "<expr>"`
//!     - Specify complicated value expression here. To specify string literal here, you have to
//!       escape double quotes(`"`) with backslash(`\`). As this attribute converts given expression
//!       into token tree directly, you can write any valid rust expression here.
//! - `rename = "<alias>"`
//!     - Specify alias name for the property. This is useful when you want to use different name
//!       for the property in config file, but want to use original name in code.
//! - `one_of = [<value>, <value>, ...]`
//!     - Specify set of allowed values for the property. This is useful when you want to restrict
//!       the value of the property to specific set of values. You can also specify default value as
//!       one of the allowed values.
//!     - Default value can be out of the allowed set, and can be excluded from the allowed set. In
//!       this case, setting value back to default value will not be allowed.
//! - `min = <value>`, `max=<value>`
//!     - Constrain the value of the property to be within given range. Any type which implements
//!       `Ord` can have min/max constraints.
//! - `env = "<env_var>"` or `env_once = "<env_var>"`
//!     - Specify environment variable name to import value from. If the environment variable is not
//!       set, the default value will be used. `TryParse` trait is used to convert environment
//!       variable value into property type.
//!     - `env_once` is only evaluated once lazily and reuse cached value after creation.
//! - `no_import`
//!     - Do not update value from imported archive. This is useful when mixed with `env` flag,
//!       which will keep its value as imported environment variable even after the archive is
//!       imported.   
//! - `no_export`
//!     - Do not export value to archive.
//! - `transient`
//!     - Value won't be archived, and won't be imported from archive.
//! - `hidden`
//!     - Hints to monitoring system that this property should not be visible.
//! - `no_notify`
//!
//! # Non-default values
//!
//! A storage template may consist of multiple non-config field. Since [`Template`] macro does not
//! require `Default` trait to be implemented, it will report error if the non-config type does not
//! provide type-level default implementation.
//!
//! ```compile_fail
//! #[derive(config_it::Template, Clone)]
//! struct Config {
//!     #[config(default = 154)]
//!     pub any_number: i32,
//!
//!     // This is just okay.
//!     pub non_config_with_default: usize,
//!
//!     pub non_config_number: std::num::NonZeroUsize,
//!  // ^^^ the trait `Default` is not implemented for `NonZeroUsize`
//! }
//! ```
//! In this case, you can specify non-default value for the field via `non_config_default_expr`
//! attribute. This attribute accepts string literal, which will be parsed as rust expression.
//!
//! ```no_run
//! #[derive(config_it::Template, Clone)]
//! struct Config {
//!     #[config(default = 154)]
//!     pub any_number: i32,
//!
//!     #[non_config_default_expr = r#"1.try_into().unwrap()"#]
//!     pub non_config_number: std::num::NonZeroUsize,
//! }
//! ```
//!
#[cfg(feature = "config")]
pub mod config;
pub mod shared;

// Just re-exported, for compatibility.
pub extern crate serde;

pub use shared::{archive, meta};

// [`core::archive::Archive`] utilizes this.
pub use serde_json::Value as ArchiveValue;

#[cfg(feature = "jsonschema")]
pub extern crate schemars;

#[cfg(feature = "jsonschema")]
pub use schemars::schema::{RootSchema as Schema, SchemaObject};

#[cfg(feature = "config")]
pub use config_export::*;

#[doc(hidden)]
#[cfg(feature = "config-derive")]
pub use memoffset::offset_of;

#[doc(hidden)]
#[cfg(feature = "config-derive")]
pub use impls::impls;

/// Primary macro for defining configuration group template.
#[cfg(feature = "config-derive")]
pub use macros::Template;

#[cfg(feature = "config")]
mod config_export {
    use crate::config::*;
    use crate::shared::*;

    pub use entity::{Validation, ValidationResult};

    pub use archive::Archive;
    pub use group::{Group, Template};
    pub use storage::{Monitor, Storage};

    #[cfg(feature = "arc-swap")]
    pub use storage::atomic::AtomicStorageArc;

    pub fn create_storage() -> Storage {
        Storage::default()
    }

    pub type BroadcastReceiver = noti::Receiver;
    pub type ArchiveCategoryRule<'a> = archive::CategoryRule<'a>;

    /// Shorthand macro for consuming multiple updates.
    ///
    /// With bracket syntax, this macro returns array of bool values, which indicates whether each
    /// elements has been updated. With simple variadic syntax, this macro returns boolean value which
    /// indicates whether any of given elements has been updated.
    ///
    /// If elements are wrapped inside parenthesis, this macro returns temporary struct which has
    /// boolean fields names each elements.
    ///
    /// In both cases, all supplied arguments will be evaluated.
    #[macro_export]
    macro_rules! consume_update{
        ($group: expr, [$($elems:ident),*]) => {
            {
                let __group = ($group).__macro_as_mut();
                [$(__group.consume_update(&__group.$elems)), *]
            }
        };

        ($group: expr, (($($elems:ident),*))) => {
            {
                #[derive(Debug, Clone, Copy)]
                struct Updates {
                    $($elems: bool),*
                }

                let __group = ($group).__macro_as_mut();
                Updates {
                    $($elems: __group.consume_update(&__group.$elems)),*
                }
            }
        };

        ($group: expr, $($elems:ident),*) => {
            {
                let __group = ($group).__macro_as_mut();
                $(__group.consume_update(&__group.$elems)) | *
            }
        };
    }

    /// Shorthand macro for marking multiple elements dirty.
    #[macro_export]
    macro_rules! mark_dirty{
    ($group: expr, $($elems:ident),+) => {
            {
                let __group = ($group).__macro_as_mut();
                $(__group.mark_dirty(&__group.$elems)); *
            }
        }
    }

    /// Shorthand macro for committing multiple elements.
    #[macro_export]
    macro_rules! commit_elem{
        ($group: expr, ($($elems:ident),+)) => {
            {
                let __group = ($group).__macro_as_mut();
                $(__group.commit_elem(&__group.$elems, false)); *
            }
        };

    ($group: expr, notify($($elems:ident),+)) => {
            {
                let __group = ($group).__macro_as_mut();
                $(__group.commit_elem(&__group.$elems, true)); *
            }
        };
    }
}
