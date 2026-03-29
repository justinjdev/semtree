## MODIFIED Requirements

### Requirement: Bench command
The CLI SHALL expose a `bench` command that accepts a phase argument. The phase `"depth-profile"` SHALL run depth distribution analysis in addition to the existing `"quality"` and `"all"` phases.

#### Scenario: Depth-profile phase
- **WHEN** the user runs `semtree bench depth-profile --repo-path /path/to/repo --queries queries.yaml`
- **THEN** the CLI computes the f_k depth distribution from the query file and writes results to the TSV file

#### Scenario: All phase includes depth-profile
- **WHEN** the user runs `semtree bench all --repo-path /path/to/repo --queries queries.yaml`
- **THEN** both the quality phase and the depth-profile phase are executed

#### Scenario: Queries flag required for depth-profile
- **WHEN** the user runs `semtree bench depth-profile` without `--queries`
- **THEN** the CLI exits with an error message indicating the queries flag is required

## ADDED Requirements

### Requirement: Queries flag for bench command
The `bench` command SHALL accept a `--queries` flag specifying the path to a benchmark query YAML file. This flag is required for the `depth-profile` phase.

#### Scenario: Queries flag provided
- **WHEN** the user runs `semtree bench depth-profile --queries bench/queries/glamdring.yaml`
- **THEN** the specified query file is used for depth profiling

#### Scenario: Queries flag with non-existent file
- **WHEN** the user runs `semtree bench depth-profile --queries nonexistent.yaml`
- **THEN** the CLI exits with a clear error message that the file was not found
