use std::net::{IpAddr, Ipv4Addr};

use axum::routing::get;
use config_it::CompactString;
use tokio::sync::mpsc;

pub enum PublicFile {
    DownloadArchive(String),
    DownloadUri(String),
    LocalPath(String),
}

pub struct Builder {
    pub bind_addr: IpAddr,
    pub bind_port: u16,

    pub app_name: CompactString,
    pub description: String,

    pub public_file: PublicFile,
    pub storage: Vec<config_it::Storage>,
    // TODO: Find out how to define authentication ...
    // TODO: Find out how to integrate with log system ...
    pub command_stream: Option<async_channel::Sender<String>>,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            app_name: "App".into(),
            bind_addr: Ipv4Addr::UNSPECIFIED.into(),
            bind_port: 15572,
            description: Default::default(),
            public_file: PublicFile::DownloadUri(
                "TODO: Publish files to github, hard code the link here.".into(),
            ),
            storage: Default::default(),
            command_stream: None,
        }
    }
}

pub struct BuildOutput {
    pub app: axum::Router,
    pub remote_terminal_input: mpsc::UnboundedReceiver<String>,
}

impl Builder {
    pub async fn build(mut self) -> BuildOutput {
        // let app = axum::Router::new().route("/api/system_info", get(api::sysinfo));

        todo!()
    }

    pub fn with_command_receiver(mut self, ch: async_channel::Sender<String>) -> Self {
        self.command_stream = Some(ch);
        self
    }
}

mod api {
    pub async fn sysinfo() {}
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
