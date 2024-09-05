use actix_web::{web, HttpResponse, Responder};
use sqlx::{PgPool};
use uuid::Uuid;
use chrono::Utc;

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String
}

pub async fn subscribe(form: web::Form<FormData>, connection_pool: web::Data<PgPool>) -> impl Responder {
    let _ = sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at)
        VALUES ($1, $2, $3, $4)
        "#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now(),
    )
        .execute(connection_pool.get_ref())
        .await;
    HttpResponse::Ok()
}