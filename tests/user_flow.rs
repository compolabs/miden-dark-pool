use std::process::Command;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::sleep;

#[tokio::test]
async fn test_user_flow() {
    // Launch test matcher server in background
    tokio::spawn(async {
        let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
        let (_, _) = listener.accept().await.unwrap();
    });

    // Wait to ensure matcher is ready
    sleep(Duration::from_secs(5)).await;

    // Call the user binary
    let output = Command::new("cargo")
        .args([
            "run",
            "--release",
            "--bin",
            "user",
            "--",
            "--user",
            "testuser",
            "--token-a",
            "ETH",
            "--amount-a",
            "50",
            "--token-b",
            "USDC",
            "--matcher-addr",
            "127.0.0.1:8080",
        ])
        .output()
        .expect("Failed to execute user binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("stdout:\n{}", stdout);
    println!("stderr:\n{}", stderr);

    assert!(output.status.success(), "User binary failed");
    assert!(stdout.contains("Note sent!"));
}
