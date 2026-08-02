use std::fs;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use thiserror::Error;
use zip::ZipArchive;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    Zip,
    TarGz,
}

#[derive(Debug, Error)]
pub enum ExtractError {
    #[error("failed to read archive: {0}")]
    Io(#[from] io::Error),
    #[error("zip archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("failed to create `{path}`: {source}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to write `{path}`: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("unsafe archive entry `{path}`")]
    UnsafePath { path: String },
}

pub fn extract(bytes: &[u8], format: ArchiveFormat, dest: &Path) -> Result<(), ExtractError> {
    fs::create_dir_all(dest).map_err(|source| ExtractError::CreateDir {
        path: dest.to_path_buf(),
        source,
    })?;
    match format {
        ArchiveFormat::Zip => extract_zip(bytes, dest),
        ArchiveFormat::TarGz => extract_tar_gz(bytes, dest),
    }
}

fn extract_zip(bytes: &[u8], dest: &Path) -> Result<(), ExtractError> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let out_path = sanitize(entry.name(), dest)?;
        if entry.is_dir() {
            fs::create_dir_all(&out_path).map_err(|source| ExtractError::CreateDir {
                path: out_path.clone(),
                source,
            })?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).map_err(|source| ExtractError::CreateDir {
                    path: parent.to_path_buf(),
                    source,
                })?;
            }
            let mut out = fs::File::create(&out_path).map_err(|source| ExtractError::Write {
                path: out_path.clone(),
                source,
            })?;
            io::copy(&mut entry, &mut out).map_err(|source| ExtractError::Write {
                path: out_path.clone(),
                source,
            })?;
        }
    }
    Ok(())
}

fn extract_tar_gz(bytes: &[u8], dest: &Path) -> Result<(), ExtractError> {
    let decoder = GzDecoder::new(Cursor::new(bytes));
    let mut archive = tar::Archive::new(decoder);
    for entry in archive.entries()? {
        // unpack_in refuses paths that escape `dest`.
        entry?.unpack_in(dest)?;
    }
    Ok(())
}

/// Maps an archive entry path into `dest`, rejecting traversal.
fn sanitize(name: &str, dest: &Path) -> Result<PathBuf, ExtractError> {
    let mut path = PathBuf::new();
    for component in name.replace('\\', "/").split('/') {
        match component {
            "" | "." => {}
            ".." => return Err(ExtractError::UnsafePath { path: name.to_string() }),
            part => path.push(part),
        }
    }
    Ok(dest.join(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicU32, Ordering};

    use flate2::write::GzEncoder;
    use flate2::Compression;
    use zip::write::SimpleFileOptions;

    static NEXT: AtomicU32 = AtomicU32::new(0);

    fn temp_dir() -> PathBuf {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let mut dir = std::env::temp_dir();
        dir.push(format!("devbox-extract-{}-{}", std::process::id(), n));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn zip_fixture() -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        writer.start_file("rg.exe", options).expect("start file");
        writer.write_all(b"fake binary").expect("write file");
        writer
            .add_directory("docs", options)
            .expect("add directory");
        writer.start_file("docs/README.md", options).expect("start doc");
        writer.write_all(b"ripgrep").expect("write doc");
        writer.finish().expect("finish zip").into_inner()
    }

    fn tar_gz_fixture() -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        {
            let mut builder = tar::Builder::new(&mut encoder);
            let mut header = tar::Header::new_gnu();
            header.set_size(b"fake binary".len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            builder
                .append_data(&mut header, "rg", &b"fake binary"[..])
                .expect("append file");
            builder.finish().expect("finish tar");
        }
        encoder.finish().expect("finish gzip")
    }

    #[test]
    fn extracts_zip_into_dest() {
        let root = temp_dir();
        let dest = root.join("out");
        extract(&zip_fixture(), ArchiveFormat::Zip, &dest).expect("extract zip");
        assert!(dest.join("rg.exe").is_file());
        assert!(dest.join("docs").is_dir());
        assert!(dest.join("docs/README.md").is_file());
        assert_eq!(fs::read_to_string(dest.join("docs/README.md")).unwrap(), "ripgrep");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn extracts_tar_gz_into_dest() {
        let root = temp_dir();
        let dest = root.join("out");
        extract(&tar_gz_fixture(), ArchiveFormat::TarGz, &dest).expect("extract tar.gz");
        assert_eq!(fs::read_to_string(dest.join("rg")).unwrap(), "fake binary");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_path_traversal() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer.start_file("../evil.txt", SimpleFileOptions::default()).expect("start");
        writer.write_all(b"bad").expect("write");
        let bytes = writer.finish().expect("finish").into_inner();

        let root = temp_dir();
        let err = extract(&bytes, ArchiveFormat::Zip, &root.join("out")).expect_err("traversal");
        assert!(matches!(err, ExtractError::UnsafePath { .. }));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_corrupt_zip() {
        let err = extract(b"not a zip", ArchiveFormat::Zip, &temp_dir().join("out"))
            .expect_err("corrupt zip");
        assert!(matches!(err, ExtractError::Zip(_)));
    }
}
