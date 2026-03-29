## MODIFIED Requirements

### Requirement: Build command
No change to the build command itself. (Placeholder -- this file modifies the bench command below.)

## ADDED Requirements

### Requirement: Bench dilution flag
The `semtree bench` command SHALL accept an optional `--dilution` flag. When provided, the bench command SHALL print a message indicating that the dilution ablation is available via the Python bench harness (`python -m bench.routing --dilution`), since the dilution experiment runs in the Python bench infrastructure.

#### Scenario: Dilution flag provided
- **WHEN** the user runs `semtree bench --dilution`
- **THEN** the CLI SHALL print instructions directing the user to run the Python bench: `python -m bench.routing --dilution --repo-path <path> --query-file <file>`

#### Scenario: No dilution flag
- **WHEN** the user runs `semtree bench` without `--dilution`
- **THEN** behavior SHALL be unchanged from the current quality-only bench phase
