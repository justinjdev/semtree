## ADDED Requirements

### Requirement: Binary .vec file format
The system SHALL store embedding vectors in a binary format with the following layout:

```
Header (16 bytes):
  magic:    [u8; 4]   = b"SVEC"
  version:  u16       = 1
  dims:     u16       = embedding dimension count
  hash_len: u32       = length of content_hash string
  reserved: u32       = 0

Body:
  content_hash: [u8; hash_len]     = SHA-256 hex string (UTF-8)
  model_name:   null-terminated    = embedding model identifier
  vector:       [f32; dims]        = IEEE 754 little-endian floats
```

#### Scenario: Write and read round-trip
- **WHEN** the system writes a .vec file with content_hash "abc123", model "BAAI/bge-small-en-v1.5", and a 384-dim vector
- **THEN** reading the same file back returns identical content_hash, model name, and vector values

#### Scenario: File size
- **WHEN** the system writes a 384-dim .vec file
- **THEN** the file size is approximately 1641 bytes (16 header + 64 hash + 25 model + 1536 vector)

### Requirement: Memory-mapped reading
The system SHALL support reading .vec files via memory mapping (mmap). The f32 vector array SHALL be readable directly from the mapped memory without deserialization.

#### Scenario: mmap read performance
- **WHEN** the system reads 300 .vec files via mmap
- **THEN** total I/O time SHALL be under 10ms

#### Scenario: Concurrent reads
- **WHEN** multiple threads read the same .vec file via mmap simultaneously
- **THEN** all reads return correct data without corruption

### Requirement: Freshness check
The system SHALL determine if a .vec file is fresh by comparing its stored content_hash and model_name against the current values. A mismatch on either field means the embedding is stale.

#### Scenario: Fresh embedding
- **WHEN** a .vec file has content_hash "abc" and model "m", and the current record has hash "abc" and the configured model is "m"
- **THEN** the system reports the embedding as fresh and skips recomputation

#### Scenario: Stale hash
- **WHEN** a .vec file has content_hash "old" but the current record has hash "new"
- **THEN** the system reports the embedding as stale and recomputes it

#### Scenario: Model changed
- **WHEN** a .vec file has model "old-model" but the configured model is "new-model"
- **THEN** the system reports the embedding as stale and recomputes it

### Requirement: Colocated storage
Binary .vec files SHALL be stored alongside .sem/ Markdown records. For a record at `.sem/foo.py.md`, the corresponding embedding SHALL be at `.sem/foo.py.vec`.

#### Scenario: File naming
- **WHEN** the system computes an embedding for the record at `.sem/auth.py.md`
- **THEN** the embedding is written to `.sem/auth.py.vec`

#### Scenario: Directory embedding
- **WHEN** the system computes an embedding for `.sem/__dir__.md`
- **THEN** the embedding is written to `.sem/__dir__.vec`

### Requirement: Inspect command
The system SHALL provide a `semtree vec inspect <path>` subcommand that prints human-readable metadata from a binary .vec file.

#### Scenario: Inspect output
- **WHEN** the user runs `semtree vec inspect .sem/foo.py.vec`
- **THEN** the output shows: magic, version, dimensions, content_hash, model_name, and first 5 vector values
