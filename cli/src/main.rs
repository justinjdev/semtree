use anyhow::Context;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod hasher;
mod records;
mod walker;
mod vec_store;
mod embedder;
mod summarizer;
mod builder;
mod server;
mod bench;
mod depth_profile;
mod review;

#[derive(Parser)]
#[command(name = "semtree", about = "Semantic Resolution Tree indexer")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build or update the SRT for a repository
    Build {
        /// Repository root path
        #[arg(default_value = ".")]
        path: PathBuf,
        /// LLM model for summarization
        #[arg(long, default_value = "claude-sonnet-4-20250514")]
        model: String,
        /// Max estimated tokens before marking file as oversized
        #[arg(long, default_value_t = 100_000)]
        max_tokens: usize,
        /// Rebuild all records, ignoring hash freshness checks
        #[arg(long)]
        force: bool,
        /// Glob pattern to exclude (can be repeated)
        #[arg(long)]
        exclude: Vec<String>,
        /// Skip embedding computation after build
        #[arg(long)]
        no_embed: bool,
        /// Embedding model name
        #[arg(long, default_value = "BAAI/bge-small-en-v1.5")]
        embed_model: String,
        /// Use Anthropic Batch API for file summaries (50% cost savings, async)
        #[arg(long)]
        batch: bool,
        /// Verify summaries with BottleSum-style orphan detection (re-summarize if children are lost)
        #[arg(long)]
        verify: bool,
        /// Cosine similarity threshold below which a child is considered an orphan (default: 0.3)
        #[arg(long, default_value_t = 0.3)]
        fidelity_threshold: f32,
        /// Max fraction of orphaned children before triggering re-summarization (default: 0.2)
        #[arg(long, default_value_t = 0.2)]
        orphan_rate: f32,
        /// Max re-summarization attempts per directory node (default: 2)
        #[arg(long, default_value_t = 2)]
        max_repair_attempts: usize,
    },

    /// Compute embeddings for existing .sem/ records
    Embed {
        /// Repository root path
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Embedding model name
        #[arg(long, default_value = "BAAI/bge-small-en-v1.5")]
        model: String,
        /// Re-embed all records, ignoring freshness checks
        #[arg(long)]
        force: bool,
    },

    /// Rank directory children by similarity to a query
    Query {
        /// Natural language query
        query: String,
        /// Directory whose children to rank
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Embedding model name
        #[arg(long, default_value = "BAAI/bge-small-en-v1.5")]
        model: String,
        /// Return only top K results
        #[arg(long)]
        top_k: Option<usize>,
        /// Minimum cosine similarity score
        #[arg(long)]
        threshold: Option<f32>,
    },

    /// Full top-down descent ranking children at each level
    Route {
        /// Natural language query
        query: String,
        /// Root directory to start descent from
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Embedding model name
        #[arg(long, default_value = "BAAI/bge-small-en-v1.5")]
        model: String,
        /// Number of children to select at each level
        #[arg(long, default_value_t = 7)]
        beam_width: usize,
        /// Maximum descent depth
        #[arg(long, default_value_t = 5)]
        max_depth: usize,
        /// Beam allocation policy
        #[arg(long, value_enum, default_value_t = embedder::BeamPolicy::Uniform)]
        beam_policy: embedder::BeamPolicy,
    },

    /// Analyze impact of changed files — find related files that may be affected
    Impact {
        /// Repository root path
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Changed files (if omitted, reads from git diff)
        #[arg(long)]
        files: Vec<String>,
        /// Embedding model name
        #[arg(long, default_value = "BAAI/bge-small-en-v1.5")]
        model: String,
        /// Number of related files to show per changed file
        #[arg(long, default_value_t = 5)]
        top_k: usize,
    },

    /// Generate a review manifest for code changes
    Review {
        /// Commit range (e.g., main..HEAD). Defaults to uncommitted changes.
        #[arg(default_value = "")]
        range: String,
        /// Repository root path
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Embedding model name
        #[arg(long, default_value = "BAAI/bge-small-en-v1.5")]
        model: String,
        /// Related files per changed file
        #[arg(long, default_value_t = 5)]
        top_k: usize,
        /// Cosine similarity threshold for cross-cutting warnings
        #[arg(long, default_value_t = 0.7)]
        similarity_threshold: f32,
    },

    /// Start daemon keeping embedding model loaded
    Serve {
        /// Unix socket path
        #[arg(long, default_value = "~/.cache/semtree/semtree.sock")]
        socket: PathBuf,
    },

    /// Run benchmark evaluation phases
    Bench {
        /// Phase to run
        #[arg(default_value = "all")]
        phase: String,
        /// Direct path to repo
        #[arg(long)]
        repo_path: Option<PathBuf>,
        /// Path to results TSV file
        #[arg(long, default_value = "results.tsv")]
        results: PathBuf,
        /// Path to benchmark query YAML file (required for depth-profile phase)
        #[arg(long)]
        queries: Option<PathBuf>,
        /// Run dilution ablation (delegates to Python bench)
        #[arg(long)]
        dilution: bool,
    },

    /// Inspect binary .vec files
    Vec {
        #[command(subcommand)]
        cmd: VecCommands,
    },
}

