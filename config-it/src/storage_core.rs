use std::sync::Arc;

use smartstring::alias::CompactString;

use crate::config::SetCoreContext;

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

    ///
    /// Try registering config set to backend driver. Result will be delivered
    ///
    OnRegisterConfigSet(Box<ConfigSetRegisterDesc>),
}

pub(crate) struct ConfigSetRegisterDesc {
    pub register_id: u64,
    pub context: Arc<SetCoreContext>,
    pub event_broadcast: async_broadcast::Sender<()>,
    pub reply_success: oneshot::Sender<Result<(), Error>>,
}

const G: usize = std::mem::size_of::<ControlDirective>();
#[test]
fn print_size() {
    dbg!(G);
}

pub enum BackendEvent {
    /// TODO:
    NotifyValueUpdate,
}

///
///
/// Message type to notify backend
///
/// TODO: Fill appropriate values with these.
///
pub(crate) enum BackendReplicateEvent {
    /// TODO:
    InitInfo,

    /// TODO:
    CategoryAdded,

    /// TODO:
    CategoryRemoved,

    /// TODO:
    SetAdded,

    /// TODO:
    SetRemoved,

    /// TODO:
    EntityValueUpdated,
}
