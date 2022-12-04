use std::net::{IpAddr, Ipv4Addr};

use config_it::CompactString;

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
    // TODO: Find out how to define authentication ...
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
        }
    }
}

impl Builder {
    pub async fn build(mut self) -> axum::Router {
        // TODO: Build a router

        todo!()
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
