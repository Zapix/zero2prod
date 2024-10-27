use crate::authentication::UserId;
use crate::utils::e500;
use actix_web::web;
use actix_web::HttpResponse;
use actix_web_flash_messages::IncomingFlashMessages;
use anyhow::Context;
use sqlx::PgPool;
use std::fmt::Write;
use uuid::Uuid;

pub async fn admin_dashboard(
    pool: web::Data<PgPool>,
    user_id: web::ReqData<UserId>,
    flash_messages: IncomingFlashMessages,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();
    let username = get_username(&pool, *user_id).await.map_err(e500)?;

    let mut html_message = String::new();
    for message in flash_messages.iter() {
        let _ = write!(html_message, "<p><i>{}</i></p>", message.content());
    }

    Ok(HttpResponse::Ok().body(format!(
        r#"
    <!DOCTYPE html>
<html lang="en">
<head>
    <meta http-equiv="content-type" content="text/html" charset="UTF-8">
    <title>Login</title>
</head>
<body>
    <p>Welcome, {username}!</p>
    {html_message}
    <ol>
        <li><a href="/admin/newsletters">Send newsletters</a></li>
        <li><a href="/admin/password">Change password</a></li>
        <li>
            <form name="logoutForm" action="/admin/logout" method="post">
                <input type="submit" value="Logout" />
            </form>
        </li>
    </ol>
</body>
</html>
    "#,
    )))
}

#[tracing::instrument(
    name = "Get username"
    skip(pool)
)]
pub async fn get_username(pool: &PgPool, user_id: Uuid) -> Result<String, anyhow::Error> {
    let row = sqlx::query!(
        r#"
        SELECT username FROM users WHERE user_id=$1
        "#,
        user_id
    )
    .fetch_one(pool)
    .await
    .context("Unable retrieve information about suer")?;

    Ok(row.username)
}
