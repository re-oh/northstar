//! The experimental `.nspkg` container codec.
//!
//! This is a small, versioned, chunked binary format: a header carrying the
//! container's self-reported identity and a metadata payload, followed by
//! an index of opaque chunks, followed by the chunk bytes themselves.
//!
//! The layout below is **version zero and explicitly experimental** — see
//! `docs/architecture.md`. It is deliberately not optimized, does not
//! support in-place editing, streaming, encryption, signatures, or
//! deduplication, and every chunk currently declares
//! [`ChunkCompression::None`] because that is the only representation
//! version zero implements. All of that is confined behind
//! [`ContainerWriter`] and [`ContainerReader`] so the layout can be replaced
//! without the rest of Northstar noticing.
//!
//! The reader validates bounds and rejects malformed or truncated input by
//! returning [`ContainerError`] — it never panics on untrusted bytes. This
//! is ordinary defensive parsing, not DRM.
//!
//! ## Wire layout (version 0)
//!
//! ```text
//! magic:              [u8; 8]           b"NSPKGCTR"
//! format_version:     u16 LE            0
//! self_identity:      tag(u8) + fields  see `encode_identity`
//! metadata:            u32 LE len + bytes
//! chunk_count:        u32 LE
//! chunk_count × {
//!     id:              u32 LE
//!     kind:            u32 LE len + bytes (UTF-8)
//!     compression:     u8               0 = None
//!     offset:          u64 LE           absolute byte offset in the file
//!     size:            u64 LE
//!     checksum:        u64 LE           FNV-1a of the chunk's bytes
//! }
//! <chunk bytes, back to back, in index order>
//! ```

use thiserror::Error;

use crate::category::AssetCategory;
use crate::chunk::{ChunkCompression, ChunkDescriptor, ChunkId};
use crate::filename::ClassifiedFilename;
use crate::package_id::PackageId;
use crate::puid::AssetPuid;
use crate::version::FormatVersion;

const MAGIC: &[u8; 8] = b"NSPKGCTR";

const IDENTITY_TAG_PACKAGE: u8 = 0;
const IDENTITY_TAG_ASSET: u8 = 1;

const COMPRESSION_TAG_NONE: u8 = 0;

fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET_BASIS;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// Everything that can go wrong building or parsing an experimental
/// container. Every variant is returned, never panicked, even for
/// adversarially truncated or corrupted input.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ContainerError {
    #[error("container data is truncated or an offset/size runs past the end of the buffer")]
    Truncated,
    #[error("bad magic bytes; this is not a Northstar container")]
    BadMagic,
    #[error("unsupported container format version {0}")]
    UnsupportedVersion(u16),
    #[error("container bytes are not valid UTF-8 where a string was expected")]
    InvalidUtf8,
    #[error("invalid self-identity in container header: {0}")]
    InvalidIdentity(String),
    #[error("duplicate chunk id {0:?}")]
    DuplicateChunkId(ChunkId),
    #[error("unknown chunk id {0:?}")]
    UnknownChunkId(ChunkId),
    #[error("chunk {0:?} failed its integrity check")]
    ChunkIntegrityMismatch(ChunkId),
    #[error("unsupported chunk compression tag {0}")]
    UnsupportedCompression(u8),
}

// ---------------------------------------------------------------------
// byte-level helpers (no panics on untrusted input)
// ---------------------------------------------------------------------

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], ContainerError> {
        let end = self.pos.checked_add(len).ok_or(ContainerError::Truncated)?;
        let slice = self
            .buf
            .get(self.pos..end)
            .ok_or(ContainerError::Truncated)?;
        self.pos = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, ContainerError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, ContainerError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Result<u32, ContainerError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn u64(&mut self) -> Result<u64, ContainerError> {
        let b = self.take(8)?;
        Ok(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    fn bytes(&mut self) -> Result<&'a [u8], ContainerError> {
        let len = self.u32()? as usize;
        self.take(len)
    }

    fn string(&mut self) -> Result<&'a str, ContainerError> {
        std::str::from_utf8(self.bytes()?).map_err(|_| ContainerError::InvalidUtf8)
    }
}

