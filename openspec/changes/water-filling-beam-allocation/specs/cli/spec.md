## ADDED Requirements

### Requirement: Beam policy flag
The `route` command SHALL accept a `--beam-policy` flag with values `uniform` and `waterfill`. The default SHALL be `uniform`.

#### Scenario: Default beam policy
- **WHEN** the user runs `semtree route "query"` without `--beam-policy`
- **THEN** the router uses uniform beam allocation (current behavior)

#### Scenario: Water-fill beam policy
- **WHEN** the user runs `semtree route "query" --beam-policy waterfill`
- **THEN** the router uses water-filling beam allocation with per-level difficulty-proportional widths

#### Scenario: Invalid beam policy value
- **WHEN** the user runs `semtree route "query" --beam-policy invalid`
- **THEN** the CLI exits with an error listing valid options

### Requirement: Beam policy propagation through daemon
The `--beam-policy` flag SHALL be included in the daemon/server JSON protocol when routing via the daemon. The daemon SHALL default to `uniform` if the `beam_policy` field is absent from the request.

#### Scenario: Daemon receives beam policy
- **WHEN** the CLI sends a route request to the daemon with `"beam_policy": "waterfill"`
- **THEN** the daemon uses water-filling beam allocation for that request

#### Scenario: Daemon backward compatibility
- **WHEN** an older client sends a route request without a `beam_policy` field
- **THEN** the daemon uses uniform beam allocation
