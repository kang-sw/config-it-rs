use std::{io, net::SocketAddr, num::NonZeroU16, path::Path};

use axum::{
    extract::Host,
    handler::HandlerWithoutStateExt,
    http::{StatusCode, Uri},
    response::Redirect,
    routing::{self},
    BoxError,
};
use axum_server::tls_rustls::RustlsConfig;
use compact_str::ToCompactString;
use tokio::select;
use tracing::{debug, info};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, EnvFilter};

#[derive(clap::Parser)]
struct Args {
    /// Defines current working directory. Default is current directory.
    #[arg(short = 'W', long, env = "CONFIG_IT_WORKING_DIR")]
    working_dir: Option<String>,

    /// Defines certificate path for HTTPS. If not specified, a temporary certificate will be
    /// generated under the working directory.
    #[arg(long, env = "CONFIG_IT_HTTPS_CERT_PATH")]
    https_cert_path: Option<String>,

    /// Defines private key path for HTTPS. If not specified, a temporary private key will be
    /// generated under the working directory.
    #[arg(long, env = "CONFIG_IT_HTTPS_KEY_PATH")]
    https_key_path: Option<String>,

    /// Port to serve HTTPS.
    #[arg(long, default_value_t = 10482, env = "CONFIG_IT_HTTPS_PORT")]
    https_port: u16,

    /// If set nonzero value, HTTP redirect will be enabled.
    #[arg(long, default_value = "10481", env = "CONFIG_IT_HTTP_PORT")]
    http_port: Option<NonZeroU16>,

    /// Disable file logging
    #[arg(long, env = "CONFIG_IT_NO_FILE_LOG")]
    no_file_log: bool,
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
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    let args = Args::get();
    if let Some(dir) = &args.working_dir {
        std::env::set_current_dir(dir).expect("this may not fail!");
    }

    let mut _file_logging_guard = None;
    {
        let subscriber = tracing_subscriber::fmt()
            .with_writer(io::stderr)
            .pretty()
            .with_env_filter(EnvFilter::from_default_env())
            .finish();

        let fwrite_layer = (!args.no_file_log).then(|| {
            use tracing_appender::*;
            let (writer, guard) = non_blocking(rolling::daily("log", "relay.log"));
            _file_logging_guard = Some(guard);
            tracing_subscriber::fmt::layer().with_ansi(false).compact().with_writer(writer)
        });

        tracing::subscriber::set_global_default(subscriber.with(fwrite_layer))
            .expect("failed to set global default subscriber");
    };

    debug!(working_dir = ?std::env::current_dir());

    // :: Setup HTTPS certification
    let config = {
        const HTTPS_CERT_PATH_DEFAULT: &str = "self_signed/cert.pem";
        const HTTPS_KEY_PATH_DEFAULT: &str = "self_signed/key.pem";

        let https_cert_path = args.https_cert_path.as_deref().unwrap_or(HTTPS_CERT_PATH_DEFAULT);
        let https_key_path = args.https_key_path.as_deref().unwrap_or(HTTPS_KEY_PATH_DEFAULT);

        match (Path::new(https_cert_path).exists(), Path::new(https_key_path).exists()) {
            (true, true) => info!(using_key_cert = ?[https_cert_path, https_key_path]),
            (false, false) => {
                std::fs::create_dir("self_signed").ok(); // just try to create ..
                prepare_self_signed_certs(https_cert_path, https_key_path)
            }

            _ => panic!(
                "HTTPS certificate and private key must be both specified or both not specified."
            ),
        }

        RustlsConfig::from_pem_file(https_cert_path, https_key_path)
            .await
            .expect("failed to load HTTPS certificate and private key")
    };

    let mut task_redirect_app = if let Some(http_port) = args.http_port {
        let https_port = args.https_port;
        let http_port = http_port.get();

        info!(http_port, https_port, "HTTP redirect enabled");
        tokio::spawn(redirect_http_to_https(http_port, https_port))
    } else {
        tokio::spawn(std::future::pending::<()>())
    };

