// run cargo build && RUSTUP_FORCE_ARG0=rustup ./target/debug/rustup-init --concurrent show ; need the concurrent flag
use std::env;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use rustup::diskio::IOPriority;

// Test that we can run the rustup command with the concurrent flag
#[tokio::test]
async fn concurrent_flag_enables_concurrency() {
    let result = std::process::Command::new("cargo")
        .args(["run", "--", "--concurrent", "show"])
        .output()
        .expect("Failed to execute command");
    
    // Make sure the command ran successfully
    assert!(result.status.success(), 
        "Command failed with stderr: {}", 
        String::from_utf8_lossy(&result.stderr));
    
    // The output should not contain errors about the --concurrent flag
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(!stderr.contains("error: unexpected argument '--concurrent'"));
}

// Test the download priority system
#[test]
fn download_priority_test() {
    // Test that IOPriority is properly exposed and usable
    let critical = IOPriority::Critical;
    let normal = IOPriority::Normal;
    let background = IOPriority::Background;
    
    assert_ne!(critical, normal);
    assert_ne!(normal, background);
    assert_ne!(critical, background);
}