#[derive(Subcommand)]
enum VecCommands {
    /// Print human-readable metadata from a .vec file
    Inspect {
        /// Path to .vec file
        path: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build { path, model, max_tokens, force, exclude, no_embed, embed_model, batch, verify, fidelity_threshold, orphan_rate, max_repair_attempts } => {
            let config = builder::BuildConfig {
                target_path: std::fs::canonicalize(&path)?,
                model,
                max_tokens,
                force,
                exclude,
                embed: !no_embed,
                embed_model,
                verify,
                fidelity_threshold,
                orphan_rate,
                max_repair_attempts,
            };
            let stats = if batch {
                builder::build_batch(&config)?
            } else {
                builder::build(&config)?
            };
            if verify {
                println!(
                    "Build complete: {} summarized, {} skipped, {} errored, {} orphans detected, {} re-summarized",
                    stats.summarized, stats.skipped, stats.errored, stats.orphans_detected, stats.resummarized
                );
            } else {
                println!(
                    "Build complete: {} summarized, {} skipped, {} errored",
                    stats.summarized, stats.skipped, stats.errored
                );
            }
        }
        Commands::Embed { path, model, force } => {
            let target = std::fs::canonicalize(&path)?;
            let stats = embedder::embed_directory(&target, &model, force)?;
            eprintln!(
                "Embed complete: {} embedded, {} skipped, {} errored",
                stats.embedded, stats.skipped, stats.errored
            );
        }
        Commands::Query { query, path, model, top_k, threshold } => {
            let target = std::fs::canonicalize(&path)?;

            let socket_path = server::default_socket_path();
            let results: Vec<(f32, String, String)> = if server::daemon_available(&socket_path) {
                let params = serde_json::json!({
                    "path": target.to_string_lossy(),
                    "query": query,
                    "model": model,
                    "top_k": top_k,
                    "threshold": threshold,
                });
                let result = server::daemon_request(&socket_path, "query", params)?;
                let children = result.get("children").unwrap_or(&result);
                children.as_array().unwrap_or(&vec![]).iter().map(|c| (
                    c["score"].as_f64().unwrap_or(0.0) as f32,
                    c["path"].as_str().unwrap_or("").to_string(),
                    c["summary"].as_str().unwrap_or("").to_string(),
                )).collect()
            } else {
                embedder::query_directory(&target, &query, &model, top_k, threshold)?
            };

            if results.is_empty() {
                eprintln!("No results found.");
            } else {
                for (score, rpath, first_line) in &results {
                    println!("{:.4}\t{}\t{}", score, rpath, first_line);
                }
            }
        }
        Commands::Route { query, path, model, beam_width, max_depth, beam_policy } => {
            let target = std::fs::canonicalize(&path)?;
            let start = std::time::Instant::now();

            // Try daemon first, fall back to local
            let socket_path = server::default_socket_path();
            let levels = if server::daemon_available(&socket_path) {
                let params = serde_json::json!({
                    "path": target.to_string_lossy(),
                    "query": query,
                    "model": model,
                    "beam_width": beam_width,
                    "max_depth": max_depth,
                });
                let result = server::daemon_request(&socket_path, "route", params)?;
                // Daemon wraps in {"levels": [...]} with {path,score,summary} objects
                let wrapper: serde_json::Value = result;
                let levels_val = wrapper.get("levels").unwrap_or(&wrapper);
                let daemon_levels: Vec<serde_json::Value> = serde_json::from_value(levels_val.clone())?;
                daemon_levels.iter().map(|l| {
                    let selected: Vec<(String, f32, String)> = l["selected"].as_array()
                        .unwrap_or(&vec![]).iter()
                        .map(|s| (
                            s["path"].as_str().unwrap_or("").to_string(),
                            s["score"].as_f64().unwrap_or(0.0) as f32,
                            s["summary"].as_str().unwrap_or("").to_string(),
                        )).collect();
                    embedder::RouteLevel {
                        dir: l["dir"].as_str().unwrap_or("").to_string(),
                        selected,
                        all_children: l["all_children"].as_u64().unwrap_or(0) as usize,
                        elapsed_ms: l["elapsed_ms"].as_u64().unwrap_or(0),
                        branching_factor: l["branching_factor"].as_u64().map(|v| v as usize),
                        ambiguity: l["ambiguity"].as_f64().map(|v| v as f32),
                        allocated_beam: l["allocated_beam"].as_u64().map(|v| v as usize),
                    }
                }).collect()
            } else {
                embedder::route_directory_with_policy(&target, &query, &model, beam_width, max_depth, beam_policy)?
            };

            if levels.is_empty() {
                eprintln!("No .sem/ records found for routing.");
            } else {
                for level in &levels {
                    let diag = match (level.branching_factor, level.ambiguity, level.allocated_beam) {
                        (Some(bf), Some(amb), Some(ab)) =>
                            format!(" B={bf} m={amb:.2} beam={ab}"),
                        _ => String::new(),
                    };
                    println!("--- {} ({} children) [{}ms]{} ---", level.dir, level.all_children, level.elapsed_ms, diag);
                    for (rpath, score, first_line) in &level.selected {
                        println!("  {:.4}  {}  {}", score, rpath, first_line);
                    }
                }
                eprintln!("\nRoute complete in {}ms", start.elapsed().as_millis());
            }
        }
        Commands::Impact { path, files, model, top_k } => {
            let target = std::fs::canonicalize(&path)?;

            // If no files specified, get from git diff
            let changed = if files.is_empty() {
                let output = std::process::Command::new("git")
                    .args(["diff", "--name-only", "HEAD"])
                    .current_dir(&target)
                    .output()?;
                let stdout = String::from_utf8_lossy(&output.stdout);
                let mut files: Vec<String> = stdout.lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| l.trim().to_string())
                    .collect();
                if files.is_empty() {
                    // Try staged
                    let output = std::process::Command::new("git")
                        .args(["diff", "--name-only", "--cached"])
                        .current_dir(&target)
                        .output()?;
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    files = stdout.lines()
                        .filter(|l| !l.trim().is_empty())
                        .map(|l| l.trim().to_string())
                        .collect();
                }
                if files.is_empty() {
                    eprintln!("No changed files found. Specify with --files or have uncommitted changes.");
                    std::process::exit(1);
                }
                files
            } else {
                files
            };

            eprintln!("Analyzing impact of {} changed file(s)...", changed.len());
            let results = embedder::impact_analysis(&target, &changed, &model, top_k)?;

            for (changed_file, related) in &results {
                println!("\n{changed_file}:");
                if related.is_empty() {
                    println!("  (no related files found)");
                } else {
                    for (path, score, first_line) in related {
                        println!("  {score:.3}  {path}  {}", &first_line[..first_line.len().min(70)]);
                    }
                }
            }
        }
        Commands::Review { range, path, model, top_k, similarity_threshold } => {
            let target = std::fs::canonicalize(&path)?;
            review::run(&target, &range, &model, top_k, similarity_threshold)?;
        }
        Commands::Serve { socket } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(server::serve(&socket))?;
        }
        Commands::Bench { phase, repo_path, results, queries, dilution } => {
            let target = match repo_path {
                Some(p) => std::fs::canonicalize(&p)
                    .with_context(|| format!("invalid repo path: {}", p.display()))?,
                None => std::env::current_dir()?,
            };

            if phase == "quality" || phase == "all" {
                eprintln!("Running quality phase...");
                let metrics = bench::run_quality(&target, "local")?;
                let now = {
                    let d = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default();
                    format!("{}", d.as_secs())
                };
                let rows: Vec<_> = metrics.iter().map(|(metric, value)| {
                    (now.clone(), "quality".to_string(), "local".to_string(), "srt".to_string(),
                     String::new(), String::new(), metric.clone(), *value)
                }).collect();
                bench::append_tsv(&results, &rows)?;
                for (metric, value) in &metrics {
                    eprintln!("  {}: {}", metric, value);
                }
            }

            if phase == "depth-profile" || (phase == "all" && queries.is_some()) {
                let queries_path = queries.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("--queries is required for the depth-profile phase")
                })?;
                if !queries_path.exists() {
                    anyhow::bail!("query file not found: {}", queries_path.display());
                }
                eprintln!("Running depth-profile phase...");
                depth_profile::run_depth_profile(&target, queries_path, &results)?;
            }

            if dilution {
                eprintln!("Dilution ablation requested. Run via Python bench:");
                eprintln!("  python -m bench.routing --repo <path> --queries <file> --dilution --results {}", results.display());
            }

            eprintln!("Benchmark complete.");
        }
        Commands::Vec { cmd } => match cmd {
            VecCommands::Inspect { path } => {
                let data = vec_store::read_vec(&path)?
                    .ok_or_else(|| anyhow::anyhow!("file not found: {}", path.display()))?;
                println!("Model:        {}", data.model);
                println!("Content hash: {}", data.content_hash);
                println!("Dimensions:   {}", data.vector.len());
                println!("First 5:      {:?}", &data.vector[..data.vector.len().min(5)]);
            }
        },
    }

    Ok(())
}