    let task_wait_signal = async {
        let sig = tokio::signal::ctrl_c();

        #[cfg(unix)]
        let sig_nix = {
            let mut sig = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to register SIGTERM handler");

            async move { sig.recv().await }
        };

        #[cfg(not(unix))]
        let sig_nix = std::future::pending::<()>();

        select! {
            _ = sig => info!("Received SIGINT"),
            _ = sig_nix => info!("Received SIGTERM"),
        }
    };

    let state = config_it_access_relay_server::create_state();
    let app = config_it_access_relay_server::api::configure_api(state.clone())
        .route("/", routing::get(api::index))
        .route("/index.html", routing::get(api::index))
        .route("/:consume", routing::get(api::retrieve_resource))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], args.https_port));
    let mut task_serve_https =
        tokio::spawn(axum_server::bind_rustls(addr, config).serve(app.into_make_service()));

    // :: Run the server, until a signal is received.
    info!("Server started. Ctrl-C to finish.");
    select! {
        _ = task_wait_signal => (),
        x = &mut task_serve_https => panic!("something wrong with HTTPS server: {x:?}"),
        x = &mut task_redirect_app => panic!("something wrong with HTTP redirect task: {x:?}"),
    }
    info!("Exit signal received. Shutting down...");

    // Shutdown HTTPS server.
    task_serve_https.abort();
    let result = task_serve_https.await;
    debug!(?result, "HTTPS server task finished");

    // Shutdown HTTP redirect server.
    task_redirect_app.abort();
    let result = task_redirect_app.await;
    debug!(?result, "HTTP redirect task finished");
}

#[tracing::instrument]
fn prepare_self_signed_certs(cert_path: &str, key_path: &str) {
    use std::process::Command;

    info!("Generating self-signed certificate...");
    let output = Command::new("openssl")
        .args(&[
            "req",
            "-x509",
            "-newkey",
            "rsa:4096",
            "-keyout",
            key_path,
            "-out",
            cert_path,
            "-days",
            "365",
            "-nodes",
            "-subj",
            "/CN=localhost",
        ])
        .output()
        .expect("openssl must be installed");

    if !output.status.success() {
        panic!(
            "Failed to generate self-signed certificate: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    info!("Self-signed certificate generated.");
}

async fn redirect_http_to_https(http_port: u16, https_port: u16) {
    fn make_https(
        host: String,
        uri: Uri,
        http_port: &str,
        https_port: &str,
    ) -> Result<Uri, BoxError> {
        let mut parts = uri.into_parts();

        parts.scheme = Some(axum::http::uri::Scheme::HTTPS);

        if parts.path_and_query.is_none() {
            parts.path_and_query = Some("/".parse().unwrap());
        }

        let https_host = host.replace(&http_port.to_string(), &https_port.to_string());
        parts.authority = Some(https_host.parse()?);

        Ok(Uri::from_parts(parts)?)
    }

    let str_ports = (http_port.to_compact_string(), https_port.to_compact_string());
    let redirect = move |Host(host): Host, uri: Uri| async move {
        match make_https(host, uri, &str_ports.0, &str_ports.1) {
            Ok(uri) => Ok(Redirect::permanent(&uri.to_string())),
            Err(error) => {
                tracing::warn!(%error, "failed to convert URI to HTTPS");
                Err(StatusCode::BAD_REQUEST)
            }
        }
    };

    axum::Server::bind(&SocketAddr::from(([0, 0, 0, 0], http_port)))
        .serve(redirect.into_make_service())
        .await
        .unwrap();
}

mod api {
    use axum::{http::StatusCode, response::IntoResponse};

    pub async fn index() -> impl IntoResponse {
        "Hello, World!"
    }

    pub async fn retrieve_resource() -> impl IntoResponse {
        StatusCode::NOT_FOUND
    }
}
