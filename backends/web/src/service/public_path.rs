use std::path::Path;

///
///
/// Defines access method to public files.
///
pub enum PublicPath {
    /// Path to specific directory which contains `index.html`
    Path(Box<Path>),

    /// A Uri which can download archive of public files.
    ///
    /// To make use of this, system must have `wget` or `curl`. Optionally requires
    /// `tar` or `unzip` command for corresponding archive types.
    ///
    /// Once Uri `PublicPath` is instantiated, it will download the archive and extract it
    /// based on the temporary directory, which is generated based on the archive checksum.
    ArchiveDownloadUri(String),

    /// Local path to archive file. After finding the archive, it works the same as
    /// `ArchiveDownloadUri`.
    ArchivePath(Box<Path>),
}

impl Default for PublicPath {
    fn default() -> Self {
        Self::Path(Path::new("static").into())
    }
}
