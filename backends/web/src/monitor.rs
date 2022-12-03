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
