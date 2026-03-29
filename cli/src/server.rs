use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;

const PID_FILE_NAME: &str = "semtree.pid";

#[derive(Deserialize)]
struct Request {
    method: String,
    params: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
struct Response {
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Start the daemon, listening on the given Unix socket path.
pub async fn serve(socket_path: &Path) -> Result<()> {
    let socket_path = expand_tilde(socket_path);

    // Check if daemon is already running
    if daemon_available(&socket_path) {
        bail!("daemon already running on {}", socket_path.display());
    }

    // Remove stale socket file if it exists
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    // Create parent directories
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Write PID file
    let pid_path = pid_path_for(&socket_path);
    std::fs::write(&pid_path, std::process::id().to_string())?;

    // Bind Unix socket
    let listener = UnixListener::bind(&socket_path)?;
    eprintln!("daemon listening on {}", socket_path.display());

    // Accept loop with graceful shutdown
    let accept_loop = async {
        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream).await {
                            eprintln!("connection error: {e}");
                        }
                    });
                }
                Err(e) => {
                    eprintln!("accept error: {e}");
                }
            }
        }
    };

    tokio::select! {
        _ = accept_loop => {}
        _ = tokio::signal::ctrl_c() => {
            eprintln!("shutting down...");
        }
    }

    // Cleanup
    let _ = std::fs::remove_file(&socket_path);
    let _ = std::fs::remove_file(&pid_path);

    Ok(())
}

/// Per-request timeout (seconds).
const REQUEST_TIMEOUT_SECS: u64 = 30;

