use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod hasher;
mod records;
mod walker;
mod vec_store;
mod embedder;
mod summarizer;
mod builder;

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
        #[arg(long, default_value_t = 3)]
        beam_width: usize,
        /// Maximum descent depth
        #[arg(long, default_value_t = 10)]
        max_depth: usize,
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
        Commands::Build { path, model, max_tokens, force, exclude, no_embed, embed_model } => {
            todo!("build command")
        }
        Commands::Embed { path, model, force } => {
            todo!("embed command")
        }
        Commands::Query { query, path, model, top_k, threshold } => {
            todo!("query command")
        }
        Commands::Route { query, path, model, beam_width, max_depth } => {
            todo!("route command")
        }
        Commands::Serve { socket } => {
            todo!("serve command")
        }
        Commands::Bench { phase, repo_path, results } => {
            todo!("bench command")
        }
        Commands::Vec { cmd } => match cmd {
            VecCommands::Inspect { path } => {
                todo!("vec inspect command")
            }
        },
    }
}
