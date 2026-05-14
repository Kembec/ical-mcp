//! End-to-end protocol tests by spawning the binary as a subprocess and
//! sending JSON-RPC lines over stdin.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

fn binary_path() -> std::path::PathBuf {
    let mut p = std::env::current_exe().expect("test exe path");
    // tests/<name>-<hash>  -> target/debug/<binary>
    p.pop(); // remove test exe filename
    if p.ends_with("deps") {
        p.pop();
    }
    p.push(if cfg!(windows) {
        "ical-mcp.exe"
    } else {
        "ical-mcp"
    });
    p
}

struct Server {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
}

impl Server {
    fn spawn() -> Self {
        let path = binary_path();
        assert!(path.exists(), "binary not built: {}", path.display());
        let mut child = Command::new(&path)
            .env("ICLOUD_USERNAME", "test@example.com")
            .env("ICLOUD_PASSWORD", "test-app-password")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn ical-mcp");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn send(&mut self, msg: &Value) {
        let line = serde_json::to_string(msg).unwrap();
        self.stdin.write_all(line.as_bytes()).unwrap();
        self.stdin.write_all(b"\n").unwrap();
        self.stdin.flush().unwrap();
    }

    fn send_raw(&mut self, raw: &str) {
        self.stdin.write_all(raw.as_bytes()).unwrap();
        self.stdin.write_all(b"\n").unwrap();
        self.stdin.flush().unwrap();
    }

    fn recv(&mut self) -> Value {
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        serde_json::from_str(line.trim()).expect("valid JSON response")
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait_timeout(Duration::from_secs(2));
    }
}

trait ChildWait {
    fn wait_timeout(&mut self, dur: Duration) -> std::io::Result<()>;
}
impl ChildWait for std::process::Child {
    fn wait_timeout(&mut self, dur: Duration) -> std::io::Result<()> {
        let start = std::time::Instant::now();
        loop {
            match self.try_wait()? {
                Some(_) => return Ok(()),
                None if start.elapsed() > dur => return Ok(()),
                None => std::thread::sleep(Duration::from_millis(20)),
            }
        }
    }
}

#[test]
fn test_initialize() {
    let mut s = Server::spawn();
    s.send(&json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2024-11-05"}
    }));
    let resp = s.recv();
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    assert!(resp["result"]["serverInfo"]["name"].as_str().unwrap() == "ical-mcp");
    assert!(resp["result"]["capabilities"].is_object());
    assert!(resp["result"]["capabilities"]["tools"].is_object());
}

#[test]
fn test_tools_list() {
    let mut s = Server::spawn();
    s.send(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
    let resp = s.recv();
    let tools = resp["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 5);
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    for expected in [
        "list-calendars",
        "get-events",
        "create-event",
        "update-event",
        "delete-event",
    ] {
        assert!(names.contains(&expected), "missing tool: {expected}");
    }
}

#[test]
fn test_unknown_method() {
    let mut s = Server::spawn();
    s.send(&json!({"jsonrpc": "2.0", "id": 3, "method": "no/such/method"}));
    let resp = s.recv();
    assert_eq!(resp["error"]["code"], -32601);
}

#[test]
fn test_invalid_json() {
    let mut s = Server::spawn();
    s.send_raw("{this is not valid json");
    let resp = s.recv();
    assert_eq!(resp["error"]["code"], -32700);
}

#[test]
fn test_notification_no_response() {
    let mut s = Server::spawn();
    // Notification (no id) — must not respond.
    s.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    // Follow with a request to verify the server is still alive.
    s.send(&json!({"jsonrpc": "2.0", "id": 99, "method": "tools/list"}));
    let resp = s.recv();
    assert_eq!(resp["id"], 99);
}
