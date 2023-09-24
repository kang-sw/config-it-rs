#![cfg(feature = "crypt-machine-id")]

use std::collections::HashMap;

use config_it::{commit_elem, meta::MetaFlag};

#[derive(config_it::Template, Clone)]
struct CryptTest {
    #[config(secret)]
    secret_str: String,

    #[config(secret)]
    secret_seq: Vec<usize>,

    #[config(secret)]
    secret_floats: Vec<f64>,

    #[config(secret)]
    secret_map: HashMap<i64, String>,
}

#[test]
fn test_crypt() {
    let storage = config_it::create_storage();
    let mut group = storage.create::<CryptTest>(["Basics"]).unwrap().updated();

    group.secret_str = "Hello, world!".to_string();
    group.secret_seq = vec![1, 2, 3, 4, 5];
    group.secret_floats = vec![1.01234, 2.04511, 3.03222, 4.06615, 5.04213];
    group.secret_map =
        vec![(1, "one".to_string()), (2, "two".to_string()), (3, "three".to_string())]
            .into_iter()
            .collect();

    assert!(group.meta(&group.secret_str).flags.contains(MetaFlag::SECRET));
    assert!(group.meta(&group.secret_seq).flags.contains(MetaFlag::SECRET));
    assert!(group.meta(&group.secret_floats).flags.contains(MetaFlag::SECRET));
    assert!(group.meta(&group.secret_map).flags.contains(MetaFlag::SECRET));

    commit_elem!(group, notify(secret_str, secret_seq, secret_floats, secret_map));

    dbg!(storage.exporter().collect());
}
