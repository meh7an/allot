//! The client inside Claude Code's sandbox. Its own test binary, because it sets the variable
//! the sandbox sets, which every other test that starts a daemon would see.

use std::{
    env,
    net::{Ipv4Addr, TcpListener},
    path::PathBuf,
    time::Duration,
};

use allot_client::{
    Client, ClientError, ClientOptions, DEFAULT_CONNECT_TIMEOUT, DEFAULT_REQUEST_TIMEOUT,
    SpawnOptions, TokenSource,
};

#[tokio::test]
async fn test_client_inside_claude_sandbox_does_not_start_allotd() {
    // Bound and dropped, so nothing listens there.
    let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let client = Client::new(ClientOptions {
        base_url: format!("http://127.0.0.1:{port}"),
        token: TokenSource::Fixed("sandbox-test-token".into()),
        connect_timeout: DEFAULT_CONNECT_TIMEOUT,
        request_timeout: DEFAULT_REQUEST_TIMEOUT,
        spawn: Some(SpawnOptions {
            daemon_exe: PathBuf::from("no-such-allotd"),
            ready_timeout: Duration::from_millis(200),
        }),
    })
    .unwrap();

    // SAFETY: this binary's only test, so no other thread reads the environment meanwhile.
    unsafe { env::set_var("SANDBOX_RUNTIME", "1") };
    let error = client.ensure_running().await.unwrap_err();

    assert!(matches!(error, ClientError::Sandboxed), "{error}");
    assert!(
        error.to_string().contains("Claude Code's sandbox"),
        "{error}"
    );
}