fn write_u8(out: &mut Vec<u8>, v: u8) {
    out.push(v);
}
fn write_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn write_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn write_u64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn write_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    write_u32(out, bytes.len() as u32);
    out.extend_from_slice(bytes);
}
fn write_str(out: &mut Vec<u8>, s: &str) {
    write_bytes(out, s.as_bytes());
}

fn encoded_str_len(s: &str) -> usize {
    4 + s.len()
}

// ---------------------------------------------------------------------
// self-identity encode/decode (reuses ClassifiedFilename)
// ---------------------------------------------------------------------

fn write_identity(out: &mut Vec<u8>, identity: &ClassifiedFilename) {
    match identity {
        ClassifiedFilename::Package { puid } => {
            write_u8(out, IDENTITY_TAG_PACKAGE);
            write_str(out, puid.as_str());
        }
        ClassifiedFilename::Asset { puid, category } => {
            write_u8(out, IDENTITY_TAG_ASSET);
            write_str(out, puid.as_str());
            write_str(out, category.as_str());
        }
    }
}

fn encoded_identity_len(identity: &ClassifiedFilename) -> usize {
    1 + match identity {
        ClassifiedFilename::Package { puid } => encoded_str_len(puid.as_str()),
        ClassifiedFilename::Asset { puid, category } => {
            encoded_str_len(puid.as_str()) + encoded_str_len(category.as_str())
        }
    }
}

fn read_identity(r: &mut Reader<'_>) -> Result<ClassifiedFilename, ContainerError> {
    match r.u8()? {
        IDENTITY_TAG_PACKAGE => {
            let puid = PackageId::new(r.string()?)
                .map_err(|e| ContainerError::InvalidIdentity(e.to_string()))?;
            Ok(ClassifiedFilename::Package { puid })
        }
        IDENTITY_TAG_ASSET => {
            let puid = AssetPuid::new(r.string()?)
                .map_err(|e| ContainerError::InvalidIdentity(e.to_string()))?;
            let category = AssetCategory::new(r.string()?)
                .map_err(|e| ContainerError::InvalidIdentity(e.to_string()))?;
            Ok(ClassifiedFilename::Asset { puid, category })
        }
        other => Err(ContainerError::InvalidIdentity(format!(
            "unknown identity tag {other}"
        ))),
    }
}

fn compression_tag(c: ChunkCompression) -> u8 {
    match c {
        ChunkCompression::None => COMPRESSION_TAG_NONE,
    }
}

fn compression_from_tag(tag: u8) -> Result<ChunkCompression, ContainerError> {
    match tag {
        COMPRESSION_TAG_NONE => Ok(ChunkCompression::None),
        other => Err(ContainerError::UnsupportedCompression(other)),
    }
}

// ---------------------------------------------------------------------
// writer
// ---------------------------------------------------------------------

/// Builds one experimental container in memory. File I/O (if any) is the
/// caller's responsibility — this type only ever produces a `Vec<u8>`.
pub struct ContainerWriter {
    self_identity: ClassifiedFilename,
    metadata: Vec<u8>,
    chunks: Vec<(ChunkDescriptor, Vec<u8>)>,
}

impl ContainerWriter {
    pub fn new(self_identity: ClassifiedFilename) -> Self {
        Self {
            self_identity,
            metadata: Vec::new(),
            chunks: Vec::new(),
        }
    }

    /// Attach a small metadata payload. Opaque bytes — this crate does not
    /// impose a schema on them.
    pub fn with_metadata(mut self, metadata: impl Into<Vec<u8>>) -> Self {
        self.metadata = metadata.into();
        self
    }

