use fubbik_cli::client::Client;

#[tokio::test]
async fn sign_in_posts_credentials_and_returns_session_token() {
    // Given the email sign-in endpoint
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/auth/sign-in/email"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "email": "test@example.com", "password": "secret-password"
        })))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "token": "session-1", "user": {"email": "test@example.com"},
                "redirect": false, "url": null
            })),
        )
        .mount(&server)
        .await;
    // When the client signs in
    let response = Client::new(server.uri())
        .sign_in("test@example.com", "secret-password")
        .await
        .unwrap();
    // Then the token and user are available to the CLI
    assert_eq!(response.token, "session-1");
    assert_eq!(response.user["email"], "test@example.com");
}

#[tokio::test]
async fn saved_session_is_sent_as_a_cookie() {
    // Given a server requiring a session cookie
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/auth/get-session"))
        .and(wiremock::matchers::header(
            "cookie",
            "fubbik_session=session-1",
        ))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "session": {"id": "s1"}, "user": {"email": "test@example.com"}
            })),
        )
        .mount(&server)
        .await;
    // When the CLI requests its identity with a saved token
    let session = Client::with_token(server.uri(), Some("session-1"))
        .unwrap()
        .current_session()
        .await
        .unwrap()
        .unwrap();
    // Then the authenticated identity is returned
    assert_eq!(session["user"]["email"], "test@example.com");
}
