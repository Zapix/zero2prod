use crate::session_state::TypedSession;
use actix_web::http::header::LOCATION;
use actix_web::web;
use actix_web::HttpResponse;
use anyhow::Context;
use sqlx::PgPool;
use uuid::Uuid;

fn e500<T>(e: T) -> actix_web::Error
where
    T: std::fmt::Debug + std::fmt::Display + 'static,
{
    actix_web::error::ErrorInternalServerError(e)
}
pub async fn admin_dashboard(
    session: TypedSession,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    let username = if let Some(user_id) = session.get_user_id().map_err(e500)? {
        get_username(&pool, user_id).await.map_err(e500)?
    } else {
        return Ok(HttpResponse::SeeOther()
            .insert_header((LOCATION, "/login"))
            .finish());
    };

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
</body>
</html>
    "#,
    )))
}

#[tracing::instrument(
    name = "Get username"
    skip(pool)
)]
async fn get_username(pool: &PgPool, user_id: Uuid) -> Result<String, anyhow::Error> {
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
