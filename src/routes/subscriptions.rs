use std::fmt::{Formatter};
use actix_web::{web, HttpResponse, ResponseError};
use actix_web::http::StatusCode;
use sqlx::{Executor, PgPool, Postgres, Transaction};
use uuid::Uuid;
use chrono::Utc;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use anyhow::Context;
use crate::domain::{SubscriberName, NewSubscriber, SubscriberEmail};
use crate::email_client::EmailClient;
use crate::startup::ApplicationBaseUrl;

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String
}

impl TryFrom<FormData> for NewSubscriber {
    type Error = String;

    fn try_from(value: FormData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse(value.name)?;
        let email = SubscriberEmail::parse(value.email)?;
        Ok(Self { email, name})
    }
}

fn chain_error_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}

pub struct StoreTokenError(sqlx::Error);

impl std::fmt::Debug for StoreTokenError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        chain_error_fmt(self, f)
    }
}

impl std::error::Error for StoreTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl std::fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered while trying to store a subscription token."
        )
    }
}


#[derive(thiserror::Error)]
pub enum SubscribeError {
    #[error("{0}")]
    ValidationError(String),
    #[error(transparent)]
    UnexepctedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for SubscribeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        chain_error_fmt(self, f)
    }
}
impl ResponseError for SubscribeError {
    fn status_code(&self) -> StatusCode {
        match self {
            SubscribeError::ValidationError(_) => StatusCode::BAD_REQUEST,
            SubscribeError::UnexepctedError(_)=> StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .json(self.to_string())
    }
}

fn generate_subscription_token() -> String {
    let mut rng = thread_rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(form, transaction)
)]
pub async fn insert_subscriber(
    transaction: &mut Transaction<'_, Postgres>,
    form: &NewSubscriber,
) -> Result<Uuid, sqlx::Error> {
    let subscriber_id = Uuid::new_v4();
    let query = sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at, status)
        VALUES ($1, $2, $3, $4, 'pending_confirmation')
        "#,
        subscriber_id,
        form.email.as_ref(),
        form.name.as_ref(),
        Utc::now(),
    );

    transaction.execute(query).await?;
    Ok(subscriber_id)
}


#[tracing::instrument(
    name = "Send a confirmation email to a new subscriber",
    skip(email_client, new_subscriber)
)]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
    base_url: &ApplicationBaseUrl,
    subscription_token: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        base_url.0,
        subscription_token,
    );
    let html_body = format!(
        "Welcome to our newsletter!<br />\
        Click <a href=\"{}\">here</a> to confirm subscription.",
        confirmation_link
    );
    let text_body= format!(
        "Welcome to our newsletter! Visit {} to confirm your subscription.",
        confirmation_link
    );
    email_client
        .send_email(
            new_subscriber.email,
            "Welcome!",
            html_body.as_str(),
            text_body.as_str(),
        )
        .await
}

#[tracing::instrument(
    name = "Storing subscription in db",
    skip(transaction, subscription_token)
)]
pub async fn store_subscription_token(transaction: &mut Transaction<'_, Postgres>, subscriber_id: &Uuid, subscription_token: &str) -> Result<(), StoreTokenError> {
    let query = sqlx::query!(
        r#"INSERT INTO subscription_tokens (subscription_token, subscriber_id) VALUES ($1, $2)"#,
        subscription_token,
        subscriber_id
    );
    transaction
        .execute(query)
        .await
        .map_err(StoreTokenError)?;
    Ok(())
}

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, connection_pool, email_client),
    fields(
        subscriber_email = %form.email,
        subscriber_name = %form.name,
    )
)]
pub async fn subscribe(
    form: web::Form<FormData>,
    connection_pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    base_url: web::Data<ApplicationBaseUrl>
) -> Result<HttpResponse, SubscribeError>{
    let new_subscriber = form.0.try_into().map_err(SubscribeError::ValidationError)?;
    let mut transaction = connection_pool.begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool.")?;

    let subscriber_id = insert_subscriber(&mut transaction, &new_subscriber)
        .await
        .context(
            "Failed to insert new subscriber in the database."
        )?;

    let subscription_token = generate_subscription_token();
    store_subscription_token(&mut transaction, &subscriber_id, &subscription_token)
        .await
        .context("Failed to store subscription token in the database.")?;

    send_confirmation_email(
        &email_client,
        new_subscriber,
        base_url.as_ref(),
        subscription_token.as_str(),
    )
        .await
        .context("Failed to send a confirmation email.")?;

    transaction.commit()
        .await
        .context("Failed to commit SQL transaction.")?;

    Ok(HttpResponse::Ok().finish())
}