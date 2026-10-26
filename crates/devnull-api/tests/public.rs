use devnull_api::Client;

/// Hits production. Not part of CI: `cargo test -p devnull-api -- --ignored`.
#[tokio::test]
#[ignore = "network"]
async fn public_user_and_commits_decode() {
    let client = Client::unsigned().expect("http client");
    let user = client.user("nunomaduro").await.expect("public user");
    assert_eq!(user.username, "nunomaduro");
    let page = client
        .user_commits("nunomaduro", false, None)
        .await
        .expect("public commits");
    assert!(!page.data.is_empty());
    assert!(page.data[0].short_hash.is_some());
}
