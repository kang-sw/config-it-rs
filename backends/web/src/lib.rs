use config_it::CompactString;
use tokio::sync::oneshot;

mod actor;
mod api;

pub enum PublicFile {
    DownloadArchive(String),
    DownloadUri(String),
    LocalPath(String),
}

pub struct Builder {
    pub app_name: CompactString,
    pub description: String,

    pub public_file: PublicFile,
    pub storage: Vec<config_it::Storage>,

    // TODO: Find out how to define authentication ...
    pub command_stream: Option<async_channel::Sender<String>>,
    pub terminal_stream: Option<async_channel::Receiver<String>>,

    pub close_signal: Option<oneshot::Receiver<()>>,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            app_name: "App".into(),
            description: Default::default(),
            public_file: PublicFile::DownloadUri(
                "TODO: Publish files to github, hard code the link here.".into(),
            ),
            storage: Default::default(),
            command_stream: None,
            terminal_stream: None,
            close_signal: None,
        }
    }
}

impl Builder {
    pub async fn build(self) -> axum::Router {
        // Create context with self, launch event loop.
        let tx = actor::Context::launch(self).await;

        // Build API router with tx
        api::build(tx)
    }

    pub fn add_storage(mut self, storage: config_it::Storage) -> Self {
        self.storage.push(storage);
        self
    }

    pub fn with_close_signal(mut self, ch: oneshot::Receiver<()>) -> Self {
        self.close_signal = Some(ch);
        self
    }

    pub fn with_terminal_writer(mut self, ch: async_channel::Receiver<String>) -> Self {
        self.terminal_stream = Some(ch);
        self
    }

    pub fn with_command_receiver(mut self, ch: async_channel::Sender<String>) -> Self {
        self.command_stream = Some(ch);
        self
    }
}

#[cfg(test)]
mod _test {
    use std::sync::Arc;

    use axum::{routing::get, Extension};

    struct Context {
        value: i32,
    }

    async fn handler(Extension(ext): Extension<Arc<Context>>) -> String {
        format!("Value is: {}", ext.value)
    }

    #[tokio::test]
    #[ignore]
    async fn layer_test() {
        let app = axum::Router::new()
            .route("/test1", get(handler))
            .layer(Extension(Arc::new(Context { value: 1 })))
            .route("/test2", get(handler))
            .layer(Extension(Arc::new(Context { value: 2 })))
            .route("/test3", get(handler));

        axum::Server::bind(&"0.0.0.0:15572".parse().unwrap())
            .serve(app.into_make_service())
            .await
            .unwrap();
    }
}
