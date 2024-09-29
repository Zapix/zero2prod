use crate::helpers::{spawn_app, ConfirmationLinks, TestApp};
use wiremock::matchers::{any, method, path};
use wiremock::{Mock, ResponseTemplate};


#[tokio::test]
async fn newsletter_are_not_delivered_to_pending_subscribers() {
    let app = spawn_app().await;
    create_unconfirmed_subscriber(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title": "newsletter title",
        "content": {
            "text": "newsletter content",
            "html": "<p>newsletter content</p>"
        }
    });

    let response = app.publish_newsletter(newsletter_request_body).await;

    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
async fn newsletter_are_delivered_to_confirmed_subscribers() {
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title": "newsletter title",
        "content": {
            "text": "newsletter content",
            "html": "<p>newsletter content</p>"
        }
    });

    let response = app.publish_newsletter(newsletter_request_body).await;

    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
async fn newsletters_returns_a_400_for_invalid_data() {
    let app = spawn_app().await;
    let test_cases = vec![
        (
            serde_json::json!({
                "content": {
                    "text": "newsletter content",
                    "html": "<p>newsletter content</p>"
                }
            }),
            "title is required.",
        ),
        (
            serde_json::json!({
                "title": "newsletter title",
            }),
            "content is required.",
        ),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = app.publish_newsletter(invalid_body).await;

        assert_eq!(response.status().as_u16(), 400);
    }
}

#[tokio::test]
async fn requests_missing_authorization_are_rejected() {
    let app = spawn_app().await;

    let response = reqwest::Client::new()
        .post(&format!("{}/newsletters", &app.address))
        .json(&serde_json::json!({
            "title": "Newsletter title",
            "content": {
                "text": "Newsletter body as plain text",
                "html": "<p>Newsletter body as HTML</b>",
            }
        }))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status().as_u16(), 401);
    assert_eq!(
        response.headers()["WWW-Authenticate"],
        r#"Basic realm="publish""#
    )
}

async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    let _mock_guard = Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount_as_scoped(&app.email_server)
        .await;

    app.post_subscriptions(body.into()).await.error_for_status().unwrap();
    app.get_confirmation_links(
        &app.email_server.received_requests().await.unwrap().pop().unwrap(),
    )
}


async fn create_confirmed_subscriber(app: &TestApp) {
    let confirmation_links = create_unconfirmed_subscriber(app).await;
    reqwest::get(confirmation_links.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}