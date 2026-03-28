## ADDED Requirements

### Requirement: Colocated .sem/ directory structure
The indexer SHALL store summary records in hidden `.sem/` directories colocated with the code they describe. Each directory in the repository that contains indexed content SHALL have a `.sem/` subdirectory.

#### Scenario: .sem/ directory created alongside code
- **WHEN** the indexer processes files in `src/auth/`
- **THEN** a `src/auth/.sem/` directory is created containing the records

### Requirement: Directory record format
Each indexed directory SHALL have a record at `<dir>/.sem/__dir__.md`. The record SHALL contain YAML frontmatter with `path` (repo-relative), `type: directory`, and `content_hash`, followed by a Markdown body with a prose summary and a `## Children` routing table.

#### Scenario: Directory record written
- **WHEN** the indexer finishes processing directory `src/auth/`
- **THEN** `src/auth/.sem/__dir__.md` exists with valid YAML frontmatter and a `## Children` section

#### Scenario: Directory record frontmatter fields
- **WHEN** the indexer writes a directory record for `src/auth/` with hash `abc123`
- **THEN** the frontmatter contains `path: src/auth`, `type: directory`, and `content_hash: abc123`

### Requirement: File record format
Each indexed file SHALL have a record at `<dir>/.sem/<filename>.md`. The record SHALL contain YAML frontmatter with `path` (repo-relative), `type: file`, and `content_hash`, followed by a Markdown body with the file's summary.

#### Scenario: File record written
- **WHEN** the indexer processes `src/auth/login.py`
- **THEN** `src/auth/.sem/login.py.md` exists with valid YAML frontmatter and summary body

#### Scenario: File record frontmatter fields
- **WHEN** the indexer writes a file record for `src/auth/login.py` with hash `def456`
- **THEN** the frontmatter contains `path: src/auth/login.py`, `type: file`, and `content_hash: def456`

### Requirement: YAML frontmatter delimiters
Records SHALL use `---` as the YAML frontmatter delimiter on its own line, both opening and closing. The Markdown body follows after a blank line.

#### Scenario: Record format structure
- **WHEN** a record is written
- **THEN** it starts with `---`, followed by YAML key-value pairs, followed by `---`, a blank line, and the Markdown summary body

### Requirement: Record reading
The indexer SHALL be able to read existing `.sem/` records to extract the stored `content_hash` for freshness comparison. Reading SHALL parse the YAML frontmatter between `---` delimiters.

#### Scenario: Existing record parsed
- **WHEN** the indexer encounters an existing `.sem/login.py.md` with frontmatter containing `content_hash: def456`
- **THEN** the stored hash `def456` is extracted and available for comparison
