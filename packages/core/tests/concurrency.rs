#![cfg(feature = "config-derive")]
use config_it::commit_elem;

#[derive(config_it::Template, Clone)]
struct Foo {
    /// This is to
    /// Multi
    ///
    /// ANd
    #[config(default = 96, env_once = "DLOGIIO")]
    var: u32,

    #[config(default = (15, 61))]
    varg: (u32, u32),

    #[config(default = "hello-woll---rd", env = "ADLSOC")]
    vk: String,

    #[config(one_of=[3,9900,150191,21430124,1,124])]
    tew: u32,

    #[non_config_default_expr = "4"]
    _other: i32,
}

#[test]
fn thread_stress_test() {
    let storage = config_it::create_storage();
    let tpool = threadpool::ThreadPool::default();

    for _ in 0..1000 {
        let storage = storage.clone();
        tpool.execute(move || {
            tick(storage.clone());
        });
    }

    tpool.join();
}

const PATH_PATTERNS: &[&[&str]] = &[
    &["packages", "core", "tests", "concurrency.rs"],
    &["packages", "core", "tests", "macro-defaults.rs"],
    &["tests", "macro-derive.rs"],
    &["packages", "macro-derive.rs"],
    &["packages", "tests", "macro-derive.rs"],
];

fn tick(storage: config_it::Storage) {
    let mut group = storage
        .find_or_create::<Foo>(PATH_PATTERNS[rand::random::<u8>() as usize % PATH_PATTERNS.len()])
        .unwrap();

    for iter in 0..1000 {
        group.update();

        if iter > 500 {
            assert_eq!(group.var, 14, "If this crashes, go and buy a lottery ticket");
            assert_eq!(group.varg, (14, 8));
            assert_eq!(group.vk, "hello");
            assert_eq!(group.tew, 1);
        }

        group.var = 14;
        group.varg = (14, 8);
        group.vk = "hello".to_string();
        group.tew = 1;

        commit_elem!(group, notify(var, varg, vk, tew));
    }
}
