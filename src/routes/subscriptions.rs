use std::error::Error;
use std::fmt::{write, Formatter};
use actix_web::{web, HttpResponse, Responder, ResponseError};
use sqlx::{Executor, PgPool, Postgres, Transaction};
use uuid::Uuid;
use chrono::Utc;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

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

impl ResponseError for StoreTokenError {}

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

    transaction.execute(query)
        .await
        .map_err(|e| {
            tracing::error!("Can not execute query {:?}", e);
            e
        })?;

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
        .map_err(|e| {
            tracing::error!("Failed to execute query {:?}", e);
            StoreTokenError(e)
        })?;
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
) -> Result<HttpResponse, actix_web::Error>{
    let new_subscriber = match form.0.try_into() {
        Ok(new_subscriber) => new_subscriber,
        Err(_) => return Ok(HttpResponse::BadRequest().finish()),
    };
    let mut transaction = match connection_pool.begin().await {
        Ok(transaction) => transaction,
        Err(_) => return Ok(HttpResponse::InternalServerError().finish()),
    };

    let subscriber_id = match insert_subscriber(&mut transaction, &new_subscriber).await {
        Ok(subscriber_id) => subscriber_id,
        Err(_) => return Ok(HttpResponse::InternalServerError().finish()),
    };
    let subscription_token = generate_subscription_token();
    store_subscription_token(&mut transaction, &subscriber_id, &subscription_token).await?;
    if send_confirmation_email(
        &email_client,
        new_subscriber,
        base_url.as_ref(),
        subscription_token.as_str(),
    ).await.is_err() {
        return Ok(HttpResponse::InternalServerError().finish());
    }
    if transaction.commit().await.is_err() {
        return Ok(HttpResponse::InternalServerError().finish());
    }
    Ok(HttpResponse::Ok().finish())
}