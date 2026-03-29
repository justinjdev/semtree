//! Query-adaptive search: classifies intent and routes to the best backend.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use anyhow::Result;

use crate::embedder;
use crate::records;

// ---------------------------------------------------------------------------
// Intent classification
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QueryIntent {
    ExactLookup,
    Lexical,
    SemanticArchitectural,
    CrossCutting,
    Mixed,
}

impl QueryIntent {
    pub fn label(&self) -> &'static str {
        match self {
            QueryIntent::ExactLookup => "exact-lookup",
            QueryIntent::Lexical => "lexical",
            QueryIntent::SemanticArchitectural => "semantic-architectural",
            QueryIntent::CrossCutting => "cross-cutting",
            QueryIntent::Mixed => "mixed",
        }
    }
}

const KNOWN_EXTENSIONS: &[&str] = &[
    ".rs", ".py", ".ts", ".js", ".go", ".java", ".rb", ".c", ".cpp", ".h",
];

const STOP_WORDS: &[&str] = &[
    "how", "does", "the", "a", "an", "is", "what", "where", "which", "when",
    "do", "are", "in", "of", "to", "for", "and", "or", "with", "that",
];

/// Classify a natural language query into an intent class using heuristics.
pub fn classify_intent(query: &str) -> QueryIntent {
    let lower = query.to_lowercase();
    let tokens: Vec<&str> = query.split_whitespace().collect();

    // --- ExactLookup ---
    // Quoted string
    if query.contains('"') || query.contains('\'') {
        let has_quoted = query.contains('"')
            && query.match_indices('"').count() >= 2
            || query.contains('\'')
                && query.match_indices('\'').count() >= 2;
        if has_quoted {
            return QueryIntent::ExactLookup;
        }
    }
    // camelCase token
    if tokens.iter().any(|t| has_camel_case(t)) {
        return QueryIntent::ExactLookup;
    }
    // File path: contains `/` and ends in known extension
    if tokens.iter().any(|t| {
        t.contains('/') && KNOWN_EXTENSIONS.iter().any(|ext| t.ends_with(ext))
    }) {
        return QueryIntent::ExactLookup;
    }
    // Endpoint pattern
    if tokens.iter().any(|t| {
        t.starts_with("/api/") || t.starts_with("/v1/") || t.starts_with("/v2/")
    }) {
        return QueryIntent::ExactLookup;
    }

    // --- Lexical ---
    // ALL_CAPS token (3+ chars)
    if tokens.iter().any(|t| {
        t.len() >= 3 && t.chars().all(|c| c.is_ascii_uppercase() || c == '_')
            && t.chars().any(|c| c.is_ascii_uppercase())
    }) {
        return QueryIntent::Lexical;
    }
    // Regex syntax
    if query.contains(".*") || query.contains("\\d") || (query.contains('[') && query.contains(']')) {
        return QueryIntent::Lexical;
    }
    // Starts with "find all", "all references to", "where is...mentioned"
    if lower.starts_with("find all")
        || lower.starts_with("all references to")
        || (lower.starts_with("where is") && lower.contains("mentioned"))
    {
        return QueryIntent::Lexical;
    }

    // --- CrossCutting ---
    if lower.contains("across")
        || lower.contains("end-to-end")
        || lower.contains("throughout")
        || (lower.contains("between") && lower.contains("and"))
    {
        return QueryIntent::CrossCutting;
    }
    if lower.contains("what changes if")
        || lower.contains("what depends on")
        || lower.contains("affected by")
        || lower.contains("which parts")
    {
        return QueryIntent::CrossCutting;
    }
    // Multiple directory/service names — count path-like tokens with `/`
    let dir_tokens = tokens.iter().filter(|t| t.contains('/')).count();
    if dir_tokens >= 2 {
        return QueryIntent::CrossCutting;
    }

    // --- SemanticArchitectural ---
    if lower.contains("how does")
        || lower.contains("what does")
        || lower.contains("which subsystem")
        || lower.contains("which module")
        || lower.contains("what is the role")
    {
        return QueryIntent::SemanticArchitectural;
    }
    if lower.contains("architecture")
        || lower.contains("design")
        || lower.contains("responsible for")
        || lower.contains("purpose of")
    {
        return QueryIntent::SemanticArchitectural;
    }
    // No code-like tokens detected
    let has_code_tokens = tokens.iter().any(|t| {
        has_camel_case(t) || has_snake_case(t)
            || KNOWN_EXTENSIONS.iter().any(|ext| t.ends_with(ext))
    });
    if !has_code_tokens {
        return QueryIntent::SemanticArchitectural;
    }

    QueryIntent::Mixed
}

