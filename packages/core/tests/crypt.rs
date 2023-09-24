#![cfg(feature = "config-derive")]
#![cfg(feature = "crypt")]

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
    #[cfg(feature = "crypt-machine-id")]
    test(None::<&str>);

    test(Some("afsd09fdsa9cvkk"));
    test(Some("as00cp;la'''''aF_)i09fawopig ib9"));
    test(Some(b"fdas9"));
    test(Some(b"fas--adg09cv,,,od0"));
    test(Some(b"fasrj"));

    for _ in 0..100 {
        test(Some(rand::random::<[u8; 15]>()));
    }
}

fn test(with_key: Option<impl AsRef<[u8]>>) {
    let storage = config_it::create_storage();

    if let Some(key) = with_key {
        storage.set_crypt_key(key);
    }

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
    drop(group);

    let group = storage.create::<CryptTest>(["Basics"]).unwrap().updated();

    assert_eq!(group.secret_str, "Hello, world!");
    assert_eq!(group.secret_seq, [1, 2, 3, 4, 5]);
    assert_eq!(group.secret_floats, [1.01234, 2.04511, 3.03222, 4.06615, 5.04213]);
    assert_eq!(
        group.secret_map,
        vec![(1, "one".to_string()), (2, "two".to_string()), (3, "three".to_string())]
            .into_iter()
            .collect()
    );

    drop(group);
    storage.import(Default::default()).apply_as_patch(false).merge_onto_cache(false);

    let group = storage.create::<CryptTest>(["Basics"]).unwrap().updated();

    assert_eq!(group.secret_str, "");
    assert!(group.secret_seq.is_empty());
    assert!(group.secret_floats.is_empty());
    assert!(group.secret_map.is_empty());
}
