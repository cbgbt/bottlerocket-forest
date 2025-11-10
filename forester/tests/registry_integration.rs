use std::process::Command;

/// Helper to run forester CLI and capture output
fn run_forester(args: &[&str]) -> (i32, String, String) {
    let output = Command::new("./target/release/forester")
        .args(args)
        .output()
        .expect("Failed to execute forester");
    
    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    
    (exit_code, stdout, stderr)
}

/// Helper to check if docker is available
fn docker_available() -> bool {
    Command::new("docker")
        .arg("--version")
        .output()
        .is_ok()
}

#[test]
fn test_registry_start_idempotent() {
    if !docker_available() {
        eprintln!("Skipping test: Docker not available");
        return;
    }
    
    // Given: Clean state
    let _ = run_forester(&["registry", "clean"]);
    
    // When: Start registry twice
    let (code1, stdout1, _) = run_forester(&["registry", "start"]);
    let (code2, stdout2, _) = run_forester(&["registry", "start"]);
    
    // Then: Both succeed
    assert_eq!(code1, 0, "First start should succeed");
    assert_eq!(code2, 0, "Second start should succeed (idempotent)");
    assert!(stdout1.contains("localhost:5000") || stdout1.contains("5000"));
    assert!(stdout2.contains("localhost:5000") || stdout2.contains("5000"));
    
    // Cleanup
    let _ = run_forester(&["registry", "clean"]);
}

#[test]
fn test_registry_status_not_created() {
    if !docker_available() {
        eprintln!("Skipping test: Docker not available");
        return;
    }
    
    // Given: Clean state
    let _ = run_forester(&["registry", "clean"]);
    
    // When: Check status
    let (code, stdout, _) = run_forester(&["registry", "status"]);
    
    // Then: Reports not created
    assert_ne!(code, 0, "Status should return non-zero when not running");
    assert!(
        stdout.contains("not created") || stdout.contains("not running") || stdout.contains("Not"),
        "Should indicate registry is not created"
    );
}

#[test]
fn test_registry_status_running() {
    if !docker_available() {
        eprintln!("Skipping test: Docker not available");
        return;
    }
    
    // Given: Registry is running
    let _ = run_forester(&["registry", "clean"]);
    let _ = run_forester(&["registry", "start"]);
    
    // When: Check status
    let (code, stdout, _) = run_forester(&["registry", "status"]);
    
    // Then: Reports running with URL
    assert_eq!(code, 0, "Status should return 0 when running");
    assert!(stdout.contains("running") || stdout.contains("Running"));
    assert!(stdout.contains("localhost:5000") || stdout.contains("5000"));
    
    // Cleanup
    let _ = run_forester(&["registry", "clean"]);
}

#[test]
fn test_registry_stop() {
    if !docker_available() {
        eprintln!("Skipping test: Docker not available");
        return;
    }
    
    // Given: Registry is running
    let _ = run_forester(&["registry", "clean"]);
    let _ = run_forester(&["registry", "start"]);
    
    // When: Stop registry
    let (code, _, _) = run_forester(&["registry", "stop"]);
    
    // Then: Stops successfully
    assert_eq!(code, 0, "Stop should succeed");
    
    // And: Status shows stopped
    let (status_code, _, _) = run_forester(&["registry", "status"]);
    assert_ne!(status_code, 0, "Status should return non-zero when stopped");
    
    // Cleanup
    let _ = run_forester(&["registry", "clean"]);
}

#[test]
fn test_registry_stop_idempotent() {
    if !docker_available() {
        eprintln!("Skipping test: Docker not available");
        return;
    }
    
    // Given: Clean state
    let _ = run_forester(&["registry", "clean"]);
    
    // When: Stop when not running
    let (code, _, _) = run_forester(&["registry", "stop"]);
    
    // Then: Succeeds (idempotent)
    assert_eq!(code, 0, "Stop should succeed even when not running");
}

#[test]
fn test_registry_clean() {
    if !docker_available() {
        eprintln!("Skipping test: Docker not available");
        return;
    }
    
    // Given: Registry is running
    let _ = run_forester(&["registry", "start"]);
    
    // When: Clean registry
    let (code, _, _) = run_forester(&["registry", "clean"]);
    
    // Then: Succeeds
    assert_eq!(code, 0, "Clean should succeed");
    
    // And: Status shows not created
    let (status_code, _, _) = run_forester(&["registry", "status"]);
    assert_ne!(status_code, 0, "Status should return non-zero after clean");
}

#[test]
fn test_registry_logs() {
    if !docker_available() {
        eprintln!("Skipping test: Docker not available");
        return;
    }
    
    // Given: Registry is running
    let _ = run_forester(&["registry", "clean"]);
    let _ = run_forester(&["registry", "start"]);
    
    // When: Get logs
    let (code, stdout, _) = run_forester(&["registry", "logs"]);
    
    // Then: Succeeds and shows logs
    assert_eq!(code, 0, "Logs should succeed");
    assert!(!stdout.is_empty(), "Should output logs");
    
    // Cleanup
    let _ = run_forester(&["registry", "clean"]);
}

#[test]
fn test_registry_custom_port() {
    if !docker_available() {
        eprintln!("Skipping test: Docker not available");
        return;
    }
    
    // Given: Custom port via environment
    let _ = run_forester(&["registry", "clean"]);
    
    let output = Command::new("./target/release/forester")
        .env("FORESTER_REGISTRY_PORT", "5001")
        .args(&["registry", "start"])
        .output()
        .expect("Failed to execute forester");
    
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    
    // Then: Uses custom port
    assert!(stdout.contains("5001"), "Should use custom port 5001");
    
    // Cleanup with custom port
    let _ = Command::new("./target/release/forester")
        .env("FORESTER_REGISTRY_PORT", "5001")
        .args(&["registry", "clean"])
        .output();
}
