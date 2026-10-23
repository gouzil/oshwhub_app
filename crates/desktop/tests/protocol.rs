use std::{
    io::Write,
    process::{Command, Stdio},
};

static NEXT_LOG: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn run(input: &[u8]) -> std::process::Output {
    let logs = std::env::temp_dir().join(format!(
        "oshwhub-sidecar-test-{}-{}",
        std::process::id(),
        NEXT_LOG.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let mut child = Command::new(env!("CARGO_BIN_EXE_oshwhub-desktop-core"))
        .arg("--log-dir")
        .arg(&logs)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let output = child.wait_with_output().unwrap();
    let _ = std::fs::remove_dir_all(logs);
    output
}

#[test]
fn concurrent_ids_and_business_errors_survive_stdio() {
    let output = run(concat!(
        "{\"id\":\"health\",\"input\":\"{\\\"command\\\":\\\"health\\\"}\"}\n",
        "{\"id\":\"invalid\",\"input\":\"not json\"}\n"
    )
    .as_bytes());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses: Vec<serde_json::Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(responses.len(), 2);
    for response in responses {
        let business: serde_json::Value =
            serde_json::from_str(response["output"].as_str().unwrap()).unwrap();
        match response["id"].as_str().unwrap() {
            "health" => assert_eq!(business["data"]["protocol"], 1),
            "invalid" => {
                assert_eq!(business["error"]["code"], "invalid_request");
                assert!(business["error"]["requestId"].is_string());
            }
            _ => panic!("unknown correlation id"),
        }
    }
}

#[test]
fn malformed_envelopes_do_not_become_business_requests() {
    for input in [
        b"not json\n".as_slice(),
        b"{\"id\":\"x\",\"input\":{}}\n",
        b"{\"id\":\"\",\"input\":\"{}\"}\n",
    ] {
        let output = run(input);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn request_byte_limit_allows_json_escaping_without_unbounded_frames() {
    for size in [1024 * 1024, 1024 * 1024 + 1] {
        let mut line = serde_json::json!({"id": "bounded", "input": "\0".repeat(size)}).to_string();
        line.push('\n');
        let output = run(line.as_bytes());
        if size == 1024 * 1024 {
            assert!(output.status.success());
            let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            let business: serde_json::Value =
                serde_json::from_str(envelope["output"].as_str().unwrap()).unwrap();
            assert_eq!(business["error"]["code"], "invalid_request");
        } else {
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
        }
    }
}
