//! Set of commonly used data structures.

pub mod archive;
pub mod meta;

use serde::{Deserialize, Serialize};
use std::hash::Hasher;

macro_rules! id_type {
    ($(#[doc = $doc:literal])* $id:ident $($args:tt)*) => {
        $(#[doc = $doc])*
        #[derive(
            Debug,
            Clone,
            Copy,
            Hash,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            derive_more::Display,
            Serialize,
            Deserialize
            $($args)*
        )]
        pub struct $id(pub u64);
    };
}

id_type!(
    /// Represents a unique identifier for a path within the archive.
    ///
    /// This ID type is a wrapper around a 64-bit unsigned integer and is used to distinguish and
    /// manage paths in a high-performance manner.
    ///
    /// As paths within the archive might be nested and represented hierarchically, having a unique
    /// ID facilitates efficient operations such as lookup, comparison, and storage.
    PathHash
);
id_type!(
    /// Represents a unique identifier for a group within the storage.
    ///
    /// `GroupID` is primarily used to track, manage, and differentiate between different
    /// configuration sets registered in storage. Each group is assigned a unique ID, ensuring
    /// accurate management and operations upon it.
    ///
    /// This ID type also supports the `From` trait, which allows seamless conversion from certain
    /// other types.
    GroupID,
    derive_more::From
);
id_type!(
    /// Represents a unique identifier for an individual item or element.
    ///
    /// `ItemID` allows for the differentiation and management of individual configuration items
    /// within the storage or a group. Each item, whether it's a variable, property, or another
    /// entity, is assigned a unique ID for precise operations and management.
    ///
    /// The `From` trait support ensures that `ItemID` can be easily constructed from other relevant
    /// types.
    ItemID,
    derive_more::From
);

impl PathHash {
    /// Creates a new `PathHash` by hashing a sequence of path strings.
    ///
    /// This function takes an iterable of path strings and sequentially hashes each string to
    /// produce a unique identifier in the form of `PathHash`. To ensure uniqueness and avoid
    /// collisions between consecutive paths, a delimiter (`\x03\x00`) is written between each
    /// hashed string.
    ///
    /// # Arguments
    ///
    /// * `paths` - An iterable of path strings that need to be hashed together to produce the
    ///   `PathHash`.
    ///
    /// # Returns
    ///
    /// * A new `PathHash` which represents the hashed value of the concatenated path strings.
    ///
    /// # Example
    ///
    /// ```
    /// let path_hash = config_it::shared::PathHash::new(["path1", "path2", "path3"]);
    /// ```
    pub fn new<'a>(paths: impl IntoIterator<Item = &'a str>) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        paths.into_iter().for_each(|x| {
            hasher.write(x.as_bytes());
            hasher.write(b"\x03\x00"); // delim
        });
        Self(hasher.finish())
    }
}

impl<'a, T: IntoIterator<Item = &'a str>> From<T> for PathHash {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl GroupID {
    pub(crate) fn new_unique_incremental() -> Self {
        static ID_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self(ID_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

impl ItemID {
    pub(crate) fn new_unique_incremental() -> Self {
        static ID_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self(ID_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}