    /// Append one chunk. Rejects a `descriptor.id` already used by a
    /// previously pushed chunk in this writer.
    pub fn push_chunk(
        &mut self,
        descriptor: ChunkDescriptor,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<(), ContainerError> {
        if self.chunks.iter().any(|(d, _)| d.id == descriptor.id) {
            return Err(ContainerError::DuplicateChunkId(descriptor.id));
        }
        self.chunks.push((descriptor, bytes.into()));
        Ok(())
    }

    /// Encode the container to bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        write_u16(&mut out, FormatVersion::EXPERIMENTAL_V0.0);
        write_identity(&mut out, &self.self_identity);
        write_bytes(&mut out, &self.metadata);
        write_u32(&mut out, self.chunks.len() as u32);

        // The index's own encoded length doesn't depend on the offsets it
        // will contain (offset/size/checksum are fixed-width u64), so we
        // can compute where chunk data starts without a placeholder pass.
        let index_len: usize = self
            .chunks
            .iter()
            .map(|(d, _)| 4 + encoded_str_len(&d.kind) + 1 + 8 + 8 + 8)
            .sum();
        let header_len = 8 // magic
            + 2 // format_version
            + encoded_identity_len(&self.self_identity)
            + encoded_str_len_for_bytes(&self.metadata)
            + 4 // chunk_count
            + index_len;

        let mut offset = header_len;
        let mut offsets = Vec::with_capacity(self.chunks.len());
        for (_, bytes) in &self.chunks {
            offsets.push(offset);
            offset += bytes.len();
        }

        for ((descriptor, bytes), offset) in self.chunks.iter().zip(offsets) {
            write_u32(&mut out, descriptor.id.0);
            write_str(&mut out, &descriptor.kind);
            write_u8(&mut out, compression_tag(descriptor.compression));
            write_u64(&mut out, offset as u64);
            write_u64(&mut out, bytes.len() as u64);
            write_u64(&mut out, fnv1a64(bytes));
        }

        debug_assert_eq!(out.len(), header_len);

        for (_, bytes) in &self.chunks {
            out.extend_from_slice(bytes);
        }

        out
    }
}

fn encoded_str_len_for_bytes(b: &[u8]) -> usize {
    4 + b.len()
}

// ---------------------------------------------------------------------
// reader
// ---------------------------------------------------------------------

#[derive(Debug)]
struct ChunkEntry {
    descriptor: ChunkDescriptor,
    offset: usize,
    size: usize,
}

/// A parsed, validated view over an experimental container's bytes.
///
/// [`ContainerReader::parse`] performs full structural validation up front —
/// bounds, UTF-8, and per-chunk integrity checksums — so accessors after a
/// successful parse cannot panic or observe corrupted data.
#[derive(Debug)]
pub struct ContainerReader<'a> {
    buf: &'a [u8],
    self_identity: ClassifiedFilename,
    metadata: &'a [u8],
    entries: Vec<ChunkEntry>,
}

impl<'a> ContainerReader<'a> {
    pub fn parse(buf: &'a [u8]) -> Result<Self, ContainerError> {
        let mut r = Reader::new(buf);

        let magic = r.take(8)?;
        if magic != MAGIC {
            return Err(ContainerError::BadMagic);
        }

        let format_version = r.u16()?;
        if format_version != FormatVersion::EXPERIMENTAL_V0.0 {
            return Err(ContainerError::UnsupportedVersion(format_version));
        }

        let self_identity = read_identity(&mut r)?;
        let metadata = r.bytes()?;
        let chunk_count = r.u32()?;

        let mut entries = Vec::with_capacity(chunk_count as usize);
        for _ in 0..chunk_count {
            let id = ChunkId(r.u32()?);
            let kind = r.string()?.to_owned();
            let compression = compression_from_tag(r.u8()?)?;
            let offset = r.u64()?;
            let size = r.u64()?;
            let checksum = r.u64()?;

            if entries.iter().any(|e: &ChunkEntry| e.descriptor.id == id) {
                return Err(ContainerError::DuplicateChunkId(id));
            }

            let offset = usize::try_from(offset).map_err(|_| ContainerError::Truncated)?;
            let size = usize::try_from(size).map_err(|_| ContainerError::Truncated)?;
            let end = offset.checked_add(size).ok_or(ContainerError::Truncated)?;
            let chunk_bytes = buf.get(offset..end).ok_or(ContainerError::Truncated)?;

            if fnv1a64(chunk_bytes) != checksum {
                return Err(ContainerError::ChunkIntegrityMismatch(id));
            }

            entries.push(ChunkEntry {
                descriptor: ChunkDescriptor {
                    id,
                    kind,
                    compression,
                },
                offset,
                size,
            });
        }

        Ok(Self {
            buf,
            self_identity,
            metadata,
            entries,
        })
    }

