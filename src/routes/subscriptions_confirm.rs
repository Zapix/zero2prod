use actix_web::{HttpResponse, web};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct Parameters {
    subscription_token: String,
}


#[tracing::instrument(
    name = "Retrieve subscriber_id by token",
    skip(db_pool, subscription_token)
)]
pub async fn get_subscriber_id_from_token(db_pool: &PgPool, subscription_token: &str) -> Result<Option<Uuid>, sqlx::Error> {
   let result = sqlx::query!(
       "SELECT subscriber_id FROM subscription_tokens \
       WHERE subscription_token = $1",
       subscription_token
   )
       .fetch_optional(db_pool)
       .await
       .map_err(|e| {
           tracing::error!("Failed to execute query: {:?}", e);
           e
       })?;

   Ok(result.map(|r| r.subscriber_id))
}

#[tracing::instrument(
    name = "Mark subscriber as confirmed",
    skip(db_pool, subscriber_id)
)]
pub async fn confirm_subscriber(
    db_pool: &PgPool,
    subscriber_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
       r#"UPDATE subscriptions SET status = 'confirmed' WHERE id = $1"#,
       subscriber_id
    )
       .execute(db_pool)
       .await
       .map_err(|e| {
           tracing::error!("Failed to execute query: {:?}", e);
           e
       })?;
    Ok(())
}

#[tracing::instrument(
    name = "Confirm a pending subscriber",
    skip(db_pool, parameters)
)]
pub async fn confirm(db_pool: web::Data<PgPool>, parameters: web::Query<Parameters>) -> HttpResponse {
    let id = match get_subscriber_id_from_token(&db_pool, parameters.subscription_token.as_str()).await {
        Ok(id) => id,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };
    match id {
        None => HttpResponse::Unauthorized().finish(),
        Some(subscriber_id) => {
            match confirm_subscriber(&db_pool, subscriber_id).await {
                Err(_) => HttpResponse::InternalServerError().finish(),
                Ok(()) => HttpResponse::Ok().finish(),
            }
        }
    }
}