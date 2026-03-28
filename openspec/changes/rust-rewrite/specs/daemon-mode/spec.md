## ADDED Requirements

### Requirement: Daemon lifecycle
The system SHALL provide a `semtree serve` command that starts a background daemon process. The daemon SHALL listen on a Unix socket at `~/.cache/semtree/semtree.sock` (configurable via `--socket`). The daemon SHALL load the embedding model once at startup and keep it resident in memory for the lifetime of the process.

#### Scenario: Start daemon
- **WHEN** the user runs `semtree serve`
- **THEN** the daemon starts, loads the embedding model, writes its PID to `~/.cache/semtree/semtree.pid`, and begins listening on the Unix socket

#### Scenario: Daemon already running
- **WHEN** the user runs `semtree serve` and a daemon is already listening on the socket
- **THEN** the command exits with an error message indicating the daemon is already running

#### Scenario: Stop daemon
- **WHEN** the user sends SIGTERM or SIGINT to the daemon process
- **THEN** the daemon shuts down gracefully, removes the socket file and PID file, and exits

### Requirement: Transparent daemon usage
CLI commands that require the embedding model (`query`, `route`) SHALL automatically detect whether a daemon is running by checking for the Unix socket. If present, the command SHALL delegate to the daemon. If absent, the command SHALL fall back to cold-start inline execution.

#### Scenario: Warm path via daemon
- **WHEN** the daemon is running and the user runs `semtree route "query" /path`
- **THEN** the CLI sends the request to the daemon over the Unix socket and returns the result. Total latency SHALL be under 10ms for a 300-node tree.

#### Scenario: Cold path without daemon
- **WHEN** no daemon is running and the user runs `semtree route "query" /path`
- **THEN** the CLI loads the model inline, performs the route, and returns the result. Total latency SHALL be under 500ms.

#### Scenario: Daemon unavailable mid-request
- **WHEN** the daemon socket exists but the daemon process has crashed
- **THEN** the CLI detects the stale socket (connection refused), removes it, and falls back to cold-start execution

### Requirement: Daemon protocol
The daemon SHALL communicate via newline-delimited JSON over the Unix socket. Each request is a JSON object with `method` and `params` fields. Each response is a JSON object with `result` or `error` fields.

#### Scenario: Route request
- **WHEN** the CLI sends `{"method":"route","params":{"query":"...","path":"/repo","beam_width":3,"max_depth":10}}`
- **THEN** the daemon returns `{"result":{"levels":[...],"elapsed_ms":3}}`

#### Scenario: Query request
- **WHEN** the CLI sends `{"method":"query","params":{"query":"...","path":"/repo","top_k":5}}`
- **THEN** the daemon returns `{"result":{"children":[{"path":"...","score":0.75,"summary":"..."}]}}`

#### Scenario: Invalid request
- **WHEN** the CLI sends malformed JSON
- **THEN** the daemon returns `{"error":"invalid request: ..."}`

### Requirement: Model preloading
The daemon SHALL preload the embedding model into memory at startup. Subsequent embed/query/route operations SHALL NOT incur model loading latency.

#### Scenario: Model loaded at startup
- **WHEN** the daemon starts
- **THEN** the embedding model is fully loaded before the daemon begins accepting connections. Startup time SHALL be under 500ms.

#### Scenario: Model not found
- **WHEN** the daemon starts and the ONNX model file is not cached locally
- **THEN** the daemon downloads the model, caches it at `~/.cache/semtree/models/`, then loads it
