use std::{
    any::{Any, TypeId},
    sync::Arc,
};

use compact_str::CompactString;
use smallvec::SmallVec;

use crate::{archive, config::GroupContext, ExportOptions, ImportOptions};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Storage driver is disposed")]
    ExpiredStorage,

    #[error("Config name is duplicated {0:?}")]
    GroupCreationFailed(Arc<Vec<CompactString>>),

    #[error("Path exist with different type")]
    DuplicatedPath,

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
        path_hash: u64,
        type_id: TypeId,

        reply: oneshot::Sender<Result<FoundGroupInfo, GroupFindError>>,
    },

    GroupDisposal(u64),

    Fence(oneshot::Sender<()>),

    EntityValueUpdate {
        group_id: u64,
        item_id: u64,
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

pub(crate) enum GroupFindError {
    PathNotFound,
    TypeIdMismatch,
}

/// Contains all necessary information to construct a group
pub(crate) struct FoundGroupInfo {
    pub context: Arc<GroupContext>,

    /// Locked before used, since if we deliver the weak pointer as-is, it might be expired
    /// before the receiver uses it.
    pub unregister_hook: Arc<dyn Any + Send + Sync>,
}

pub(crate) struct GroupRegisterParam {
    pub group_id: u64,
    pub context: Arc<GroupContext>,
    pub event_broadcast: async_broadcast::Sender<()>,
    pub reply_success: oneshot::Sender<Result<(), Error>>,
}

pub enum MonitorEvent {
    GroupUpdateNotify { updates: SmallVec<[u64; 4]> },
}

///
///
/// Message type to notify backend
///
#[derive(Clone)]
pub enum ReplicationEvent {
    InitialGroups(Vec<(u64, Arc<GroupContext>)>),
    GroupAdded(u64, Arc<GroupContext>),
    GroupRemoved(u64),
    EntityValueUpdated { group_id: u64, item_id: u64 },
}
