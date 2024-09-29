use actix_web::{web, HttpRequest, HttpResponse, ResponseError};
use actix_web::http::{header, StatusCode};
use actix_web::http::header::{HeaderMap, HeaderValue};
use anyhow::Context;
use secrecy::Secret;
use sqlx::PgPool;
use base64::{decode, Engine};

use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::routes::helpers::chain_error_fmt;

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[tracing::instrument(name = "Getting confirmed subscribers", skip(pool))]
async fn get_confirmed_subscribers(pool: &PgPool) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    let confirmed_subscribers = sqlx::query!(
        r#"
            SELECT email
            FROM subscriptions
            WHERE status = 'confirmed'
        "#
    )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| {
            match SubscriberEmail::parse(row.email) {
                Ok(email) => Ok(ConfirmedSubscriber { email }),
                Err(error) => Err(anyhow::anyhow!(error)),
            }
        })
        .collect();
    Ok(confirmed_subscribers)
}

#[derive(serde::Deserialize)]
pub struct BodyData {
    title: String,
    content: Content,
}

#[derive(serde::Deserialize)]
pub struct Content {
    text: String,
    html: String,
}

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error("Authentication")]
    AuthError(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        chain_error_fmt(self, f)
    }
}

impl ResponseError for PublishError {
    fn status_code(&self) -> StatusCode {
        match self {
            PublishError::AuthError(_) => StatusCode::UNAUTHORIZED,
            PublishError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        match self {
            PublishError::UnexpectedError(_) => {
                HttpResponse::new(self.status_code())
            },
            PublishError::AuthError(_) => {
                let mut response = HttpResponse::new(self.status_code());
                let header_value = HeaderValue::from_str(
                    r#"Basic realm="publish""#
                ).unwrap();
                response
                    .headers_mut()
                    .insert(header::WWW_AUTHENTICATE, header_value);
                response
            },
        }
    }
}

pub async fn publish_newsletter(
    body: web::Json<BodyData>,
    db_pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    request: HttpRequest,
) -> Result<HttpResponse, PublishError> {
    let _credentials = basic_authentication(request.headers()).map_err(PublishError::AuthError)?;
    let subscribers = get_confirmed_subscribers(&db_pool)
        .await
        .map_err(PublishError::UnexpectedError)?;
    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client.send_email(
                    &subscriber.email,
                    &body.title,
                    &body.content.text,
                    &body.content.html
                )
                    .await
                    .with_context(|| format!("Failed to send newsletter issue to {}", subscriber.email))
                    .map_err(PublishError::UnexpectedError)?;
            },
            Err(error) => {
                tracing::warn!(
                    error.cause_chain = ?error,
                    "Skipping an invalid subscriber email. Their stored contact details are invalid"
                )
            }
        }
    }
    Ok(HttpResponse::Ok().finish())
}

struct Credentials {
    username: String,
    password: Secret<String>,
}

fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    let header_value = headers
        .get("Authorization")
        .context("The 'Auhtorization' header was missing")?
        .to_str()
        .context("The 'Authorization' header was not a valid UTF8 string")?;

    let base64encoded_segment = header_value
        .strip_prefix("Basic ")
        .context("The authorization scheme was not 'Basic'.")?;

    let decoded_bytes = base64::engine::general_purpose::STANDARD
        .decode(base64encoded_segment)
        .context("Failed to base64-decode 'Basic' credentials.")?;
    let decoded_credentials = String::from_utf8(decoded_bytes)
        .context("The decoded credential string is not valid UTF8.")?;

    let mut credentials = decoded_credentials.splitn(2, ':');
    let username = credentials
        .next()
        .ok_or_else(|| {
           anyhow::anyhow!("A username must be provided in 'Basic auth.")
        })?
        .to_string();
    let password = credentials
        .next()
        .ok_or_else(|| {
            anyhow::anyhow!("A password must be provided in 'Basic' auth.")
        })?
        .to_string();

    Ok(Credentials {
        username,
        password: Secret::new(password)
    })
}