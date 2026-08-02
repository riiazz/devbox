use thiserror::Error;

use crate::checksum::Checksum;

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("failed to download `{url}`: {source}")]
    Request {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("checksum mismatch for `{url}`: expected {expected}, got {actual}")]
    ChecksumMismatch { url: String, expected: String, actual: String },
}

pub fn fetch(client: &reqwest::blocking::Client, url: &str) -> Result<Vec<u8>, DownloadError> {
    let response = client
        .get(url)
        .send()
        .map_err(|source| DownloadError::Request {
            url: url.to_string(),
            source,
        })?
        .error_for_status()
        .map_err(|source| DownloadError::Request {
            url: url.to_string(),
            source,
        })?;
    let bytes = response
        .bytes()
        .map_err(|source| DownloadError::Request {
            url: url.to_string(),
            source,
        })?
        .to_vec();
    Ok(bytes)
}

pub fn verify(data: &[u8], expected: &Checksum, url: &str) -> Result<(), DownloadError> {
    let actual = Checksum::compute(data);
    if actual != *expected {
        return Err(DownloadError::ChecksumMismatch {
            url: url.to_string(),
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checksum::Checksum;

    #[test]
    fn verify_accepts_matching_checksum() {
        let data = b"archive bytes";
        let expected = Checksum::compute(data);
        verify(data, &expected, "https://example.com/archive.zip").expect("verify");
    }

    #[test]
    fn verify_rejects_mismatch() {
        let data = b"archive bytes";
        let expected = Checksum::compute(b"different bytes");
        let err = verify(data, &expected, "https://example.com/archive.zip").expect_err("verify");
        assert!(matches!(err, DownloadError::ChecksumMismatch { .. }));
    }
}