    pub fn format_version(&self) -> FormatVersion {
        FormatVersion::EXPERIMENTAL_V0
    }

    pub fn self_identity(&self) -> &ClassifiedFilename {
        &self.self_identity
    }

    pub fn metadata(&self) -> &'a [u8] {
        self.metadata
    }

    pub fn chunk_descriptors(&self) -> impl Iterator<Item = &ChunkDescriptor> {
        self.entries.iter().map(|e| &e.descriptor)
    }

    pub fn chunk_bytes(&self, id: ChunkId) -> Result<&'a [u8], ContainerError> {
        let entry = self
            .entries
            .iter()
            .find(|e| e.descriptor.id == id)
            .ok_or(ContainerError::UnknownChunkId(id))?;
        Ok(&self.buf[entry.offset..entry.offset + entry.size])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_identity() -> ClassifiedFilename {
        ClassifiedFilename::Asset {
            puid: AssetPuid::new("pebble_sea_islands").unwrap(),
            category: AssetCategory::new("map").unwrap(),
        }
    }

    #[test]
    fn round_trips_multiple_arbitrary_chunks_exactly() {
        let mut w = ContainerWriter::new(map_identity()).with_metadata(*b"hello metadata");
        w.push_chunk(
            ChunkDescriptor::new(ChunkId(0), "header"),
            b"first chunk".to_vec(),
        )
        .unwrap();
        w.push_chunk(
            ChunkDescriptor::new(ChunkId(1), "geometry"),
            Vec::<u8>::new(),
        )
        .unwrap();
        w.push_chunk(
            ChunkDescriptor::new(ChunkId(7), "texture"),
            vec![0xffu8; 4096],
        )
        .unwrap();

        let bytes = w.encode();
        let r = ContainerReader::parse(&bytes).unwrap();

        assert_eq!(r.format_version(), FormatVersion::EXPERIMENTAL_V0);
        assert_eq!(r.self_identity(), &map_identity());
        assert_eq!(r.metadata(), b"hello metadata");
        assert_eq!(r.chunk_bytes(ChunkId(0)).unwrap(), b"first chunk");
        assert_eq!(r.chunk_bytes(ChunkId(1)).unwrap(), b"");
        assert_eq!(
            r.chunk_bytes(ChunkId(7)).unwrap(),
            vec![0xffu8; 4096].as_slice()
        );
        assert_eq!(r.chunk_descriptors().count(), 3);
    }

    #[test]
    fn writer_rejects_duplicate_chunk_ids() {
        let mut w = ContainerWriter::new(map_identity());
        w.push_chunk(ChunkDescriptor::new(ChunkId(0), "a"), b"x".to_vec())
            .unwrap();
        let err = w
            .push_chunk(ChunkDescriptor::new(ChunkId(0), "b"), b"y".to_vec())
            .unwrap_err();
        assert_eq!(err, ContainerError::DuplicateChunkId(ChunkId(0)));
    }

    #[test]
    fn reader_rejects_unknown_chunk_id() {
        let w = ContainerWriter::new(map_identity());
        let bytes = w.encode();
        let r = ContainerReader::parse(&bytes).unwrap();
        assert_eq!(
            r.chunk_bytes(ChunkId(99)),
            Err(ContainerError::UnknownChunkId(ChunkId(99)))
        );
    }

    #[test]
    fn truncated_header_does_not_panic() {
        for len in 0..8 {
            assert_eq!(
                ContainerReader::parse(&MAGIC[..len]).unwrap_err(),
                ContainerError::Truncated
            );
        }
    }

    #[test]
    fn bad_magic_is_rejected() {
        let bytes = b"NOTNSPKG_garbage_after".to_vec();
        assert_eq!(
            ContainerReader::parse(&bytes).unwrap_err(),
            ContainerError::BadMagic
        );
    }

    #[test]
    fn unsupported_future_version_is_rejected() {
        let mut w = ContainerWriter::new(map_identity()).encode();
        // format_version is the two bytes right after the 8-byte magic.
        w[8] = 7;
        w[9] = 0;
        assert_eq!(
            ContainerReader::parse(&w).unwrap_err(),
            ContainerError::UnsupportedVersion(7)
        );
    }

    #[test]
    fn impossible_chunk_offset_is_rejected_without_panicking() {
        let mut w = ContainerWriter::new(map_identity());
        w.push_chunk(ChunkDescriptor::new(ChunkId(0), "a"), b"data".to_vec())
            .unwrap();
        let mut bytes = w.encode();
        // Corrupt the recorded offset (first u64 in the one index entry) to
        // something wildly out of range. Index entry starts right after
        // chunk_count; find it via chunk_bytes-free arithmetic isn't worth
        // it here — instead corrupt the *last* 24 bytes before chunk data,
        // which is guaranteed to include offset/size/checksum for this
        // single-chunk container: overwrite the offset field specifically
        // by locating it structurally.
        let identity_len = encoded_identity_len(&map_identity());
        let metadata_len = encoded_str_len_for_bytes(&[]);
        let header_prefix = 8 + 2 + identity_len + metadata_len + 4;
        let kind_len = encoded_str_len("a");
        let offset_field_pos = header_prefix + 4 + kind_len + 1; // id + kind + compression
        bytes[offset_field_pos..offset_field_pos + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(
            ContainerReader::parse(&bytes).unwrap_err(),
            ContainerError::Truncated
        );
    }

    #[test]
    fn corrupted_chunk_bytes_fail_integrity_check() {
        let mut w = ContainerWriter::new(map_identity());
        w.push_chunk(ChunkDescriptor::new(ChunkId(0), "a"), b"data".to_vec())
            .unwrap();
        let mut bytes = w.encode();
        // Flip a byte inside the chunk payload (at the very end of the
        // buffer, since it's the last and only chunk).
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff;
        assert_eq!(
            ContainerReader::parse(&bytes).unwrap_err(),
            ContainerError::ChunkIntegrityMismatch(ChunkId(0))
        );
    }

    #[test]
    fn unsupported_compression_tag_is_rejected() {
        let mut w = ContainerWriter::new(map_identity());
        w.push_chunk(ChunkDescriptor::new(ChunkId(0), "a"), b"data".to_vec())
            .unwrap();
        let mut bytes = w.encode();
        let identity_len = encoded_identity_len(&map_identity());
        let metadata_len = encoded_str_len_for_bytes(&[]);
        let header_prefix = 8 + 2 + identity_len + metadata_len + 4;
        let kind_len = encoded_str_len("a");
        let compression_field_pos = header_prefix + 4 + kind_len;
        bytes[compression_field_pos] = 99;
        assert_eq!(
            ContainerReader::parse(&bytes).unwrap_err(),
            ContainerError::UnsupportedCompression(99)
        );
    }

    #[test]
    fn package_identity_round_trips() {
        let identity = ClassifiedFilename::Package {
            puid: PackageId::new("basegame").unwrap(),
        };
        let bytes = ContainerWriter::new(identity.clone()).encode();
        let r = ContainerReader::parse(&bytes).unwrap();
        assert_eq!(r.self_identity(), &identity);
    }
}
