use smartstring::alias::CompactString;

///
///
/// Message type to drive storage
///
pub(crate) enum ControlDirective {
    Backend(BackendEvent),
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
