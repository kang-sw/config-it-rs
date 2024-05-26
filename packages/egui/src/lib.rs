use config_it::Storage;

pub mod link {}

#[cfg(feature = "egui")]
pub mod egui {}

// TODO: dioxus monitor UI?

/// Creates monitoring link to given configuration storage.
pub struct StorageMonitor {
    src: Storage,
}

impl StorageMonitor {
    /// Create new storage monitor from given storage instance. It'll replace existing storage's
    /// monitor reference.
    pub fn new(storage: &config_it::Storage) -> Self {
        todo!()
    }
}
