/// Identifies one chunk within a container, unique within that container.
///
/// Chunks are "opaque named or typed byte chunks" per the container design —
/// a `ChunkId` is the typed half of that; [`ChunkDescriptor::kind`] carries
/// an open-ended label (e.g. `"metadata"`, `"model"`, `"tar.gz"`) for
/// whatever readers want to key off of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkId(pub u32);

/// How a chunk's bytes are represented on disk.
///
/// Version-zero of the experimental container only supports
/// [`ChunkCompression::None`]. The type exists so a future format version
/// can add real compression schemes without changing the chunk model's
/// shape — compression is a property of a chunk, not an assumption about
/// the whole container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ChunkCompression {
    None,
}

/// Logical description of one chunk: identity, an open-ended kind label, and
/// its compression marker.
///
/// Physical placement (byte offset/size within an encoded container) is a
/// concern of [`crate::container`], not of this logical descriptor — a
/// `ChunkDescriptor` is what a writer is asked to store, not what gets
/// written to disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkDescriptor {
    pub id: ChunkId,
    pub kind: String,
    pub compression: ChunkCompression,
}

impl ChunkDescriptor {
    pub fn new(id: ChunkId, kind: impl Into<String>) -> Self {
        Self {
            id,
            kind: kind.into(),
            compression: ChunkCompression::None,
        }
    }
}
