use crate::helpers::{assert_is_redirect_to, spawn_app};
use crate::newsletter::{create_confirmed_subscriber, create_unconfirmed_subscriber};

use wiremock::matchers::{any, method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn can_not_get_access_without_authorization() {
    let app = spawn_app().await;

    let response = app.get_newsletters().await;

    assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn get_access_to_html_page() {
    let app = spawn_app().await;

    let login_body = serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password,
    });
    let response = app.post_login(&login_body).await;
    assert_is_redirect_to(&response, "/admin/dashboard");

    let response = app.get_newsletters().await;
    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
async fn post_send_newsletters_title_is_required() {
    let app = spawn_app().await;

    let login_body = serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password,
    });

    let response = app.post_login(&login_body).await;
    assert_is_redirect_to(&response, "/admin/dashboard");

    let response = app
        .post_newsletter(serde_json::json!({
            "title": "",
            "content_text": "",
            "content_html": "",
            "idempotency_key": uuid::Uuid::new_v4().to_string(),
        }))
        .await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    let page_html = app.get_newsletter_html().await;
    assert!(page_html.contains("Title is required field"));
}

#[tokio::test]
async fn post_send_newsletters_content_text_is_required() {
    let app = spawn_app().await;

    let login_body = serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password,
    });

    let response = app.post_login(&login_body).await;
    assert_is_redirect_to(&response, "/admin/dashboard");

    let response = app
        .post_newsletter(serde_json::json!({
            "title": "Rust Weekly",
            "content_text": "",
            "content_html": "",
            "idempotency_key": uuid::Uuid::new_v4().to_string(),
        }))
        .await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    let page_html = app.get_newsletter_html().await;
    assert!(page_html.contains("Content text field is required"));
}

#[tokio::test]
async fn post_send_newsletters_content_html_is_required() {
    let app = spawn_app().await;

    let login_body = serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password,
    });

    let response = app.post_login(&login_body).await;
    assert_is_redirect_to(&response, "/admin/dashboard");

    let response = app
        .post_newsletter(serde_json::json!({
            "title": "Rust Weekly",
            "content_text": "New content",
            "content_html": "",
            "idempotency_key": uuid::Uuid::new_v4().to_string(),
        }))
        .await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    let page_html = app.get_newsletter_html().await;
    assert!(page_html.contains("Content html field is required"));
}

#[tokio::test]
async fn newsletters_are_delivered_to_confirmed_subscribers() {
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let login_body = serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password,
    });

    let response = app.post_login(&login_body).await;
    assert_is_redirect_to(&response, "/admin/dashboard");

    let response = app
        .post_newsletter(serde_json::json!({
            "title": "Rust Weekly",
            "content_text": "Weekly news",
            "content_html": "<p>Weekly news</p>",
            "idempotency_key": uuid::Uuid::new_v4().to_string(),
        }))
        .await;
    assert_is_redirect_to(&response, "/admin/dashboard");

    let page_html = app.get_admin_dashboard_html().await;
    assert!(page_html.contains("Newsletters were sent to subscribers!"));
}

#[tokio::test]
async fn newsletters_are_not_delivered_to_pending_subscribers() {
    let app = spawn_app().await;
    create_unconfirmed_subscriber(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&app.email_server)
        .await;

    let login_body = serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password,
    });

    let response = app.post_login(&login_body).await;
    assert_is_redirect_to(&response, "/admin/dashboard");

    let response = app
        .post_newsletter(serde_json::json!({
            "title": "Rust Weekly",
            "content_text": "Weekly news",
            "content_html": "<p>Weekly news</p>",
            "idempotency_key": uuid::Uuid::new_v4().to_string(),
        }))
        .await;
    assert_is_redirect_to(&response, "/admin/dashboard");

    let page_html = app.get_admin_dashboard_html().await;
    assert!(page_html.contains("Newsletters were sent to subscribers!"));
}

#[tokio::test]
async fn newsletter_creation_is_idempotent() {
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;

    let login_body = serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password,
    });

    let response = app.post_login(&login_body).await;
    assert_is_redirect_to(&response, "/admin/dashboard");

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "content_text": "Hello World!",
        "content_html": "<b>Hello World!</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });

    // Send newsletters first time
    let response = app.post_newsletter(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/dashboard");
    let page_html = app.get_admin_dashboard_html().await;
    assert!(page_html.contains("Newsletters were sent to subscribers!"));

    // Send newsletters second time
    let response = app.post_newsletter(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/dashboard");
    // Mock have to raise an error
}
