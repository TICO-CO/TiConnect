use std::fmt;

use thiserror::Error;

pub const MAX_CANONICAL_BYTES: usize = 64 * 1024;
pub const MAX_DOMAIN_LEN: usize = 64;

const PREFIX: &[u8] = b"TiConnect canonical v1";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CanonicalError {
    #[error("canonical encoding exceeds {MAX_CANONICAL_BYTES} bytes")]
    TooLarge,

    #[error("canonical domain exceeds {MAX_DOMAIN_LEN} bytes")]
    DomainTooLong,

    #[error("canonical domain does not match")]
    DomainMismatch,

    #[error("canonical encoding is truncated")]
    Truncated,

    #[error("canonical encoding has trailing bytes")]
    TrailingBytes,

    #[error("canonical length does not fit the target type")]
    LengthOverflow,

    #[error("canonical string is not valid UTF-8")]
    InvalidUtf8,
}

#[derive(Debug, Clone, Default)]
pub struct Encoder {
    bytes: Vec<u8>,
}

impl Encoder {
    pub fn new(domain: &str) -> Result<Self, CanonicalError> {
        if domain.len() > MAX_DOMAIN_LEN {
            return Err(CanonicalError::DomainTooLong);
        }

        let mut encoder = Self::default();
        encoder.bytes.extend_from_slice(PREFIX);
        encoder.write_len(domain.len())?;
        encoder.bytes.extend_from_slice(domain.as_bytes());
        Ok(encoder)
    }

    pub fn bytes(&mut self, value: &[u8]) -> Result<(), CanonicalError> {
        self.write_len(value.len())?;
        self.reserve(value.len())?;
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    pub fn string(&mut self, value: &str) -> Result<(), CanonicalError> {
        self.bytes(value.as_bytes())
    }

    pub fn u8(&mut self, value: u8) -> Result<(), CanonicalError> {
        self.reserve(1)?;
        self.bytes.push(value);
        Ok(())
    }

    pub fn u16(&mut self, value: u16) -> Result<(), CanonicalError> {
        self.raw(&value.to_be_bytes())
    }

    pub fn u32(&mut self, value: u32) -> Result<(), CanonicalError> {
        self.raw(&value.to_be_bytes())
    }

    pub fn u64(&mut self, value: u64) -> Result<(), CanonicalError> {
        self.raw(&value.to_be_bytes())
    }

    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn raw(&mut self, value: &[u8]) -> Result<(), CanonicalError> {
        self.reserve(value.len())?;
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn write_len(&mut self, length: usize) -> Result<(), CanonicalError> {
        let length = u32::try_from(length).map_err(|_| CanonicalError::TooLarge)?;
        self.raw(&length.to_be_bytes())
    }

    fn reserve(&self, additional: usize) -> Result<(), CanonicalError> {
        let total = self
            .bytes
            .len()
            .checked_add(additional)
            .ok_or(CanonicalError::TooLarge)?;
        if total > MAX_CANONICAL_BYTES {
            return Err(CanonicalError::TooLarge);
        }
        Ok(())
    }
}

pub struct Decoder<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Decoder<'a> {
    pub fn new(domain: &str, bytes: &'a [u8]) -> Result<Self, CanonicalError> {
        if bytes.len() > MAX_CANONICAL_BYTES {
            return Err(CanonicalError::TooLarge);
        }

        let mut decoder = Self { bytes, cursor: 0 };
        let prefix = decoder.take(PREFIX.len())?;
        if prefix != PREFIX {
            return Err(CanonicalError::DomainMismatch);
        }
        let length = decoder.length()?;
        if length > MAX_DOMAIN_LEN {
            return Err(CanonicalError::DomainTooLong);
        }
        let decoded = decoder.take(length)?;
        if decoded != domain.as_bytes() {
            return Err(CanonicalError::DomainMismatch);
        }
        Ok(decoder)
    }

    pub fn bytes(&mut self) -> Result<&'a [u8], CanonicalError> {
        let length = self.length()?;
        self.take(length)
    }

