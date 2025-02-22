use crate::helpers::{spawn_app, ConfirmationLinks, TestApp};
use fake::faker::internet::en::SafeEmail;
use fake::faker::name::en::Name;
use fake::Fake;
use uuid::Uuid;
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
    app.dispatch_all_pending_emails().await;
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
    app.dispatch_all_pending_emails().await;
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

        assert_eq!(response.status().as_u16(), 400, "{}", error_message);
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
    );
}

#[tokio::test]
async fn non_existing_user_is_rejected() {
    let mut app = spawn_app().await;
    app.test_user.username = Uuid::new_v4().to_string();
    app.test_user.password = Uuid::new_v4().to_string();

    let newsletter_request_body = serde_json::json!({
        "title": "newsletter title",
        "content": {
            "text": "newsletter content",
            "html": "<p>newsletter content</p>"
        }
    });

    let response = app.publish_newsletter(newsletter_request_body).await;

    assert_eq!(response.status().as_u16(), 401);
    assert_eq!(
        response.headers()["WWW-Authenticate"],
        r#"Basic realm="publish""#
    );
}

#[tokio::test]
async fn invalid_password_is_rejected() {
    let mut app = spawn_app().await;
    app.test_user.password = Uuid::new_v4().to_string();

    let newsletter_request_body = serde_json::json!({
        "title": "newsletter title",
        "content": {
            "text": "newsletter content",
            "html": "<p>newsletter content</p>"
        }
    });

    let response = app.publish_newsletter(newsletter_request_body).await;

    assert_eq!(response.status().as_u16(), 401);
    assert_eq!(
        response.headers()["WWW-Authenticate"],
        r#"Basic realm="publish""#
    );
}

pub async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
    let name: String = Name().fake();
    let email: String = SafeEmail().fake();
    let body = serde_urlencoded::to_string(&serde_json::json!({
        "name": name,
        "email": email
    }))
    .unwrap();

    let _mock_guard = Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount_as_scoped(&app.email_server)
        .await;

    app.post_subscriptions(body.into())
        .await
        .error_for_status()
        .unwrap();
    app.get_confirmation_links(
        &app.email_server
            .received_requests()
            .await
            .unwrap()
            .pop()
            .unwrap(),
    )
}

pub async fn create_confirmed_subscriber(app: &TestApp) {
    let confirmation_links = create_unconfirmed_subscriber(app).await;
    reqwest::get(confirmation_links.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}
