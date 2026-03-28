//! Binary `.vec` format for storing embedding vectors alongside SRT records.
//!
//! Format:
//! ```text
//! Header (16 bytes):
//!   magic:    [u8; 4]   = b"SVEC"
//!   version:  u16       = 1  (little-endian)
//!   dims:     u16       = 384 (little-endian)
//!   hash_len: u32       = 64  (little-endian, SHA-256 hex string length)
//!   reserved: u32       = 0   (little-endian)
//!
//! Body:
//!   content_hash: [u8; hash_len]     = SHA-256 hex string (UTF-8)
//!   model_name:   null-terminated    = e.g. "BAAI/bge-small-en-v1.5\0"
//!   vector:       [f32; dims]        = IEEE 754 little-endian
//! ```

use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};
use memmap2::Mmap;

const MAGIC: &[u8; 4] = b"SVEC";
const VERSION: u16 = 1;
const HEADER_SIZE: usize = 16;

/// Owned embedding data read from a `.vec` file.
#[derive(Debug, Clone)]
pub struct VecData {
    pub model: String,
    pub content_hash: String,
    pub vector: Vec<f32>,
}

/// Memory-mapped embedding data that borrows the vector slice from the mmap.
pub struct VecMmap {
    _mmap: Mmap,
    pub model: String,
    pub content_hash: String,
    pub dims: usize,
    vector_offset: usize,
}

impl VecMmap {
    /// Access the embedding vector as a slice of f32.
    ///
    /// # Safety
    /// The offset and length were verified during construction in `read_vec_mmap`.
    /// The mmap is kept alive by `_mmap` so the slice remains valid.
    pub fn vector(&self) -> &[f32] {
        let bytes = &self._mmap[self.vector_offset..];
        unsafe {
            std::slice::from_raw_parts(bytes.as_ptr() as *const f32, self.dims)
        }
    }
}

/// Write an embedding vector to a `.vec` file.
pub fn write_vec(
    path: &Path,
    model: &str,
    content_hash: &str,
    vector: &[f32],
) -> Result<()> {
    let hash_bytes = content_hash.as_bytes();
    let hash_len = hash_bytes.len() as u32;
    let dims = vector.len() as u16;
    let model_bytes = model.as_bytes();

    // Calculate total size
    let body_size = hash_len as usize + model_bytes.len() + 1 + (vector.len() * 4);
    let total_size = HEADER_SIZE + body_size;

    let mut buf: Vec<u8> = Vec::with_capacity(total_size);

    // Header
    buf.write_all(MAGIC)?;
    buf.write_all(&VERSION.to_le_bytes())?;
    buf.write_all(&dims.to_le_bytes())?;
    buf.write_all(&hash_len.to_le_bytes())?;
    buf.write_all(&0u32.to_le_bytes())?; // reserved

    // Body: content_hash
    buf.write_all(hash_bytes)?;

    // Body: null-terminated model name + padding to 4-byte alignment
    buf.write_all(model_bytes)?;
    buf.write_all(&[0u8])?;

    // Pad to 4-byte alignment for f32 vector
    let current_len = buf.len();
    let padding = (4 - (current_len % 4)) % 4;
    for _ in 0..padding {
        buf.write_all(&[0u8])?;
    }

    // Body: vector as little-endian f32
    for &val in vector {
        buf.write_all(&val.to_le_bytes())?;
    }

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, &buf).with_context(|| format!("writing vec file: {}", path.display()))?;
    Ok(())
}

/// Read an embedding vector from a `.vec` file. Returns `None` if the file does not exist.
pub fn read_vec(path: &Path) -> Result<Option<VecData>> {
    if !path.exists() {
        return Ok(None);
    }

    let data = fs::read(path).with_context(|| format!("reading vec file: {}", path.display()))?;
    if data.len() < HEADER_SIZE {
        bail!("vec file too small: {}", path.display());
    }

    // Parse header
    let magic = &data[0..4];
    if magic != MAGIC {
        bail!("invalid magic in vec file: {}", path.display());
    }
    let dims = u16::from_le_bytes([data[6], data[7]]) as usize;
    let hash_len = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;

    // Parse body
    let hash_start = HEADER_SIZE;
    let hash_end = hash_start + hash_len;
    let content_hash = std::str::from_utf8(&data[hash_start..hash_end])?.to_string();

    // Find null-terminated model name
    let model_start = hash_end;
    let model_end = data[model_start..]
        .iter()
        .position(|&b| b == 0)
        .map(|p| model_start + p)
        .ok_or_else(|| anyhow::anyhow!("no null terminator for model: {}", path.display()))?;
    let model = std::str::from_utf8(&data[model_start..model_end])?.to_string();

    // Skip padding to 4-byte alignment
    let after_null = model_end + 1;
    let vec_start = after_null + (4 - (after_null % 4)) % 4;

    // Parse vector
    let mut vector = Vec::with_capacity(dims);
    for i in 0..dims {
        let offset = vec_start + i * 4;
        if offset + 4 > data.len() {
            bail!("vec file truncated at vector data: {}", path.display());
        }
        vector.push(f32::from_le_bytes([
            data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
        ]));
    }

    Ok(Some(VecData { model, content_hash, vector }))
}

