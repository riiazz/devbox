pub mod checksum;
pub mod download;
pub mod extract;
pub mod installer;
pub mod resolve;
pub mod target;

pub use checksum::{Checksum, ChecksumParseError};
pub use download::DownloadError;
pub use extract::{extract, ArchiveFormat, ExtractError};
pub use installer::{InstallError, Installer};
pub use resolve::{resolve_source, GitHubRepo, Manifest, ResolveError, Source, ToolSpec};
pub use target::Target;
