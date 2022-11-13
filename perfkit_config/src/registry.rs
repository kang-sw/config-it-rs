use std::sync::Arc;

///
///
/// A root registry for multiple config storages.
///
/// This class simply works as handle to registry instance, as registry itself is vastly shared
///  across multiple components/classes.
///
/// Implementation for `Registry` provides various user-interact methods.
///
pub struct Registry(Arc<RegistryBody>);

struct RegistryBody {}

impl Registry {
    ///
    /// Creates empty registry instance.
    ///
    pub(crate) fn new() -> Self {
        todo!()
    }

    // TODO: Register event monitor channel (std version ...)
    // TODO: Register new storage
    // TODO: Unregister expired storage
}

///
///
/// Context of single storage registration.
///
/// Will be shared between registry and storage.
///
pub(crate) struct StorageContext {}

impl StorageContext {
    // TODO: Notify config set registration/unregistration (with multiple entities)
}