/// Check if a token contains camelCase (lowercase followed by uppercase).
fn has_camel_case(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    for i in 0..chars.len().saturating_sub(1) {
        if chars[i].is_ascii_lowercase() && chars[i + 1].is_ascii_uppercase() {
            return true;
        }
    }
    false
}

/// Check if a token looks like snake_case (contains underscore between alpha chars).
fn has_snake_case(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    for i in 1..chars.len().saturating_sub(1) {
        if chars[i] == '_' && chars[i - 1].is_ascii_alphanumeric() && chars[i + 1].is_ascii_alphanumeric() {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Search result
// ---------------------------------------------------------------------------

struct SearchResult {
    path: String,
    score: f32,
    backend: &'static str,
    context: String,
}

// ---------------------------------------------------------------------------
// Backend dispatch
// ---------------------------------------------------------------------------

fn dispatch(
    target: &Path,
    query: &str,
    model: &str,
    intent: QueryIntent,
    max_results: usize,
) -> Result<Vec<SearchResult>> {
    match intent {
        QueryIntent::ExactLookup => dispatch_exact(target, query, max_results),
        QueryIntent::Lexical => dispatch_lexical(target, query, max_results),
        QueryIntent::SemanticArchitectural => dispatch_semantic(target, query, model, max_results),
        QueryIntent::CrossCutting => dispatch_crosscutting(target, query, model, max_results),
        QueryIntent::Mixed => dispatch_mixed(target, query, model, max_results),
    }
}

/// Extract an identifier from the query: quoted strings, camelCase tokens, path-like tokens.
fn extract_identifier(query: &str) -> String {
    // Try quoted string first
    for quote in ['"', '\''] {
        let parts: Vec<&str> = query.split(quote).collect();
        if parts.len() >= 3 && !parts[1].is_empty() {
            return parts[1].to_string();
        }
    }
    // Try camelCase or path-like token
    for token in query.split_whitespace() {
        if has_camel_case(token) || token.contains('/') || token.contains('.') {
            // Strip punctuation from edges
            let cleaned = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '.' && c != '_' && c != '-');
            if !cleaned.is_empty() {
                return cleaned.to_string();
            }
        }
    }
    // Fallback: longest non-stop-word token
    query.split_whitespace()
        .filter(|t| !STOP_WORDS.contains(&t.to_lowercase().as_str()))
        .max_by_key(|t| t.len())
        .unwrap_or(query)
        .to_string()
}

/// Extract keywords from query, skipping stop words.
fn extract_keywords(query: &str) -> Vec<String> {
    query.split_whitespace()
        .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric() && c != '_'))
        .filter(|t| t.len() >= 2 && !STOP_WORDS.contains(&t.to_lowercase().as_str()))
        .map(|t| t.to_string())
        .collect()
}

fn dispatch_exact(target: &Path, query: &str, max_results: usize) -> Result<Vec<SearchResult>> {
    let identifier = extract_identifier(query);
    let output = Command::new("rg")
        .args(["-c", "--no-heading", &identifier])
        .current_dir(target)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut file_counts: Vec<(String, u32)> = stdout
        .lines()
        .filter_map(|line| {
            let (path, count) = line.rsplit_once(':')?;
            Some((path.to_string(), count.parse::<u32>().ok()?))
        })
        .collect();

    file_counts.sort_by(|a, b| b.1.cmp(&a.1));
    file_counts.truncate(max_results);

    let max_count = file_counts.first().map(|f| f.1).unwrap_or(1).max(1) as f32;

    let results = file_counts
        .into_iter()
        .map(|(path, count)| {
            let context = load_summary_context(target, &path)
                .unwrap_or_else(|| format!("{count} matches"));
            SearchResult {
                path,
                score: count as f32 / max_count,
                backend: "rg-exact",
                context,
            }
        })
        .collect();

    Ok(results)
}

