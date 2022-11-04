use std::sync::Arc;

///
///
/// User interacts with this class
///
pub struct Registry {
    body: Arc<RegistryBackend>,
}

impl Registry {
    /// Creates new empty registry.
    fn new() -> Self {
        Self {
            body: Arc::new(RegistryBackend::default()),
        }
    }
}

///
///
/// Backend classes interacts with components under this namespace.
///
#[derive(Default)]
pub struct RegistryBackend {}

impl RegistryBackend {}

pub trait ObserveRegistry {}