    pub fn string(&mut self) -> Result<&'a str, CanonicalError> {
        std::str::from_utf8(self.bytes()?).map_err(|_| CanonicalError::InvalidUtf8)
    }

    pub fn u8(&mut self) -> Result<u8, CanonicalError> {
        Ok(self.take(1)?[0])
    }

    pub fn u16(&mut self) -> Result<u16, CanonicalError> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    pub fn u32(&mut self) -> Result<u32, CanonicalError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    pub fn u64(&mut self) -> Result<u64, CanonicalError> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    pub fn finish(self) -> Result<(), CanonicalError> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(CanonicalError::TrailingBytes)
        }
    }

    fn length(&mut self) -> Result<usize, CanonicalError> {
        let length = self.u32()? as usize;
        if length > MAX_CANONICAL_BYTES {
            return Err(CanonicalError::TooLarge);
        }
        Ok(length)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], CanonicalError> {
        self.take(N)?
            .try_into()
            .map_err(|_| CanonicalError::Truncated)
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], CanonicalError> {
        let end = self
            .cursor
            .checked_add(length)
            .ok_or(CanonicalError::LengthOverflow)?;
        if end > self.bytes.len() {
            return Err(CanonicalError::Truncated);
        }
        let slice = &self.bytes[self.cursor..end];
        self.cursor = end;
        Ok(slice)
    }
}

impl fmt::Debug for Decoder<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Decoder")
            .field("cursor", &self.cursor)
            .field("length", &self.bytes.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding_is_deterministic() {
        let first = sample();
        let second = sample();

        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
    }

    #[test]
    fn round_trip_preserves_fields() {
        let bytes = sample();
        let mut decoder = Decoder::new("pairing", &bytes).expect("decoder");

        assert_eq!(decoder.u16().expect("version"), 1);
        assert_eq!(decoder.u8().expect("mode"), 2);
        assert_eq!(decoder.bytes().expect("token"), b"abcdef");
        assert_eq!(decoder.string().expect("node"), "node-1");
        assert_eq!(decoder.u64().expect("expiry"), 42);
        decoder.finish().expect("finished");
    }

    #[test]
    fn rejects_wrong_domain() {
        let bytes = sample();
        assert!(matches!(
            Decoder::new("wire", &bytes),
            Err(CanonicalError::DomainMismatch)
        ));
    }

    #[test]
    fn rejects_truncated_and_trailing_bytes() {
        let bytes = sample();
        let mut truncated = Decoder::new("pairing", &bytes[..bytes.len() - 1]).expect("decoder");
        truncated.u16().expect("version");
        truncated.u8().expect("mode");
        truncated.bytes().expect("token");
        truncated.string().expect("node");
        assert!(matches!(truncated.u64(), Err(CanonicalError::Truncated)));

        let mut extended = bytes;
        extended.push(0);
        let mut decoder = Decoder::new("pairing", &extended).expect("decoder");
        decoder.u16().expect("version");
        decoder.u8().expect("mode");
        decoder.bytes().expect("token");
        decoder.string().expect("node");
        decoder.u64().expect("expiry");
        assert!(matches!(
            decoder.finish(),
            Err(CanonicalError::TrailingBytes)
        ));
    }

    #[test]
    fn rejects_oversized_field() {
        let mut encoder = Encoder::new("pairing").expect("encoder");
        let too_large = vec![0_u8; MAX_CANONICAL_BYTES];
        assert!(matches!(
            encoder.bytes(&too_large),
            Err(CanonicalError::TooLarge)
        ));
    }

    fn sample() -> Vec<u8> {
        let mut encoder = Encoder::new("pairing").expect("encoder");
        encoder.u16(1).expect("version");
        encoder.u8(2).expect("mode");
        encoder.bytes(b"abcdef").expect("token");
        encoder.string("node-1").expect("node");
        encoder.u64(42).expect("expiry");
        encoder.finish()
    }
}
