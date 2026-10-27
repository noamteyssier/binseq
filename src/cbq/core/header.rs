use std::fmt::Display;

use bytemuck::{Pod, Zeroable};

use crate::{Result, error::CbqError};

use super::{DEFAULT_BLOCK_SIZE, DEFAULT_COMPRESSION_LEVEL, FILE_MAGIC, FILE_VERSION};

/// Records are paired
pub const PRESENCE_PAIRED: u64 = 1 << 0;

/// Records have quality scores
pub const PRESENCE_QUALITIES: u64 = 1 << 1;

/// Records have headers
pub const PRESENCE_HEADERS: u64 = 1 << 2;

/// Records have flags
pub const PRESENCE_FLAGS: u64 = 1 << 3;

/// The file header for a CBQ file.
///
/// This is stored identically in memory and on disk.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Zeroable, Pod)]
#[repr(C)]
pub struct FileHeader {
    // File Type Metadata (8 bytes)
    /// File magic number
    magic: [u8; 7],
    /// File version number
    pub version: u8,

    // Data presence flags (8 bytes)
    /// A bitfield indicating which data fields are present in the file
    pub presence_flags: u64,

    // Configuration (16 bytes)
    /// zstd compression level (negative values select zstd's fast modes)
    pub compression_level: i32,
    /// Upper half of the original `u64` compression level field (always zero)
    compression_level_pad: [u8; 4],
    /// block size in bytes
    pub block_size: u64,

    /// Reserved for future use
    reserved: [u8; 32],
}
impl Default for FileHeader {
    fn default() -> Self {
        let mut header = Self {
            magic: *FILE_MAGIC,
            version: FILE_VERSION,
            presence_flags: 0,
            compression_level: DEFAULT_COMPRESSION_LEVEL,
            compression_level_pad: [0; 4],
            block_size: DEFAULT_BLOCK_SIZE,
            reserved: [0; 32],
        };
        header.set_headers();
        header.set_qualities();
        header
    }
}

/// Flag getters and setters
impl FileHeader {
    pub fn set_paired(&mut self) {
        self.presence_flags |= PRESENCE_PAIRED;
    }
    pub fn set_qualities(&mut self) {
        self.presence_flags |= PRESENCE_QUALITIES;
    }
    pub fn set_headers(&mut self) {
        self.presence_flags |= PRESENCE_HEADERS;
    }
    pub fn set_flags(&mut self) {
        self.presence_flags |= PRESENCE_FLAGS;
    }

    #[inline]
    #[must_use]
    pub fn is_paired(&self) -> bool {
        self.presence_flags & PRESENCE_PAIRED != 0
    }
    #[inline]
    #[must_use]
    pub fn has_qualities(&self) -> bool {
        self.presence_flags & PRESENCE_QUALITIES != 0
    }
    #[inline]
    #[must_use]
    pub fn has_headers(&self) -> bool {
        self.presence_flags & PRESENCE_HEADERS != 0
    }
    #[inline]
    #[must_use]
    pub fn has_flags(&self) -> bool {
        self.presence_flags & PRESENCE_FLAGS != 0
    }
}

impl Display for FileHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CBQ {{ version: {}, paired: {}, qualities: {}, headers: {}, flags: {}, block_size: {}, compression: {} }}",
            self.version,
            self.is_paired(),
            self.has_qualities(),
            self.has_headers(),
            self.has_flags(),
            self.block_size,
            self.compression_level,
        )
    }
}

impl FileHeader {
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(self)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let header: Self = *bytemuck::from_bytes(bytes);
        if header.magic != *FILE_MAGIC {
            return Err(CbqError::InvalidFileHeaderMagic.into());
        }
        if header.version > FILE_VERSION {
            return Err(CbqError::UnsupportedFileVersion {
                found: header.version,
                supported: FILE_VERSION,
            }
            .into());
        }
        Ok(header)
    }
}

/// Builder for a [`FileHeader`](crate::cbq::FileHeader).
///
/// Defaults: unpaired, headers and qualities on, flags off,
/// [`DEFAULT_BLOCK_SIZE`](crate::cbq::DEFAULT_BLOCK_SIZE), and
/// [`DEFAULT_COMPRESSION_LEVEL`](crate::cbq::DEFAULT_COMPRESSION_LEVEL).
#[derive(Clone, Copy, Debug)]
pub struct FileHeaderBuilder {
    header: FileHeader,
}

impl Default for FileHeaderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl FileHeaderBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            header: FileHeader::default(),
        }
    }

    /// zstd compression level (negative values select zstd's fast modes)
    #[must_use]
    pub fn compression_level(mut self, level: i32) -> Self {
        self.header.compression_level = level;
        self
    }

    /// Target uncompressed block size in bytes
    #[must_use]
    pub fn block_size(mut self, block_size: usize) -> Self {
        self.header.block_size = block_size as u64;
        self
    }

    #[must_use]
    pub fn paired(self, paired: bool) -> Self {
        self.presence(PRESENCE_PAIRED, paired)
    }

    #[must_use]
    pub fn flags(self, flags: bool) -> Self {
        self.presence(PRESENCE_FLAGS, flags)
    }

    #[must_use]
    pub fn headers(self, headers: bool) -> Self {
        self.presence(PRESENCE_HEADERS, headers)
    }

    #[must_use]
    pub fn qualities(self, qualities: bool) -> Self {
        self.presence(PRESENCE_QUALITIES, qualities)
    }

    fn presence(mut self, bit: u64, on: bool) -> Self {
        if on {
            self.header.presence_flags |= bit;
        } else {
            self.header.presence_flags &= !bit;
        }
        self
    }

    #[must_use]
    pub fn build(self) -> FileHeader {
        self.header
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;

    #[test]
    fn test_from_bytes_rejects_newer_version() {
        let mut header = FileHeader::default();
        assert!(FileHeader::from_bytes(header.as_bytes()).is_ok());

        header.version = FILE_VERSION + 1;
        assert!(matches!(
            FileHeader::from_bytes(header.as_bytes()),
            Err(Error::CbqError(CbqError::UnsupportedFileVersion { .. }))
        ));
    }

    #[test]
    fn test_compression_level_layout_matches_u64() {
        // The on-disk field was a little-endian u64; the i32 must occupy its low bytes.
        assert_eq!(size_of::<FileHeader>(), 64);
        let header = FileHeaderBuilder::new().compression_level(-5).build();
        let bytes = header.as_bytes();
        assert_eq!(&bytes[16..20], &(-5i32).to_le_bytes());
        assert_eq!(&bytes[20..24], &[0; 4]);
        assert_eq!(FileHeader::from_bytes(bytes).unwrap().compression_level, -5);
    }

    #[test]
    fn test_builder_defaults_and_toggles() {
        let header = FileHeaderBuilder::new().build();
        assert_eq!(header, FileHeader::default());
        assert!(header.has_headers() && header.has_qualities());
        assert!(!header.is_paired() && !header.has_flags());

        let header = FileHeaderBuilder::new()
            .paired(true)
            .flags(true)
            .headers(false)
            .qualities(false)
            .build();
        assert!(header.is_paired() && header.has_flags());
        assert!(!header.has_headers() && !header.has_qualities());
    }
}
