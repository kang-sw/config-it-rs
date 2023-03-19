use std::time::Duration;

use capture_it::capture;
use config_it_monitor::server::StorageTable;
use tracing::metadata::LevelFilter;

#[derive(config_it::Template, Clone, Debug)]
struct CoreCfg {
    #[config(transient)]
    tick: usize,

    #[config(transient)]
    add_sub: bool,

    #[config(transient)]
    exit: bool,
}

#[derive(config_it::Template, Clone, Debug)]
struct SubCfg {
    #[config]
    name: String,
}

fn main() {
    tracing_subscriber::fmt()
        .compact()
        .with_target(false)
        .with_file(true)
        .with_max_level(LevelFilter::DEBUG)
        .init();

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async_main());
}

async fn async_main() {
    let (storage, task) = config_it::create_storage();
    tokio::spawn(task);

    tokio::spawn(capture!([storage], async move {
        config_it_monitor::server::Service::builder()
            .bind_port(9000)
            .table(
                StorageTable::default()
                    .entry("default", storage.clone())
                    .add_access_key("admin:admin", true)
                    .submit(),
            )
            .system_name("example")
            .system_desc("sample system")
            .build()
            .serve()
            .await
            .unwrap();
    }));

    let mut g_core = storage.create_group::<CoreCfg>(["core"]).await.unwrap();
    tokio::spawn(capture!([*g_core], async move {
        loop {
            g_core.tick += 1;
            g_core.commit_elem(&g_core.tick, false);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }));

    let mut evt = g_core.watch_update();
    tokio::spawn(capture!([*g_core], async move {
        while let Ok(_) = evt.recv().await {
            if g_core.update() && g_core.add_sub {
                println!("info: adding sub ...");
                g_core.add_sub = false;
                g_core.commit_elem(&g_core.add_sub, false);

                let mut g_sub = storage
                    .create_group::<SubCfg>(["sub", &format!("{}", g_core.tick)])
                    .await
                    .unwrap();

                tokio::spawn(async move {
                    let mut evt = g_sub.watch_update();

                    while let Ok(_) = evt.recv().await {
                        if g_sub.update() {
                            println!("info: sub updated = {:#?}", *g_sub);
                        }
                    }
                });
            }
        }
    }));

    let mut evt = g_core.watch_update();
    while let Ok(_) = evt.recv().await {
        if g_core.update() {
            println!("info: core updated = {:#?}", *g_core);
        }

        if g_core.exit {
            break;
        }
    }
}
