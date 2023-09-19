use serde::{Deserialize, Serialize};
use std::hash::Hasher;

macro_rules! id_type {
    ($id:ident $($args:tt)*) => {
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

id_type!(PathHash);

id_type!(GroupID, derive_more::From);
id_type!(ItemID, derive_more::From);

impl PathHash {
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
    pub(crate) fn new_unique() -> Self {
        static ID_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self(ID_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

impl ItemID {
    pub(crate) fn new_unique() -> Self {
        static ID_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self(ID_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}
