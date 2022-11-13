use std::collections::{HashMap, HashSet};
use std::hash::Hasher;
use std::sync;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use crate::entity::{EntityBase, Metadata};
use crate::registry::Registry;

#[cfg(feature = "tokio")]
use tokio::sync::broadcast;

///
///
/// Stores multiple storage instance. A proxy to storage body class.
///
#[derive(Clone)]
pub struct Storage {
    body: Arc<StorageBody>,
}

pub(crate) struct StorageBody {
    name: String,
    rg: Registry,
    ictx: Mutex<InternalContext>,
    local_offset_id_gen: AtomicUsize,
}

struct InternalContext {
    /// To check if prefix duplication occurs ...
    prefix_dup_table: HashSet<u64>,

    /// - Key: offset fence, generated from local_offset_id_gen
    config_sets: HashMap<usize, Arc<ConfigSetContext>>,
}

pub(crate) struct ConfigSetContext {
    /// Also used as hash-map key
    pub alloc_offset_id: usize,

    /// List of all contained entities
    pub entities: Vec<Arc<EntityBase>>,

    /// Cached prefix string sequence
    pub prefix: Arc<[String]>,

    /// Cached prefix hash calculation. Will be used for removing `prefix_dup_table`
    ///  on unregister.
    pub prefix_hash_cache: u64,

    /// Internal context value.
    update_fence: AtomicUsize,
}

impl Storage {
    ///
    /// Creates new storage instance.
    ///
    /// Returns existing storage instance
    ///
    pub fn new(rg: Registry, category: &str) -> Self {
        // Try create or find reference to storage body.
        todo!()
    }

    ///
    /// Registers set of entities
    ///
    /// Returns offset id, that each entity will be registered
    ///  as id `[retval ... retval + entities.size()]`
    ///
    pub(crate) fn __register(&self, prefix: &[&str], meta_ents: &[Arc<Metadata>]) -> Option<Arc<ConfigSetContext>> {
        // Generate hash string from prefixes
        let prefix_hash = {
            let mut hash = fnv::FnvHasher::default();
            for x in prefix {
                hash.write_usize(x.len());
                hash.write(x.as_bytes());
            }
            hash.finish()
        };

        // Concat prefix strings into shared string array
        let prefix = prefix.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let prefix: Arc<[String]> = prefix.into();

        // Calculate registration offset. The only constraint for local offset id is uniqueness,
        //  thus it's okay to pre-assigning ID value, even it fails later.
        let offset_id = self.body.local_offset_id_gen.fetch_add(meta_ents.len(), Ordering::Relaxed);

        // Create list of config entities.
        let mut entities = Vec::with_capacity(meta_ents.len());
        for (meta, index) in meta_ents.iter().zip(0..meta_ents.len()) {
            let w_body = Arc::downgrade(&self.body);

            entities.push(EntityBase::create(
                meta.clone(),
                offset_id + index,
                prefix.clone()));
        }

        // Create config set context
        let ctx_set = ConfigSetContext {
            alloc_offset_id: offset_id,
            prefix_hash_cache: prefix_hash,
            prefix: prefix.clone(),
            entities: entities.clone(),
            update_fence: AtomicUsize::new(0),
        };
        let ctx_set = Arc::new(ctx_set);

        // Modify internal state
        {
            let mut ctx = self.body.ictx.lock().unwrap();

            // Check if given prefix hash duplicates with existing config set context
            if ctx.prefix_dup_table.insert(prefix_hash) == false {
                return None;
            }

            assert!(ctx.config_sets.insert(offset_id, ctx_set.clone()).is_none());
        }

        // TODO: Access registry's config cache, load initial config values to entities
        Some(ctx_set)
    }

    ///
    /// Unregisters given config set with offset_id.
    ///
    pub(crate) fn __unregister(&self, offset_id: usize) {
        let mut ctx = self.body.ictx.lock().unwrap();
        let mut elem = ctx.config_sets.remove(&offset_id).unwrap();
        assert!(ctx.prefix_dup_table.remove(&elem.prefix_hash_cache));

        // TODO: Notify removal to registry
    }

    ///
    /// Creates end-user event receiver
    ///
    #[cfg(feature = "tokio")]
    pub fn subscribe_events(&self) -> broadcast::Receiver<StorageEvent> {
        todo!()
    }

    // TODO: Dump to serializer
    // TODO: Load from deserializer
}

impl ConfigSetContext {
    pub fn check_update(&self, local_fence: &mut usize) -> bool {
        match self.update_fence.load(Ordering::Relaxed) {
            v if v == *local_fence => false,
            v => {
                *local_fence = v;
                true
            }
        }
    }
}

impl StorageBody {
    // TODO: Commit value from backend
    // TODO: Commit value from user
    // TODO:
}

pub enum StorageEvent {
    ///
    /// Remote backend send update to this storage.
    ///
    /// - 0: Remote backend identifier
    /// - 1: Updated target's registration IDs (sorted)
    ///
    RemoteUpdate(Arc<str>, Arc<[usize]>),

    ///
    /// Imported from any deserializer
    ///
    Import,

    ///
    /// Exported to any serializer
    ///
    Export,
}
