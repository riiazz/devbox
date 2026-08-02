use sha2::{Digest, Sha256};
use thiserror::Error;

/// A SHA-256 digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checksum(pub [u8; 32]);

impl Checksum {
    pub fn compute(data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(data);
        Checksum(hasher.finalize().into())
    }

    pub fn from_hex(input: &str) -> Result<Self, ChecksumParseError> {
        let hex = input.trim();
        if hex.len() != 64 {
            return Err(ChecksumParseError::Length(hex.len()));
        }
        let mut bytes = [0u8; 32];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
                .map_err(|_| ChecksumParseError::Invalid)?;
        }
        Ok(Checksum(bytes))
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

impl std::fmt::Display for Checksum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_hex())
    }
}

#[derive(Debug, Error)]
pub enum ChecksumParseError {
    #[error("expected 64 hex characters, got {0}")]
    Length(usize),
    #[error("invalid hex string")]
    Invalid,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_matches_known_vector() {
        let checksum = Checksum::compute(b"hello");
        assert_eq!(
            checksum.to_hex(),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn hex_round_trips() {
        let checksum = Checksum::compute(b"devbox");
        let parsed = Checksum::from_hex(&checksum.to_hex()).expect("parse hex");
        assert_eq!(parsed, checksum);
    }

    #[test]
    fn from_hex_accepts_whitespace() {
        let checksum = Checksum::from_hex(" 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824 ")
            .expect("parse hex");
        assert_eq!(checksum.to_hex(), "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
    }

    #[test]
    fn from_hex_rejects_bad_length() {
        assert!(matches!(
            Checksum::from_hex("abc"),
            Err(ChecksumParseError::Length(3))
        ));
    }

    #[test]
    fn from_hex_rejects_invalid_chars() {
        assert!(matches!(
            Checksum::from_hex("zz".repeat(32).as_str()),
            Err(ChecksumParseError::Invalid)
        ));
    }
}
