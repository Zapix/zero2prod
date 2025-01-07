use crate::authentication::UserId;
use crate::idempotency::{save_response, try_processing, IdempotencyKey, NextAction};
use crate::utils::{e400, e500, see_other};
use actix_web::web;
use actix_web::web::ReqData;
use actix_web::HttpResponse;
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use sqlx::{Executor, PgPool, Postgres, Transaction};
use thiserror::Error;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct NewsletterFormData {
    title: String,
    content_text: String,
    content_html: String,
    idempotency_key: String,
}

struct NewsletterData {
    title: String,
    content_text: String,
    content_html: String,
}

impl NewsletterData {
    fn from(title: String, content_text: String, content_html: String) -> Self {
        Self {
            title,
            content_text,
            content_html,
        }
    }
}

#[derive(Error, Debug)]
enum NewsletterDataError {
    #[error("{0}")]
    ValidationError(String),
}

#[tracing::instrument(
    name = "Publish a newsletter",
    skip(db_pool, form),
    fields(user_id=%&*user_id)
)]
pub async fn publish_newsletter(
    db_pool: web::Data<PgPool>,
    form: web::Form<NewsletterFormData>,
    user_id: ReqData<UserId>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();
    let NewsletterFormData {
        idempotency_key,
        title,
        content_text,
        content_html,
    } = form.0;
    let newsletter_data = NewsletterData::from(title, content_text, content_html);
    if let Err(e) = validate_form_data(&newsletter_data) {
        FlashMessage::error(format!("{}", e.to_string())).send();
        return Ok(see_other("/admin/newsletters"));
    }
    let idempotency_key: IdempotencyKey = idempotency_key.try_into().map_err(e400)?;
    let mut transaction = match try_processing(&db_pool, &idempotency_key, *user_id)
        .await
        .map_err(e500)?
    {
        NextAction::StartProcessing(t) => t,
        NextAction::ReturnSavedResponse(response) => {
            success_message().send();
            return Ok(response);
        }
    };
    let issue_id = insert_newsletter_issue(
        &mut transaction,
        newsletter_data.title.as_str(),
        newsletter_data.content_text.as_str(),
        newsletter_data.content_text.as_str(),
    )
    .await
    .context("Failed to store newsletter issue details.")
    .map_err(e500)?;
    enqueue_delivery_tasks(&mut transaction, issue_id)
        .await
        .context("Failed to enqueue delivery tasks")
        .map_err(e500)?;

    success_message().send();
    let response = see_other("/admin/dashboard");
    let response = save_response(transaction, &idempotency_key, *user_id, response)
        .await
        .map_err(e500)?;
    Ok(response)
}

fn success_message() -> FlashMessage {
    FlashMessage::info("Newsletters were sent to subscribers!")
}

fn validate_form_data(data: &NewsletterData) -> Result<(), NewsletterDataError> {
    if data.title.trim().len() == 0 {
        return Err(NewsletterDataError::ValidationError(
            "Title is required field".to_string(),
        ));
    }
    if data.content_text.trim().len() == 0 {
        return Err(NewsletterDataError::ValidationError(
            "Content text field is required".to_string(),
        ));
    }
    if data.content_html.trim().len() == 0 {
        return Err(NewsletterDataError::ValidationError(
            "Content html field is required".to_string(),
        ));
    }
    Ok(())
}

#[tracing::instrument(skip_all)]
async fn insert_newsletter_issue(
    transaction: &mut Transaction<'_, Postgres>,
    title: &str,
    text_content: &str,
    html_content: &str,
) -> Result<Uuid, sqlx::Error> {
    let newsletter_issue_id = Uuid::new_v4();
    let query = sqlx::query!(
        r#"
        INSERT INTO newsletter_issues (
            newsletter_issue_id,
            title,
            text_content,
            html_content,
            published_at
        )
        VALUES ($1, $2, $3, $4, now())
        "#,
        newsletter_issue_id,
        title,
        text_content,
        html_content
    );
    transaction.execute(query).await?;
    Ok(newsletter_issue_id)
}

#[tracing::instrument(skip_all)]
async fn enqueue_delivery_tasks(
    transaction: &mut Transaction<'_, Postgres>,
    newsletter_issue_id: Uuid,
) -> Result<(), sqlx::Error> {
    let query = sqlx::query!(
        r#"
        INSERT INTO issue_delivery_queue (
            newsletter_issue_id,
            subscriber_email
        )
        SELECT $1, email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
        newsletter_issue_id
    );
    transaction.execute(query).await?;
    Ok(())
}
