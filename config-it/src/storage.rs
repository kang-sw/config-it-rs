use crate::config;
use smol_str::SmolStr;

///
/// Storage manages multiple sets registered by preset key.
///
/// A storage provides following functionalities:
/// - Create config set
/// -
///
pub struct Storage {}

impl Storage {
    ///
    /// Creates new config set from storage.
    ///
    /// # Parameters
    ///
    /// - `category`:
    ///
    pub fn create_set<T: config::CollectPropMeta>(
        &self,
        category: Vec<SmolStr>,
    ) -> config::Set<T> {
        todo!()
    }

    // TODO: Dump all contents I/O from/to Serializer/Deserializer

    // TODO: Install change notification hook

    // dfdlskaj
}
