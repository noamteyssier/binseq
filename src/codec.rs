//! Thin wrapper over the (pure-Rust) zstd backend so the rest of the crate never names it.
use std::io;

use mzstd::{Compressor, Decompressor, EncoderConfig, LdmParams, Level, LongDistanceMatching};

fn err(e: impl std::error::Error + Send + Sync + 'static) -> io::Error {
    io::Error::other(e)
}

/// Reusable compression context.
pub struct CCtx(Compressor);
impl CCtx {
    pub fn new(level: i32, ldm: bool) -> io::Result<Self> {
        let mut config = EncoderConfig::default().with_level(Level::try_from(level).map_err(err)?);
        if ldm {
            config = config
                .with_long_distance_matching(LongDistanceMatching::Enabled(LdmParams::default()));
        }
        Ok(Self(Compressor::new(config).map_err(err)?))
    }

    /// Replaces the contents of `dst` with the compressed `src`.
    pub fn compress(&mut self, dst: &mut Vec<u8>, src: &[u8]) -> io::Result<()> {
        dst.clear();
        self.0.compress_into(src, dst).map_err(err)?;
        Ok(())
    }
}

/// Reusable decompression context.
#[derive(Default)]
pub struct DCtx(Decompressor);
impl DCtx {
    /// Decompresses `src` into the front of `dst`, returning the number of bytes written.
    pub fn decompress(&mut self, dst: &mut [u8], src: &[u8]) -> io::Result<usize> {
        self.0.decompress_to_slice(src, dst).map_err(err)
    }

    /// Replaces the contents of `dst` with the decompressed `src`.
    pub fn decompress_vec(&mut self, dst: &mut Vec<u8>, src: &[u8]) -> io::Result<usize> {
        dst.clear();
        Ok(self.0.decompress_into(src, dst).map_err(err)?.len())
    }
}

/// One-shot compression into `dst` (replacing its contents).
pub fn encode(dst: &mut Vec<u8>, src: &[u8], level: i32) -> io::Result<()> {
    CCtx::new(level, false)?.compress(dst, src)
}

/// One-shot decompression into `dst` (replacing its contents).
pub fn decode(dst: &mut Vec<u8>, src: &[u8]) -> io::Result<()> {
    DCtx::default().decompress_vec(dst, src).map(drop)
}
