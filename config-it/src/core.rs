use std::sync::Arc;

use smartstring::alias::CompactString;

use crate::{config::GroupContext, entity::Metadata};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Storage driver is disposed")]
    ExpiredStorage,

    #[error("Config name is duplicated {0:?}")]
    GroupCreationFailed(Arc<Vec<CompactString>>),
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

    // TODO: Perform initial replication on open.
    MonitorRegister {},
}

pub(crate) struct GroupRegisterParam {
    pub group_id: u64,
    pub context: Arc<GroupContext>,
    pub event_broadcast: async_broadcast::Sender<()>,
    pub reply_success: oneshot::Sender<Result<(), Error>>,
}

pub enum MonitorEvent {
    /// TODO:
    ValueUpdateRequest,
}

///
///
/// Message type to notify backend
///
/// TODO: Fill appropriate values with these.
///
pub enum ReplicationEvent {
    InitialGroups(Vec<(u64, Arc<GroupContext>)>),
    GroupAdded(u64, Arc<GroupContext>),
    GroupRemoved(u64),
    EntityValueUpdated { group_id: u64, item_id: u64 },
}