fn dispatch_lexical(target: &Path, query: &str, max_results: usize) -> Result<Vec<SearchResult>> {
    let keywords = extract_keywords(query);
    if keywords.is_empty() {
        return Ok(Vec::new());
    }

    let mut file_scores: HashMap<String, u32> = HashMap::new();

    for kw in &keywords {
        let output = Command::new("rg")
            .args(["-c", "--no-heading", kw])
            .current_dir(target)
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some((path, count_str)) = line.rsplit_once(':') {
                if let Ok(count) = count_str.parse::<u32>() {
                    *file_scores.entry(path.to_string()).or_default() += count;
                }
            }
        }
    }

    let mut ranked: Vec<(String, u32)> = file_scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1));
    ranked.truncate(max_results);

    let max_score = ranked.first().map(|f| f.1).unwrap_or(1).max(1) as f32;

    let results = ranked
        .into_iter()
        .map(|(path, count)| {
            let context = load_summary_context(target, &path)
                .unwrap_or_else(|| format!("{count} grep hits"));
            SearchResult {
                path,
                score: count as f32 / max_score,
                backend: "rg-lexical",
                context,
            }
        })
        .collect();

    Ok(results)
}

fn dispatch_semantic(
    target: &Path,
    query: &str,
    model: &str,
    max_results: usize,
) -> Result<Vec<SearchResult>> {
    route_to_results(target, query, model, 7, 5, max_results, "srt-route")
}

fn dispatch_crosscutting(
    target: &Path,
    query: &str,
    model: &str,
    max_results: usize,
) -> Result<Vec<SearchResult>> {
    route_to_results(target, query, model, 10, 5, max_results, "srt-route-wide")
}

fn route_to_results(
    target: &Path,
    query: &str,
    model: &str,
    beam_width: usize,
    max_depth: usize,
    max_results: usize,
    backend: &'static str,
) -> Result<Vec<SearchResult>> {
    let levels = embedder::route_directory(target, query, model, beam_width, max_depth)?;

    let mut results: Vec<SearchResult> = Vec::new();
    for level in &levels {
        for (rpath, score, first_line) in &level.selected {
            // Only include files (not directories — directories have children we descend into)
            let abs = target.join(rpath);
            if abs.is_file() {
                results.push(SearchResult {
                    path: rpath.clone(),
                    score: *score,
                    backend,
                    context: first_line.clone(),
                });
            }
        }
    }

    // Deduplicate by path (keep highest score)
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut deduped: Vec<SearchResult> = Vec::new();
    for r in results {
        if let Some(&idx) = seen.get(&r.path) {
            if r.score > deduped[idx].score {
                deduped[idx] = r;
            }
        } else {
            seen.insert(r.path.clone(), deduped.len());
            deduped.push(r);
        }
    }

    deduped.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    deduped.truncate(max_results);
    Ok(deduped)
}

