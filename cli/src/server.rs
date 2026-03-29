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

async fn handle_connection(stream: tokio::net::UnixStream) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        let resp = match serde_json::from_str::<Request>(&line) {
            Ok(req) => dispatch(&req).await,
            Err(e) => Response {
                result: None,
                error: Some(format!("invalid request: {e}")),
            },
        };
        let json = serde_json::to_string(&resp)?;
        writer.write_all(json.as_bytes()).await?;
        writer.write_all(b"\n").await?;
    }

    Ok(())
}

async fn dispatch(req: &Request) -> Response {
    match req.method.as_str() {
        "route" => {
            // Parse params: query, path, beam_width, max_depth
            // Will delegate to embedder::route_directory once implemented
            Response {
                result: Some(serde_json::json!({"status": "not_implemented"})),
                error: None,
            }
        }
        "query" => {
            // Parse params: query, path, top_k, threshold
            // Will delegate to embedder::query_directory once implemented
            Response {
                result: Some(serde_json::json!({"status": "not_implemented"})),
                error: None,
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
