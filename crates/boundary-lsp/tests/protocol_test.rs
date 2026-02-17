use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{json, Value};

fn lsp_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_boundary-lsp"))
}

fn go_fixture_path() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{manifest_dir}/../boundary/tests/fixtures/sample-go-project")
}

/// Encode a JSON-RPC message with Content-Length header for LSP transport.
fn encode_message(msg: &Value) -> Vec<u8> {
    let body = serde_json::to_string(msg).unwrap();
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body).into_bytes()
}

/// A non-blocking LSP message reader that runs in a background thread.
struct LspReader {
    rx: mpsc::Receiver<Value>,
}

impl LspReader {
    fn new(stdout: std::process::ChildStdout) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                match Self::read_one(&mut reader) {
                    Some(msg) => {
                        if tx.send(msg).is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
        });
        Self { rx }
    }

    fn read_one(reader: &mut BufReader<impl Read>) -> Option<Value> {
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => return None,
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        break;
                    }
                    if let Some(len_str) = trimmed.strip_prefix("Content-Length: ") {
                        content_length = len_str.parse().ok();
                    }
                }
                Err(_) => return None,
            }
        }
        let length = content_length?;
        let mut body = vec![0u8; length];
        reader.read_exact(&mut body).ok()?;
        serde_json::from_slice(&body).ok()
    }

    /// Wait for a message matching the predicate, with timeout.
    fn wait_for(&self, predicate: impl Fn(&Value) -> bool, timeout: Duration) -> Option<Value> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return None;
            }
            match self.rx.recv_timeout(remaining) {
                Ok(msg) if predicate(&msg) => return Some(msg),
                Ok(_) => continue,
                Err(mpsc::RecvTimeoutError::Timeout) => return None,
                Err(mpsc::RecvTimeoutError::Disconnected) => return None,
            }
        }
    }
}

fn send_shutdown_and_exit(stdin: &mut impl Write) {
    let shutdown = json!({
        "jsonrpc": "2.0",
        "id": 999,
        "method": "shutdown"
    });
    let _ = stdin.write_all(&encode_message(&shutdown));
    let _ = stdin.flush();
    std::thread::sleep(Duration::from_millis(100));

    let exit = json!({
        "jsonrpc": "2.0",
        "method": "exit"
    });
    let _ = stdin.write_all(&encode_message(&exit));
    let _ = stdin.flush();
}

fn wait_for_exit(child: &mut std::process::Child, timeout: Duration) {
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return,
        }
    }
}

#[test]
fn test_lsp_initialize() {
    let mut child = lsp_cmd()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn boundary-lsp");

    let mut stdin = child.stdin.take().unwrap();
    let reader = LspReader::new(child.stdout.take().unwrap());

    let init_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "processId": std::process::id(),
            "capabilities": {},
            "rootUri": null
        }
    });
    stdin.write_all(&encode_message(&init_request)).unwrap();
    stdin.flush().unwrap();

    let response = reader
        .wait_for(
            |msg| msg.get("id") == Some(&json!(1)),
            Duration::from_secs(10),
        )
        .expect("should receive initialize response");

    let result = &response["result"];
    assert!(
        result.get("capabilities").is_some(),
        "should have capabilities: {response}"
    );

    let capabilities = &result["capabilities"];
    assert!(
        capabilities.get("textDocumentSync").is_some(),
        "should have textDocumentSync capability"
    );
    assert!(
        capabilities.get("hoverProvider").is_some(),
        "should have hoverProvider capability"
    );

    let server_info = &result["serverInfo"];
    assert_eq!(server_info["name"], "boundary-lsp");

    send_shutdown_and_exit(&mut stdin);
    wait_for_exit(&mut child, Duration::from_secs(5));
}