/// Read an embedding vector using memory mapping. Returns `None` if the file does not exist.
///
/// The vector data is not copied — it is accessed directly from the mmap.
pub fn read_vec_mmap(path: &Path) -> Result<Option<VecMmap>> {
    if !path.exists() {
        return Ok(None);
    }

    let file =
        fs::File::open(path).with_context(|| format!("opening vec file: {}", path.display()))?;

    // Safety: we only read from the mmap and keep it alive in VecMmap.
    let mmap = unsafe { Mmap::map(&file)? };

    if mmap.len() < HEADER_SIZE {
        bail!("vec file too small for header: {}", path.display());
    }

    // Parse header
    let mut cursor = Cursor::new(&mmap[..HEADER_SIZE]);
    let mut magic = [0u8; 4];
    cursor.read_exact(&mut magic)?;
    if &magic != MAGIC {
        bail!("invalid magic bytes in vec file: {}", path.display());
    }

    let mut ver_buf = [0u8; 2];
    cursor.read_exact(&mut ver_buf)?;
    let version = u16::from_le_bytes(ver_buf);
    if version != VERSION {
        bail!(
            "unsupported vec file version {}, expected {}",
            version,
            VERSION
        );
    }

    let mut dims_buf = [0u8; 2];
    cursor.read_exact(&mut dims_buf)?;
    let dims = u16::from_le_bytes(dims_buf) as usize;

    let mut hash_len_buf = [0u8; 4];
    cursor.read_exact(&mut hash_len_buf)?;
    let hash_len = u32::from_le_bytes(hash_len_buf) as usize;

    // Skip reserved
    let mut _reserved = [0u8; 4];
    cursor.read_exact(&mut _reserved)?;

    // Parse body
    let body_start = HEADER_SIZE;
    if mmap.len() < body_start + hash_len {
        bail!("vec file truncated at content_hash: {}", path.display());
    }

    let content_hash =
        std::str::from_utf8(&mmap[body_start..body_start + hash_len])?.to_string();

    // Find null-terminated model name
    let model_start = body_start + hash_len;
    let null_pos = mmap[model_start..]
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| anyhow::anyhow!("no null terminator for model name: {}", path.display()))?;
    let model = std::str::from_utf8(&mmap[model_start..model_start + null_pos])?.to_string();

    // Skip past null terminator and any padding to 4-byte alignment
    let after_null = model_start + null_pos + 1;
    let vector_offset = after_null + (4 - (after_null % 4)) % 4;
    let expected_end = vector_offset + dims * 4;

    if mmap.len() < expected_end {
        bail!(
            "vec file truncated at vector data: expected {} bytes, got {}",
            expected_end,
            mmap.len()
        );
    }

    // Verify alignment: f32 requires 4-byte alignment. Mmap base is page-aligned,
    // but vector_offset may not be 4-byte aligned. We check here and bail if not.
    if vector_offset % std::mem::align_of::<f32>() != 0 {
        bail!(
            "vector data at offset {} is not f32-aligned in {}",
            vector_offset,
            path.display()
        );
    }

    Ok(Some(VecMmap {
        _mmap: mmap,
        model,
        content_hash,
        dims,
        vector_offset,
    }))
}

