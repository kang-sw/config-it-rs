use std::{
    any::{Any, TypeId},
    hash::Hasher,
    sync::Arc,
};

use compact_str::CompactString;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::{archive, config::GroupContext, ExportOptions, ImportOptions};

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
        paths.into_iter().for_each(|x| hasher.write(x.as_bytes()));
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
    #[error("Storage driver is disposed")]
    ExpiredStorage,

    #[error("Config name is duplicated {0:?}")]
    GroupCreationFailed(Arc<Vec<CompactString>>),

    #[error("Path exist with different type")]
    MismatchedTypeID,

    #[error("Deserialization failed")]
    DeserializationFailed(#[from] erased_serde::Error),

    #[error("Validation failed")]
    ValueValidationFailed,
}

///
///
/// Message type to drive storage
///
pub(crate) enum ControlDirective {
    FromMonitor(MonitorEvent),

    TryGroupRegister(Box<GroupRegisterParam>),

    TryFindGroup {
        path_hash: PathHash,
        type_id: TypeId,

        reply: oneshot::Sender<Result<FoundGroupInfo, GroupFindError>>,
    },

    GroupDisposal(GroupID),

    Fence(oneshot::Sender<()>),

    EntityValueUpdate {
        group_id: GroupID,
        item_id: ItemID,
        silent_mode: bool,
    },

    MonitorRegister {
        reply_to: oneshot::Sender<flume::Receiver<ReplicationEvent>>,
    },

    Import {
        body: archive::Archive,
        option: ImportOptions,
    },

    Export {
        /// If None is specified,
        destination: oneshot::Sender<archive::Archive>,
        option: ExportOptions,
    },

    Close,
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

/// Contains all necessary information to construct a group
pub(crate) struct FoundGroupInfo {
    pub context: Arc<GroupContext>,

    /// Locked before used, since if we deliver the weak pointer as-is, it might be expired
    /// before the receiver uses it.
    pub unregister_hook: Arc<dyn Any + Send + Sync>,
}

pub(crate) struct GroupRegisterParam {
    pub group_id: GroupID,
    pub context: Arc<GroupContext>,
    pub event_broadcast: async_broadcast::Sender<()>,
    pub reply_success: oneshot::Sender<Result<(), Error>>,
}

pub enum MonitorEvent {
    GroupUpdateNotify { updates: SmallVec<[GroupID; 4]> },
}

///
///
/// Message type to notify backend
///
#[derive(Clone)]
pub enum ReplicationEvent {
    InitialGroups(Vec<(GroupID, Arc<GroupContext>)>),
    GroupAdded(GroupID, Arc<GroupContext>),
    GroupRemoved(GroupID),
    EntityValueUpdated { group_id: GroupID, item_id: ItemID },
}
