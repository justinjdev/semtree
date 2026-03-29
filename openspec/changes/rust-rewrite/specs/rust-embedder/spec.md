## ADDED Requirements

### Requirement: ONNX Runtime inference
The system SHALL load a BAAI/bge-small-en-v1.5 ONNX model via the `ort` crate and produce 384-dimensional embedding vectors from text input.

#### Scenario: Document embedding
- **WHEN** the system embeds the text "Authentication module for user sessions"
- **THEN** it returns a 384-dimensional f32 vector with L2 norm approximately 1.0

#### Scenario: Query embedding
- **WHEN** the system embeds a query "how does authentication work?"
- **THEN** it returns a 384-dimensional f32 vector suitable for cosine similarity comparison with document embeddings

#### Scenario: Batch embedding
- **WHEN** the system embeds 100 texts in a single batch call
- **THEN** it returns 100 vectors, completing in under 500ms

### Requirement: Query vs document embedding
The system SHALL distinguish between query and document (passage) embedding. Query text SHALL be prefixed with "query: " and document text with "passage: " before tokenization, matching the bge-small-en-v1.5 model's expected input format.

#### Scenario: Prefix application
- **WHEN** the system embeds "auth module" as a document
- **THEN** the actual input to the model is "passage: auth module"

#### Scenario: Query prefix
- **WHEN** the system embeds "how does auth work?" as a query
- **THEN** the actual input to the model is "query: how does auth work?"

### Requirement: Cosine similarity ranking
The system SHALL compute cosine similarity between a query vector and a set of child vectors, returning results sorted by similarity descending.

#### Scenario: Ranking correctness
- **WHEN** query vector is [1,0,0] and children are a=[0,1,0], b=[0.9,0.1,0], c=[0.5,0.5,0]
- **THEN** the ranking is b (highest), c, a (lowest)

#### Scenario: Empty children
- **WHEN** the children set is empty
- **THEN** the system returns an empty ranking

### Requirement: Model caching
The ONNX model file SHALL be cached at `~/.cache/semtree/models/`. If the model is not present, the system SHALL download it on first use.

#### Scenario: First use download
- **WHEN** the user runs `semtree embed` for the first time and no model is cached
- **THEN** the system downloads the model, prints a progress indicator, caches it, and proceeds with embedding

#### Scenario: Cached model
- **WHEN** the model file exists at the cache path
- **THEN** the system loads it directly without network access

### Requirement: Embed directory
The system SHALL provide an `embed_directory` function that walks all .sem/*.md records under a path, checks freshness of each .vec sidecar, and recomputes stale embeddings in batch.

#### Scenario: Incremental embedding
- **WHEN** 300 records exist and 10 have changed content_hash since last embed
- **THEN** only 10 embeddings are recomputed and written

#### Scenario: Force re-embed
- **WHEN** the --force flag is set
- **THEN** all embeddings are recomputed regardless of freshness

#### Scenario: Stats reporting
- **WHEN** embedding completes
- **THEN** the system reports counts: embedded, skipped, errored

### Requirement: Route directory
The system SHALL provide a `route_directory` function that performs beam-search descent from a root directory, ranking children by cosine similarity at each level, descending into top-scoring directories, and collecting file candidates.

#### Scenario: Multi-level descent
- **WHEN** route is called with beam_width=3, max_depth=10 on a 5-level tree
- **THEN** the system descends through up to 10 levels, selecting top-3 children at each, and returns file candidates sorted by cosine score

#### Scenario: Depth limit
- **WHEN** max_depth=2
- **THEN** the system descends at most 2 levels from the root

#### Scenario: Performance
- **WHEN** routing over a 300-node tree with a warm model
- **THEN** total latency SHALL be under 5ms
