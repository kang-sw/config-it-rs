use std::sync::Arc;

use smallvec::SmallVec;
use smartstring::alias::CompactString;

use crate::{archive, config::GroupContext};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Storage driver is disposed")]
    ExpiredStorage,

    #[error("Config name is duplicated {0:?}")]
    GroupCreationFailed(Arc<Vec<CompactString>>),

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

    GroupDisposal(u64),

    EntityValueUpdate {
        group_id: u64,
        item_id: u64,
        silent_mode: bool,
    },

    #[allow(unused)] // TODO: Implement monitoring logics!
    MonitorRegister {
        reply_to: oneshot::Sender<async_channel::Receiver<ReplicationEvent>>,
    },

    Import {
        body: archive::Archive,
        merge_onto_exist: bool,
    },

    Export {
        /// If None is specified,
        destination: oneshot::Sender<archive::Archive>,
        no_merge_exist: bool,
        no_update: bool,
    },
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
/// TODO: Fill appropriate values with these.
///
#[derive(Clone)]
pub enum ReplicationEvent {
    InitialGroups(Vec<(u64, Arc<GroupContext>)>),
    GroupAdded(u64, Arc<GroupContext>),
    GroupRemoved(u64),
    EntityValueUpdated { group_id: u64, item_id: u64 },
}
