use std::io;

use crate::Result;
use crate::codec::CCtx;

pub(crate) fn sized_compress(dst: &mut Vec<u8>, src: &[u8], cctx: &mut CCtx) -> Result<()> {
    Ok(cctx.compress(dst, src)?)
}

pub(crate) fn extension_read<R: io::Read>(
    reader: &mut R,
    dst: &mut Vec<u8>,
    size: usize,
) -> Result<()> {
    dst.resize(size, 0);
    reader.read_exact(dst)?;
    Ok(())
}

pub(crate) fn slice_and_increment<'a>(offset: &mut usize, len: u64, bytes: &'a [u8]) -> &'a [u8] {
    let slice = &bytes[*offset..*offset + len as usize];
    *offset += len as usize;
    slice
}

/// Resize a vector to the target length without initializing new elements.
///
/// # Safety
/// The caller must ensure that all elements in the range [`old_len..new_len`]
/// are initialized before reading them. This is safe when immediately followed
/// by operations that write to the entire buffer (e.g., decompression).
#[inline]
#[allow(clippy::uninit_vec)]
pub(crate) fn resize_uninit<T>(vec: &mut Vec<T>, new_len: usize) {
    match new_len.cmp(&vec.len()) {
        std::cmp::Ordering::Greater => {
            // Growing: reserve and set length (unsafe but fast)
            vec.reserve(new_len - vec.len());
            unsafe {
                vec.set_len(new_len);
            }
        }
        std::cmp::Ordering::Less => {
            // Shrinking: truncate (safe and fast)
            vec.truncate(new_len);
        }
        std::cmp::Ordering::Equal => {
            // Same size: do nothing
        }
    }
}

pub(crate) fn calculate_offsets(values: &[u64], offsets: &mut Vec<u64>) {
    offsets.clear();
    offsets.push(0);
    for i in 1..values.len() {
        offsets.push(offsets[i - 1] + values[i - 1]);
    }
}

/// Build a `Range<usize>` from a u64 offset and length.
pub(crate) fn span(offset: u64, length: u64) -> std::ops::Range<usize> {
    offset as usize..(offset + length) as usize
}
