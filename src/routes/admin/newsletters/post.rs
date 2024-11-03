use crate::authentication::UserId;
use crate::email_client::EmailClient;
use crate::idempotency::{get_saved_response, save_response, IdempotencyKey};
use crate::routes::get_confirmed_subscribers;
use crate::utils::{e400, e500, see_other};
use actix_web::web;
use actix_web::web::ReqData;
use actix_web::HttpResponse;
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use sqlx::PgPool;
use thiserror::Error;

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
    name = "Publish a newsletter"
    skip(db_pool, email_client, form)
)]
pub async fn publish_newsletter(
    db_pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
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
    let idempotency_key: IdempotencyKey = idempotency_key.try_into().map_err(e400)?;
    if let Some(saved_response) = get_saved_response(&db_pool, &idempotency_key, *user_id)
        .await
        .map_err(e500)?
    {
        return Ok(saved_response);
    }

    if let Err(e) = validate_form_data(&newsletter_data) {
        FlashMessage::error(format!("{}", e.to_string())).send();
        return Ok(see_other("/admin/newsletters"));
    }
    let subscribers = get_confirmed_subscribers(&db_pool)
        .await
        .map_err(|e| e500(e))?;
    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email(
                        &subscriber.email,
                        &newsletter_data.title,
                        &newsletter_data.content_text,
                        &newsletter_data.content_html,
                    )
                    .await
                    .with_context(|| {
                        format!("Failed to send newsletter issue to {}", subscriber.email)
                    })
                    .map_err(e500)?;
            }
            Err(error) => {
                tracing::warn!(
                    error.cause_chain = ?error,
                    "Skipping an invalid subscriber email. Their stored contact details are invalid"
                )
            }
        }
    }
    FlashMessage::info("Newsletters were sent to subscribers!").send();
    let response = see_other("/admin/dashboard");
    let response = save_response(&db_pool, &idempotency_key, *user_id, response)
        .await
        .map_err(e500)?;
    Ok(response)
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
