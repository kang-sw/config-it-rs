use serde::{Deserialize, Serialize};
use std::{hash::Hasher, sync::Arc};

use crate::config::GroupContext;

macro_rules! id_type {
    ($id:ident) => {
        #[derive(
            Debug,
            Clone,
            Copy,
            Hash,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            derive_more::From,
            derive_more::Display,
            Serialize,
            Deserialize,
        )]
        pub struct $id(pub u64);
    };
}

id_type!(PathHash);
id_type!(GroupID);
id_type!(ItemID);

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

///
/// ?
///
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Config name is duplicated")]
    GroupPathDuplication,

    #[error("Path exist with different type")]
    MismatchedTypeID,

    #[error("Deserialization failed")]
    DeserializationFailed(#[from] erased_serde::Error),

    #[error("Validation failed")]
    ValueValidationFailed,
}

#[derive(thiserror::Error, Debug)]
pub enum GroupFindError {
    #[error("Given path was not found")]
    PathNotFound,
    #[error("Type ID mismatch from original registration")]
    MismatchedTypeID,
    #[error("The original group was already disposed")]
    ExpiredStorage,
}

pub trait Monitor: Send + Sync + 'static {
    fn initial_groups(&self, iter: &mut dyn Iterator<Item = (&GroupID, &Arc<GroupContext>)>) {
        let _ = iter;
    }
    fn group_added(&self, group_id: &GroupID, group: &Arc<GroupContext>) {
        let _ = (group_id, group);
    }
    fn group_removed(&self, group_id: &GroupID) {
        let _ = group_id;
    }
    fn entity_value_updated(&self, group_id: &GroupID, item_id: &ItemID) {
        let _ = (group_id, item_id);
    }
}