fn dispatch_mixed(
    target: &Path,
    query: &str,
    model: &str,
    max_results: usize,
) -> Result<Vec<SearchResult>> {
    // Run lexical
    let lexical = dispatch_lexical(target, query, max_results)?;
    // Run SRT route
    let semantic = route_to_results(target, query, model, 7, 5, max_results, "srt-route")?;

    // Merge: union with boost for files appearing in both
    let mut score_map: HashMap<String, (f32, &'static str, String)> = HashMap::new();

    for r in &lexical {
        score_map.insert(r.path.clone(), (r.score, r.backend, r.context.clone()));
    }
    for r in &semantic {
        if let Some(entry) = score_map.get_mut(&r.path) {
            // Boost: average of both + 0.1 bonus
            entry.0 = ((entry.0 + r.score) / 2.0 + 0.1).min(1.0);
            entry.1 = "mixed-boosted";
            if entry.2.is_empty() || entry.2.ends_with("grep hits") {
                entry.2 = r.context.clone();
            }
        } else {
            score_map.insert(r.path.clone(), (r.score, r.backend, r.context.clone()));
        }
    }

    let mut results: Vec<SearchResult> = score_map
        .into_iter()
        .map(|(path, (score, backend, context))| SearchResult {
            path,
            score,
            backend,
            context,
        })
        .collect();

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(max_results);
    Ok(results)
}

// ---------------------------------------------------------------------------
// Summary context loader
// ---------------------------------------------------------------------------

/// Load the first line of a .sem/ summary for a file, if available.
fn load_summary_context(target: &Path, repo_relative: &str) -> Option<String> {
    let record_path = records::record_path_for_file(target, repo_relative);
    let record = records::read_record(&record_path).ok()??;
    let first_line = record.summary.lines().next()?.trim().to_string();
    if first_line.is_empty() { None } else { Some(first_line) }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn run(target: &Path, query: &str, model: &str, max_results: usize) -> Result<()> {
    let intent = classify_intent(query);
    let results = dispatch(target, query, model, intent, max_results)?;

    println!("[intent: {}]\n", intent.label());

    if results.is_empty() {
        println!("  (no results found)");
    } else {
        for r in &results {
            let ctx = if r.context.len() > 70 {
                format!("{}...", &r.context[..67])
            } else {
                r.context.clone()
            };
            println!("  {:.4}  {} — {}", r.score, r.path, ctx);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_lookup_quoted() {
        assert_eq!(classify_intent("find \"validateWebhook\""), QueryIntent::ExactLookup);
    }

    #[test]
    fn test_exact_lookup_camel_case() {
        assert_eq!(classify_intent("where is validateWebhook defined"), QueryIntent::ExactLookup);
    }

    #[test]
    fn test_exact_lookup_file_path() {
        assert_eq!(classify_intent("show me src/auth/handler.rs"), QueryIntent::ExactLookup);
    }

    #[test]
    fn test_exact_lookup_endpoint() {
        assert_eq!(classify_intent("what handles /api/v1/users"), QueryIntent::ExactLookup);
    }

    #[test]
    fn test_lexical_all_caps() {
        assert_eq!(classify_intent("find RETRY_MAX in config"), QueryIntent::Lexical);
    }

    #[test]
    fn test_lexical_regex() {
        assert_eq!(classify_intent("search for log.*Error"), QueryIntent::Lexical);
    }

    #[test]
    fn test_lexical_find_all() {
        assert_eq!(classify_intent("find all usages of this function"), QueryIntent::Lexical);
    }

    #[test]
    fn test_cross_cutting_across() {
        assert_eq!(classify_intent("how is auth used across services"), QueryIntent::CrossCutting);
    }

    #[test]
    fn test_cross_cutting_depends() {
        assert_eq!(classify_intent("what depends on the database module"), QueryIntent::CrossCutting);
    }

    #[test]
    fn test_semantic_how_does() {
        assert_eq!(classify_intent("how does the build system work"), QueryIntent::SemanticArchitectural);
    }

    #[test]
    fn test_semantic_architecture() {
        assert_eq!(classify_intent("explain the overall architecture"), QueryIntent::SemanticArchitectural);
    }

    #[test]
    fn test_semantic_no_code_tokens() {
        assert_eq!(classify_intent("logging strategy"), QueryIntent::SemanticArchitectural);
    }

    #[test]
    fn test_mixed_fallback() {
        // snake_case token but no other category matches
        assert_eq!(classify_intent("something about my_function here"), QueryIntent::Mixed);
    }

    #[test]
    fn test_extract_identifier_quoted() {
        assert_eq!(extract_identifier("find \"validateWebhook\" in code"), "validateWebhook");
    }

    #[test]
    fn test_extract_identifier_camel() {
        assert_eq!(extract_identifier("where is validateWebhook"), "validateWebhook");
    }

    #[test]
    fn test_extract_keywords_stops() {
        let kws = extract_keywords("how does the build system work");
        assert!(!kws.contains(&"how".to_string()));
        assert!(!kws.contains(&"the".to_string()));
        assert!(kws.contains(&"build".to_string()));
        assert!(kws.contains(&"system".to_string()));
        assert!(kws.contains(&"work".to_string()));
    }
}