#[test]
fn test_lsp_diagnostics_on_save() {
    let fixture = go_fixture_path();
    let fixture_canonical =
        std::fs::canonicalize(&fixture).unwrap_or_else(|_| std::path::PathBuf::from(&fixture));
    let root_uri = format!("file://{}", fixture_canonical.display());

    let mut child = lsp_cmd()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn boundary-lsp");

    let mut stdin = child.stdin.take().unwrap();
    let reader = LspReader::new(child.stdout.take().unwrap());

    // Initialize with project root
    let init_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "processId": std::process::id(),
            "capabilities": {},
            "rootUri": root_uri
        }
    });
    stdin.write_all(&encode_message(&init_request)).unwrap();
    stdin.flush().unwrap();

    let _ = reader
        .wait_for(
            |msg| msg.get("id") == Some(&json!(1)),
            Duration::from_secs(10),
        )
        .expect("should receive initialize response");

    // Send initialized — triggers initial analysis
    let initialized = json!({
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": {}
    });
    stdin.write_all(&encode_message(&initialized)).unwrap();
    stdin.flush().unwrap();

    // Wait for publishDiagnostics
    let diagnostics = reader
        .wait_for(
            |msg| msg.get("method") == Some(&json!("textDocument/publishDiagnostics")),
            Duration::from_secs(15),
        )
        .expect("should receive publishDiagnostics notification");

    let params = &diagnostics["params"];
    assert!(params.get("uri").is_some(), "diagnostics should have uri");
    assert!(
        params.get("diagnostics").is_some(),
        "diagnostics should have diagnostics array"
    );

    let diag_array = params["diagnostics"].as_array().unwrap();
    assert!(
        !diag_array.is_empty(),
        "should have at least one diagnostic (domain->infra violation)"
    );

    let first_diag = &diag_array[0];
    assert!(first_diag.get("range").is_some(), "should have range");
    assert!(first_diag.get("message").is_some(), "should have message");
    assert_eq!(
        first_diag.get("source"),
        Some(&json!("boundary")),
        "source should be 'boundary'"
    );

    // Test didSave triggers re-analysis
    let did_save = json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didSave",
        "params": {
            "textDocument": {
                "uri": format!("{root_uri}/internal/domain/user/bad_dependency.go")
            }
        }
    });
    stdin.write_all(&encode_message(&did_save)).unwrap();
    stdin.flush().unwrap();

    let diagnostics_after_save = reader
        .wait_for(
            |msg| msg.get("method") == Some(&json!("textDocument/publishDiagnostics")),
            Duration::from_secs(15),
        )
        .expect("should receive publishDiagnostics after didSave");

    assert!(
        diagnostics_after_save["params"]
            .get("diagnostics")
            .is_some(),
        "should have diagnostics after save"
    );

    send_shutdown_and_exit(&mut stdin);
    wait_for_exit(&mut child, Duration::from_secs(5));
}

#[test]
fn test_lsp_shutdown() {
    let mut child = lsp_cmd()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn boundary-lsp");

    let mut stdin = child.stdin.take().unwrap();
    let reader = LspReader::new(child.stdout.take().unwrap());

    // Initialize
    let init_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "processId": std::process::id(),
            "capabilities": {},
            "rootUri": null
        }
    });
    stdin.write_all(&encode_message(&init_request)).unwrap();
    stdin.flush().unwrap();

    let _ = reader
        .wait_for(
            |msg| msg.get("id") == Some(&json!(1)),
            Duration::from_secs(10),
        )
        .expect("should receive initialize response");

    // Send initialized (required for clean lifecycle)
    let initialized = json!({
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": {}
    });
    stdin.write_all(&encode_message(&initialized)).unwrap();
    stdin.flush().unwrap();

    // Small delay to let initialized processing complete
    std::thread::sleep(Duration::from_millis(200));

    // Send shutdown
    let shutdown = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "shutdown"
    });
    stdin.write_all(&encode_message(&shutdown)).unwrap();
    stdin.flush().unwrap();

    let shutdown_response = reader
        .wait_for(
            |msg| msg.get("id") == Some(&json!(2)),
            Duration::from_secs(5),
        )
        .expect("should receive shutdown response");

    assert!(
        shutdown_response.get("result").is_some(),
        "shutdown should return result: {shutdown_response}"
    );

    // Send exit
    let exit = json!({
        "jsonrpc": "2.0",
        "method": "exit"
    });
    stdin.write_all(&encode_message(&exit)).unwrap();
    stdin.flush().unwrap();

    // Drop stdin to close the pipe — tower-lsp exits when stdin closes
    drop(stdin);

    // Wait for exit
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => {
                // tower-lsp may exit with 0 or non-zero after exit notification;
                // the important thing is that it exited.
                break;
            }
            Ok(None) => {
                if start.elapsed() > Duration::from_secs(5) {
                    let _ = child.kill();
                    panic!("boundary-lsp did not exit within timeout after shutdown+exit");
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => panic!("error waiting for boundary-lsp: {e}"),
        }
    }
}
