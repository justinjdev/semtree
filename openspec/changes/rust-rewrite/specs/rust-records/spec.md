## ADDED Requirements

### Requirement: File record path convention
The records module SHALL compute the record path for a file as `<parent_dir>/.sem/<filename>.md`. For example, the record for `src/main.rs` is stored at `src/.sem/main.rs.md`.

#### Scenario: File record path computed
- **WHEN** the records module computes the record path for `src/auth/login.rs`
- **THEN** the result is `src/auth/.sem/login.rs.md`

### Requirement: Directory record path convention
The records module SHALL compute the record path for a directory as `<dir>/.sem/__dir__.md`. This is the primary directory record containing the routing table.

#### Scenario: Directory record path computed
- **WHEN** the records module computes the directory record path for `src/auth/`
- **THEN** the result is `src/auth/.sem/__dir__.md`

### Requirement: Directory sibling record path convention
The records module SHALL compute a sibling record path for a directory as `<parent_dir>/.sem/<dirname>.md`. This allows the parent directory's routing table to reference a child directory's summary without descending into it.

#### Scenario: Directory sibling record path computed
- **WHEN** the records module computes the sibling record path for `src/auth/`
- **THEN** the result is `src/.sem/auth.md`

### Requirement: YAML frontmatter format
Records SHALL use `---` as the YAML frontmatter delimiter on its own line, both opening and closing. The frontmatter MUST contain three fields: `path` (repo-relative path), `type` (`file` or `directory`), and `content_hash` (SHA-256 hex string). The Markdown summary body SHALL follow after a blank line.

#### Scenario: Record format structure
- **WHEN** a record is written for `src/main.rs` with hash `abc123`
- **THEN** the file content is: `---\npath: src/main.rs\ntype: file\ncontent_hash: abc123\n---\n\n<summary body>`

#### Scenario: Directory record frontmatter
- **WHEN** a directory record is written for `src/auth/` with hash `def456`
- **THEN** the frontmatter contains `path: src/auth`, `type: directory`, and `content_hash: def456`

### Requirement: Record reading and parsing
The records module SHALL read existing `.sem/` records and parse the YAML frontmatter to extract `path`, `type`, and `content_hash` fields. The Markdown body after the closing `---` delimiter SHALL be extracted as the summary text.

#### Scenario: Existing record parsed successfully
- **WHEN** the records module reads `.sem/login.rs.md` containing valid frontmatter with `content_hash: def456`
- **THEN** the parsed record contains `content_hash: "def456"` and the summary body text

#### Scenario: Missing record returns None
- **WHEN** the records module attempts to read a record that does not exist on disk
- **THEN** it returns `None` (or the Rust equivalent `Option::None`) rather than erroring

#### Scenario: Malformed record handled gracefully
- **WHEN** the records module reads a file with invalid or missing YAML frontmatter
- **THEN** it returns an error or `None` rather than panicking

### Requirement: Record writing
The records module SHALL write `.sem/` records to disk, creating the `.sem/` directory if it does not exist. Writing MUST be atomic or safe against partial writes (write to temp file then rename).

#### Scenario: .sem/ directory created on first write
- **WHEN** the records module writes a record for `src/auth/login.rs` and `src/auth/.sem/` does not exist
- **THEN** `src/auth/.sem/` is created and the record is written to `src/auth/.sem/login.rs.md`

#### Scenario: Existing record overwritten
- **WHEN** the records module writes a record for a file that already has an existing `.sem/` record
- **THEN** the existing record is replaced with the new content

#### Scenario: Write is atomic
- **WHEN** the records module writes a record
- **THEN** the write uses a temporary file and rename to prevent partial writes from corrupting the record