/// Check if an existing vec record is fresh (matches current content hash and model).
pub fn is_vec_fresh(existing: Option<&VecData>, content_hash: &str, model: &str) -> bool {
    match existing {
        Some(data) => data.content_hash == content_hash && data.model == model,
        None => false,
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn parse_vec_bytes(data: &[u8]) -> Result<VecData> {
    if data.len() < HEADER_SIZE {
        bail!("vec data too small for header ({} bytes)", data.len());
    }

    let mut cursor = Cursor::new(&data[..HEADER_SIZE]);
    let mut magic = [0u8; 4];
    cursor.read_exact(&mut magic)?;
    if &magic != MAGIC {
        bail!("invalid magic bytes in vec data");
    }

    let mut ver_buf = [0u8; 2];
    cursor.read_exact(&mut ver_buf)?;
    let version = u16::from_le_bytes(ver_buf);
    if version != VERSION {
        bail!(
            "unsupported vec version {}, expected {}",
            version,
            VERSION
        );
    }

    let mut dims_buf = [0u8; 2];
    cursor.read_exact(&mut dims_buf)?;
    let dims = u16::from_le_bytes(dims_buf) as usize;

    let mut hash_len_buf = [0u8; 4];
    cursor.read_exact(&mut hash_len_buf)?;
    let hash_len = u32::from_le_bytes(hash_len_buf) as usize;

    // Skip reserved
    let mut _reserved = [0u8; 4];
    cursor.read_exact(&mut _reserved)?;

    // Body
    let body_start = HEADER_SIZE;
    if data.len() < body_start + hash_len {
        bail!("vec data truncated at content_hash");
    }
    let content_hash = std::str::from_utf8(&data[body_start..body_start + hash_len])?.to_string();

    let model_start = body_start + hash_len;
    let null_pos = data[model_start..]
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| anyhow::anyhow!("no null terminator for model name"))?;
    let model = std::str::from_utf8(&data[model_start..model_start + null_pos])?.to_string();

    let vector_start = model_start + null_pos + 1;
    let expected_end = vector_start + dims * 4;
    if data.len() < expected_end {
        bail!(
            "vec data truncated at vector: expected {} bytes, got {}",
            expected_end,
            data.len()
        );
    }

    let mut vector = Vec::with_capacity(dims);
    for i in 0..dims {
        let offset = vector_start + i * 4;
        let bytes: [u8; 4] = data[offset..offset + 4].try_into()?;
        vector.push(f32::from_le_bytes(bytes));
    }

    Ok(VecData {
        model,
        content_hash,
        vector,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_vector(dims: usize) -> Vec<f32> {
        (0..dims).map(|i| i as f32 * 0.01).collect()
    }

    #[test]
    fn test_write_read_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.vec");
        let model = "BAAI/bge-small-en-v1.5";
        let hash = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
        let vector = sample_vector(384);

        write_vec(&path, model, hash, &vector).unwrap();
        let data = read_vec(&path).unwrap().expect("should read back");

        assert_eq!(data.model, model);
        assert_eq!(data.content_hash, hash);
        assert_eq!(data.vector.len(), 384);
        for (a, b) in data.vector.iter().zip(vector.iter()) {
            assert!((a - b).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn test_mmap_read_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test_mmap.vec");
        let model = "BAAI/bge-small-en-v1.5";
        let hash = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
        let vector = sample_vector(384);

        write_vec(&path, model, hash, &vector).unwrap();
        let mmap_data = read_vec_mmap(&path).unwrap().expect("should read back via mmap");

        assert_eq!(mmap_data.model, model);
        assert_eq!(mmap_data.content_hash, hash);
        assert_eq!(mmap_data.dims, 384);

        let mmap_vec = mmap_data.vector();
        assert_eq!(mmap_vec.len(), 384);
        for (a, b) in mmap_vec.iter().zip(vector.iter()) {
            assert!((a - b).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn test_missing_file_returns_none() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("nonexistent.vec");

        assert!(read_vec(&path).unwrap().is_none());
        assert!(read_vec_mmap(&path).unwrap().is_none());
    }

    #[test]
    fn test_is_vec_fresh() {
        let model = "BAAI/bge-small-en-v1.5";
        let hash = "abc123";

        let data = VecData {
            model: model.to_string(),
            content_hash: hash.to_string(),
            vector: vec![0.0; 384],
        };

        // Matching hash and model -> fresh
        assert!(is_vec_fresh(Some(&data), hash, model));

        // Different hash -> stale
        assert!(!is_vec_fresh(Some(&data), "different_hash", model));

        // Different model -> stale
        assert!(!is_vec_fresh(Some(&data), hash, "other-model"));

        // None -> stale
        assert!(!is_vec_fresh(None, hash, model));
    }

    #[test]
    fn test_small_vector() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("small.vec");
        let model = "test-model";
        let hash = "0000000000000000000000000000000000000000000000000000000000000000";
        let vector = vec![1.0, 2.0, 3.0];

        write_vec(&path, model, hash, &vector).unwrap();
        let data = read_vec(&path).unwrap().expect("should read back");

        assert_eq!(data.vector, vec![1.0, 2.0, 3.0]);
        assert_eq!(data.model, "test-model");
    }

    #[test]
    fn test_invalid_magic_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("bad.vec");
        let mut bad_data = vec![0u8; 64];
        bad_data[..4].copy_from_slice(b"NOPE");
        fs::write(&path, &bad_data).unwrap();

        assert!(read_vec(&path).is_err());
    }

    #[test]
    fn test_header_format() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("header.vec");
        let model = "m";
        let hash = "h";
        let vector = vec![0.5f32];

        write_vec(&path, model, hash, &vector).unwrap();
        let raw = fs::read(&path).unwrap();

        // Magic
        assert_eq!(&raw[0..4], b"SVEC");
        // Version = 1
        assert_eq!(u16::from_le_bytes([raw[4], raw[5]]), 1);
        // Dims = 1
        assert_eq!(u16::from_le_bytes([raw[6], raw[7]]), 1);
        // Hash len = 1
        assert_eq!(u32::from_le_bytes([raw[8], raw[9], raw[10], raw[11]]), 1);
        // Reserved = 0
        assert_eq!(
            u32::from_le_bytes([raw[12], raw[13], raw[14], raw[15]]),
            0
        );
        // Content hash = "h"
        assert_eq!(raw[16], b'h');
        // Model = "m\0"
        assert_eq!(raw[17], b'm');
        assert_eq!(raw[18], 0);
        // Padding: offset 19 is not 4-byte aligned, so 1 byte of padding at [19]
        // Vector starts at offset 20 (next 4-byte aligned)
        assert_eq!(
            f32::from_le_bytes([raw[20], raw[21], raw[22], raw[23]]),
            0.5
        );
        // Total size: 16 header + 1 hash + 2 model+null + 1 padding + 4 vector = 24
        assert_eq!(raw.len(), 24);
    }
}