async fn handle_connection(stream: tokio::net::UnixStream) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        let resp = match serde_json::from_str::<Request>(&line) {
            Ok(req) => {
                let method = req.method.clone();
                eprintln!("[daemon] request: method={} params={}", method, req.params);
                let start = std::time::Instant::now();

                // Clone data for the blocking task (avoid borrowing across await)
                let req_owned = req;

                let timeout = tokio::time::Duration::from_secs(REQUEST_TIMEOUT_SECS);
                let result = tokio::time::timeout(
                    timeout,
                    tokio::task::spawn_blocking(move || dispatch(&req_owned)),
                )
                .await;

                let elapsed_ms = start.elapsed().as_millis();

                match result {
                    Ok(Ok(resp)) => {
                        let status = if resp.error.is_some() { "error" } else { "ok" };
                        eprintln!("[daemon] response: method={} status={} elapsed={}ms", method, status, elapsed_ms);
                        resp
                    }
                    Ok(Err(e)) => {
                        eprintln!("[daemon] panic: method={} error={} elapsed={}ms", method, e, elapsed_ms);
                        Response {
                            result: None,
                            error: Some(format!("dispatch panicked: {e}")),
                        }
                    }
                    Err(_) => {
                        eprintln!("[daemon] timeout: method={} exceeded {}s", method, REQUEST_TIMEOUT_SECS);
                        Response {
                            result: None,
                            error: Some(format!("request timed out after {}s", REQUEST_TIMEOUT_SECS)),
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("[daemon] parse error: {}", e);
                Response {
                    result: None,
                    error: Some(format!("invalid request: {e}")),
                }
            }
        };
        let json = serde_json::to_string(&resp)?;
        writer.write_all(json.as_bytes()).await?;
        writer.write_all(b"\n").await?;
    }

    Ok(())
}

fn dispatch(req: &Request) -> Response {
    match req.method.as_str() {
        "route" => {
            let query = req.params["query"].as_str().unwrap_or("");
            let path_str = req.params["path"].as_str().unwrap_or(".");
            let beam_width = req.params["beam_width"].as_u64().unwrap_or(3) as usize;
            let max_depth = req.params["max_depth"].as_u64().unwrap_or(10) as usize;
            let model = req.params["model"].as_str().unwrap_or("BAAI/bge-small-en-v1.5");
            let policy = match req.params.get("beam_policy").and_then(|v| v.as_str()) {
                Some("waterfill") => crate::embedder::BeamPolicy::Waterfill,
                _ => crate::embedder::BeamPolicy::Uniform,
            };

            let path = std::path::PathBuf::from(path_str);
            match crate::embedder::route_directory_with_policy(&path, query, model, beam_width, max_depth, policy) {
                Ok(levels) => {
                    let json_levels: Vec<serde_json::Value> = levels.iter().map(|l| {
                        let mut obj = serde_json::json!({
                            "dir": l.dir,
                            "all_children": l.all_children,
                            "elapsed_ms": l.elapsed_ms,
                            "selected": l.selected.iter().map(|(p, s, fl)| {
                                serde_json::json!({"path": p, "score": s, "summary": fl})
                            }).collect::<Vec<_>>(),
                        });
                        if let Some(bf) = l.branching_factor {
                            obj["branching_factor"] = serde_json::json!(bf);
                        }
                        if let Some(amb) = l.ambiguity {
                            obj["ambiguity"] = serde_json::json!(amb);
                        }
                        if let Some(ab) = l.allocated_beam {
                            obj["allocated_beam"] = serde_json::json!(ab);
                        }
                        obj
                    }).collect();
                    Response { result: Some(serde_json::json!({"levels": json_levels})), error: None }
                }
                Err(e) => Response { result: None, error: Some(format!("{e}")) },
            }
        }
        "query" => {
            let query = req.params["query"].as_str().unwrap_or("");
            let path_str = req.params["path"].as_str().unwrap_or(".");
            let top_k = req.params["top_k"].as_u64().map(|v| v as usize);
            let threshold = req.params["threshold"].as_f64().map(|v| v as f32);
            let model = req.params["model"].as_str().unwrap_or("BAAI/bge-small-en-v1.5");

            let path = std::path::PathBuf::from(path_str);
            match crate::embedder::query_directory(&path, query, model, top_k, threshold) {
                Ok(results) => {
                    let json_results: Vec<serde_json::Value> = results.iter().map(|(score, p, fl)| {
                        serde_json::json!({"score": score, "path": p, "summary": fl})
                    }).collect();
                    Response { result: Some(serde_json::json!({"children": json_results})), error: None }
                }
                Err(e) => Response { result: None, error: Some(format!("{e}")) },
            }
        }
        _ => Response {
            result: None,
            error: Some(format!("unknown method: {}", req.method)),
        },
    }
}

// ---------------------------------------------------------------------------
// Client helpers
// ---------------------------------------------------------------------------

/// Check if a daemon is running by trying to connect to the socket.
pub fn daemon_available(socket_path: &Path) -> bool {
    std::os::unix::net::UnixStream::connect(socket_path).is_ok()
}

/// Send a request to the daemon and return the response.
pub fn daemon_request(
    socket_path: &Path,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    use std::io::{BufRead, Write};

    let mut stream = std::os::unix::net::UnixStream::connect(socket_path)?;
    let req = serde_json::json!({"method": method, "params": params});
    writeln!(stream, "{}", serde_json::to_string(&req)?)?;

    let mut reader = std::io::BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;

    let resp: Response = serde_json::from_str(&line)?;
    if let Some(err) = resp.error {
        bail!("daemon error: {err}");
    }
    resp.result.ok_or_else(|| anyhow::anyhow!("empty response"))
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

/// Default socket path for the daemon.
pub fn default_socket_path() -> PathBuf {
    expand_tilde(Path::new("~/.cache/semtree/semtree.sock"))
}

fn expand_tilde(path: &Path) -> PathBuf {
    if let Ok(stripped) = path.strip_prefix("~") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    }
    path.to_path_buf()
}

fn pid_path_for(socket_path: &Path) -> PathBuf {
    socket_path
        .parent()
        .unwrap_or(Path::new("."))
        .join(PID_FILE_NAME)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde_with_home() {
        let home = std::env::var("HOME").unwrap();
        let path = Path::new("~/.cache/semtree/semtree.sock");
        let expanded = expand_tilde(path);
        assert_eq!(
            expanded,
            PathBuf::from(&home).join(".cache/semtree/semtree.sock")
        );
    }

    #[test]
    fn test_expand_tilde_absolute_path() {
        let path = Path::new("/tmp/semtree.sock");
        let expanded = expand_tilde(path);
        assert_eq!(expanded, PathBuf::from("/tmp/semtree.sock"));
    }

    #[test]
    fn test_expand_tilde_relative_path() {
        let path = Path::new("foo/bar");
        let expanded = expand_tilde(path);
        assert_eq!(expanded, PathBuf::from("foo/bar"));
    }

    #[test]
    fn test_request_deserialization() {
        let json = r#"{"method":"route","params":{"query":"test","path":"/repo"}}"#;
        let req: Request = serde_json::from_str(json).unwrap();
        assert_eq!(req.method, "route");
        assert_eq!(req.params["query"], "test");
        assert_eq!(req.params["path"], "/repo");
    }

    #[test]
    fn test_response_serialization_result() {
        let resp = Response {
            result: Some(serde_json::json!({"score": 0.95})),
            error: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"score\""));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_response_serialization_error() {
        let resp = Response {
            result: None,
            error: Some("unknown method: foo".to_string()),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"error\""));
        assert!(!json.contains("\"result\""));
    }

    #[test]
    fn test_daemon_available_no_socket() {
        let path = Path::new("/tmp/semtree_test_nonexistent.sock");
        assert!(!daemon_available(path));
    }

    #[test]
    fn test_pid_path_for() {
        let socket = Path::new("/tmp/cache/semtree.sock");
        let pid = pid_path_for(&socket);
        assert_eq!(pid, PathBuf::from("/tmp/cache/semtree.pid"));
    }
}
