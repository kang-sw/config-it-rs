mod common;

use config_it_web_dashboard as dashboard;
use futures::join;

#[tokio::test]
#[ignore]
async fn test_app() {
    let (storage, task) = config_it::create_storage();
    tokio::spawn(task);

    let (tx, mut rx) = dashboard::channel::unbounded();
    let svc = dashboard::Builder::new()
        .with_name("TestApp".into())
        .with_bind_port(15672)
        .with_command_source(tx)
        .add_storage(storage.clone());

    let task_run_service = svc.run();
    let task_parse_command = async {
        while let Ok(cmd) = rx.recv().await {
            println!("Received command: {}", cmd);

            match cmd.as_str() {
                "q" | "quit" => break,
                _ => (),
            }
        }
    };

    fn test_send(_: &impl Send) {}
    test_send(&task_run_service);
    test_send(&task_run_service);

    join!(task_parse_command, task_run_service);
}
