use std::sync::Arc;

use smartstring::alias::CompactString;

use crate::{config::SetContext, entity::Metadata};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Storage driver is disposed")]
    ExpiredStorage,

    #[error("Config name is duplicated {0:?}")]
    SetCreationFailed(Arc<Vec<CompactString>>),
}

///
///
/// Message type to drive storage
///
pub(crate) enum ControlDirective {
    Backend(BackendEvent),

    OnRegisterConfigSet(Box<ConfigSetRegisterDesc>),

    OnUnregisterConfigSet(u64),

    EntityNotifyCommit { register_id: u64, item_id: u64 },

    EntityValueUpdate { register_id: u64, item_id: u64 },

    // TODO: Perform initial replication on open.
    NewSessionOpen {},
}

pub(crate) struct ConfigSetRegisterDesc {
    pub register_id: u64,
    pub context: Arc<SetContext>,
    pub event_broadcast: async_broadcast::Sender<()>,
    pub reply_success: oneshot::Sender<Result<(), Error>>,
}

pub enum BackendEvent {
    /// TODO:
    ValueUpdateRequest,
}

///
///
/// Message type to notify backend
///
/// TODO: Fill appropriate values with these.
///
pub enum BackendReplicateEvent {
    InitInfo,
    CategoryAdded,
    CategoryRemoved,
    SetAdded,
    SetRemoved,
    EntityValueUpdated,
}
