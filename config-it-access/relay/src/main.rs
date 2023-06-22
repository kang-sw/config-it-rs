use std::sync::atomic::AtomicUsize;

use axum::{response::IntoResponse, routing::get, Router, Server};

#[derive(clap::Parser)]
struct Args {
    /// Defines current working directory. Default is current directory.
    #[arg(short='W', long, default_value_t=Into::into("."))]
    working_dir: String,

    /// Defines certificate path for HTTPS. If not specified, a temporary certificate will be
    /// generated under the working directory.
    #[arg(long)]
    https_cert_path: Option<String>,

    /// Defines private key path for HTTPS. If not specified, a temporary private key will be
    /// generated under the working directory.
    #[arg(long)]
    https_key_path: Option<String>,
}

impl Args {
    fn get() -> &'static Self {
        lazy_static::lazy_static!(
            static ref ARGS: Args = <Args as clap::Parser>::parse();
        );

        &ARGS
    }
}

#[tokio::main]
async fn main() {
    let args = Args::get();
    let app = Router::new().route("/hello", get(hello));
    Server::bind(&"0.0.0.0:5944".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
}

async fn hello() -> impl IntoResponse {
    static COUNT: AtomicUsize = AtomicUsize::new(0);
    COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    format!("Hello, world! (count: {})", COUNT.load(std::sync::atomic::Ordering::Relaxed))
}
