use crate::email_client::EmailClient;
use crate::routes::get_confirmed_subscribers;
use crate::utils::{e500, see_other};
use actix_web::web;
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
}

#[derive(Error, Debug)]
enum NewsletterFormDataError {
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
) -> Result<HttpResponse, actix_web::Error> {
    if let Err(e) = validate_form_data(&form.0) {
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
                        &form.0.title,
                        &form.0.content_text,
                        &form.0.content_html,
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
    Ok(see_other("/admin/dashboard"))
}

fn validate_form_data(form: &NewsletterFormData) -> Result<(), NewsletterFormDataError> {
    if form.title.trim().len() == 0 {
        return Err(NewsletterFormDataError::ValidationError(
            "Title is required field".to_string(),
        ));
    }
    if form.content_text.trim().len() == 0 {
        return Err(NewsletterFormDataError::ValidationError(
            "Content text field is required".to_string(),
        ));
    }
    if form.content_html.trim().len() == 0 {
        return Err(NewsletterFormDataError::ValidationError(
            "Content html field is required".to_string(),
        ));
    }
    Ok(())
}
