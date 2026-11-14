use std::fmt;
use std::str::FromStr;

use thiserror::Error;

use crate::message::MessageKind;

pub const DEFAULT_MAX_ATTACHMENT_SIZE: u64 = 2 * 1024 * 1024 * 1024;
pub const MAX_ATTACHMENT_NAME: usize = 255;
pub const MAX_MIME_LEN: usize = 127;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AttachmentHash([u8; 32]);

impl AttachmentHash {
    pub fn of(bytes: &[u8]) -> Self {
        Self(*blake3::hash(bytes).as_bytes())
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }
}

impl FromStr for AttachmentHash {
    type Err = AttachmentError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(AttachmentError::InvalidHash);
        }
        let mut bytes = [0_u8; 32];
        hex::decode_to_slice(value, &mut bytes).map_err(|_| AttachmentError::InvalidHash)?;
        Ok(Self(bytes))
    }
}

impl fmt::Display for AttachmentHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentRef {
    pub hash: AttachmentHash,
    pub name: String,
    pub size: u64,
    pub mime: String,
}

impl AttachmentRef {
    pub fn new(
        hash: AttachmentHash,
        name: impl Into<String>,
        size: u64,
        mime: impl Into<String>,
    ) -> Result<Self, AttachmentError> {
        let value = Self {
            hash,
            name: name.into(),
            size,
            mime: mime.into(),
        };
        value.validate(DEFAULT_MAX_ATTACHMENT_SIZE)?;
        Ok(value)
    }

    pub fn validate(&self, max_size: u64) -> Result<(), AttachmentError> {
        validate_name(&self.name)?;
        validate_mime(&self.mime)?;
        if self.size > max_size {
            return Err(AttachmentError::TooLarge {
                size: self.size,
                max: max_size,
            });
        }
        Ok(())
    }

    pub fn kind(&self) -> MessageKind {
        if self.mime.starts_with("image/") {
            MessageKind::Image
        } else if self.mime.starts_with("audio/") {
            MessageKind::Audio
        } else {
            MessageKind::File
        }
    }
}

pub fn validate_name(name: &str) -> Result<(), AttachmentError> {
    if name.is_empty() || name.len() > MAX_ATTACHMENT_NAME {
        return Err(AttachmentError::InvalidName);
    }
    if name == "." || name == ".." {
        return Err(AttachmentError::InvalidName);
    }
    if name
        .chars()
        .any(|character| character.is_control() || matches!(character, '/' | '\\' | '\0'))
    {
        return Err(AttachmentError::InvalidName);
    }
    Ok(())
}

pub fn validate_mime(mime: &str) -> Result<(), AttachmentError> {
    if mime.is_empty() || mime.len() > MAX_MIME_LEN || mime.bytes().any(|byte| !byte.is_ascii()) {
        return Err(AttachmentError::InvalidMime);
    }
    let Some((kind, subtype)) = mime.split_once('/') else {
        return Err(AttachmentError::InvalidMime);
    };
    if kind.is_empty() || subtype.is_empty() || subtype.contains('/') {
        return Err(AttachmentError::InvalidMime);
    }
    let token = |value: &str| {
        value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'!' | b'#'
                        | b'$'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        })
    };
    if !token(kind) || !token(subtype) {
        return Err(AttachmentError::InvalidMime);
    }
    Ok(())
}

pub fn mime_from_name(name: &str) -> &'static str {
    let extension = name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase());
    match extension.as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("mp3") => "audio/mpeg",
        Some("ogg" | "oga") => "audio/ogg",
        Some("opus") => "audio/opus",
        Some("wav") => "audio/wav",
        Some("flac") => "audio/flac",
        Some("mp4" | "m4a") => "audio/mp4",
        Some("txt") => "text/plain",
        Some("pdf") => "application/pdf",
        Some("zip") => "application/zip",
        _ => "application/octet-stream",
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AttachmentError {
    #[error("attachment hash is invalid")]
    InvalidHash,

    #[error("attachment name is invalid")]
    InvalidName,

    #[error("attachment MIME type is invalid")]
    InvalidMime,

    #[error("attachment size {size} exceeds the maximum of {max} bytes")]
    TooLarge { size: u64, max: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_round_trips_and_hashes_content() {
        let hash = AttachmentHash::of(b"hello");
        assert_eq!(hash.to_hex().len(), 64);
        assert_eq!(
            hash.to_string().parse::<AttachmentHash>().expect("parse"),
            hash
        );
        assert_ne!(AttachmentHash::of(b"hello"), AttachmentHash::of(b"world"));
        assert!("xyz".parse::<AttachmentHash>().is_err());
        assert!(
            format!("{}0", hash.to_hex())
                .parse::<AttachmentHash>()
                .is_err()
        );
    }

    #[test]
    fn names_reject_traversal_and_control_characters() {
        for name in [
            "",
            ".",
            "..",
            "a/b",
            "a\\b",
            "a\0b",
            "a\nb",
            &"x".repeat(256),
        ] {
            assert!(validate_name(name).is_err(), "{name:?}");
        }
        assert!(validate_name("holiday photo.png").is_ok());
    }

    #[test]
    fn mime_requires_type_and_subtype() {
        assert!(validate_mime("image/png").is_ok());
        assert!(validate_mime("application/octet-stream").is_ok());
        for mime in [
            "",
            "image",
            "image/",
            "/png",
            "image/png/extra",
            "image/p ng",
        ] {
            assert!(validate_mime(mime).is_err(), "{mime:?}");
        }
    }

    #[test]
    fn attachment_ref_validates_size_and_kind() {
        let image = AttachmentRef::new(AttachmentHash::of(b"image"), "photo.png", 12, "image/png")
            .expect("image");
        assert_eq!(image.kind(), MessageKind::Image);

        let audio = AttachmentRef::new(AttachmentHash::of(b"audio"), "clip.ogg", 12, "audio/ogg")
            .expect("audio");
        assert_eq!(audio.kind(), MessageKind::Audio);

        let file = AttachmentRef::new(
            AttachmentHash::of(b"file"),
            "report.pdf",
            12,
            "application/pdf",
        )
        .expect("file");
        assert_eq!(file.kind(), MessageKind::File);

        let oversized = AttachmentRef {
            hash: AttachmentHash::of(b"x"),
            name: "big.bin".to_owned(),
            size: 100,
            mime: "application/octet-stream".to_owned(),
        };
        assert!(matches!(
            oversized.validate(10),
            Err(AttachmentError::TooLarge { .. })
        ));
    }

    #[test]
    fn extension_mime_is_stable() {
        assert_eq!(mime_from_name("photo.PNG"), "image/png");
        assert_eq!(mime_from_name("clip.opus"), "audio/opus");
        assert_eq!(mime_from_name("unknown"), "application/octet-stream");
    }
}
