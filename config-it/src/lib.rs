pub mod config;
pub mod entity;
pub mod storage;
pub mod storage_backend;
pub mod storage_core;

pub use smartstring::alias::CompactString;

pub use config::CollectPropMeta;
pub use config::Set;
pub use storage::Storage;

// TODO: Use macro
