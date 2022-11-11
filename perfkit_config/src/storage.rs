use std::sync::Arc;
use crate::registry::Registry;

///
///
/// User interacts with this class.
///
#[derive(Clone)]
pub struct Storage {
    body: Arc<StorageBody>,
}

struct StorageBody {}

impl Storage {
    ///
    /// Creates new storage instance
    ///
    pub fn new(rg: Registry, category: String) -> Self { todo!() }

    ///
    /// Gets current update fence value
    ///
    pub(crate) fn update_fence(&self) -> usize { todo!() }
}

