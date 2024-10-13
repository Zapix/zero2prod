use crate::authentication::{
    change_password as change_password_in_db, validate_credentials, AuthError, Credentials, UserId,
};
use crate::routes::admin::dashboard::get_username;
use crate::session_state::TypedSession;
use crate::utils::{e500, see_other};
use actix_web::error::InternalError;
use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use secrecy::{ExposeSecret, Secret};
use sqlx::PgPool;
use std::fmt::Display;
use thiserror::Error;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    current_password: Secret<String>,
    new_password: Secret<String>,
    new_password_check: Secret<String>,
}

pub async fn change_password(
    pool: web::Data<PgPool>,
    form: web::Form<FormData>,
    user_id: web::ReqData<UserId>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();
    if form.new_password.expose_secret() != form.new_password_check.expose_secret() {
        FlashMessage::error(
            "You entered two different new passwords - the fields values must match.",
        )
        .send();
        return Ok(see_other("/admin/password"));
    }
    let username = get_username(&pool, *user_id).await.map_err(e500)?;
    let credentials = Credentials {
        username,
        password: form.0.current_password,
    };
    if let Err(e) = validate_credentials(credentials, &pool).await {
        return match e {
            AuthError::InvalidCredentials(_) => {
                FlashMessage::error("The current password is incorrect.").send();
                Ok(see_other("/admin/password"))
            }
            AuthError::UnexpectedError(_) => Err(e500(e).into()),
        };
    }
    if let Err(e) = validate_password(form.0.new_password) {
        FlashMessage::error(format!("{}", e.to_string())).send();
        return Ok(see_other("/admin/password"));
    }
    change_password_in_db(&pool, *user_id, form.0.new_password_check)
        .await
        .map_err(e500)?;
    FlashMessage::info("Your password has been changed.").send();
    Ok(see_other("/admin/password"))
}

#[derive(Error, Debug)]
pub enum PasswordValidationError {
    #[error("{0}")]
    InvalidPassword(String),
}

fn validate_password(password: Secret<String>) -> Result<(), PasswordValidationError> {
    if password.expose_secret().len() <= 8 {
        return Err(PasswordValidationError::InvalidPassword(
            "Password is too short.".to_string(),
        ));
    }
    if password.expose_secret().len() > 129 {
        return Err(PasswordValidationError::InvalidPassword(
            "Password is to long.".to_string(),
        ));
    }

    Ok(())
}

fn reject_anonymous_users(session: TypedSession) -> Result<Uuid, actix_web::Error> {
    match session.get_user_id().map_err(e500)? {
        Some(user_id) => Ok(user_id),
        None => {
            let response = see_other("/login");
            let e = anyhow::anyhow!("The user has not logged in");
            Err(InternalError::from_response(e, response).into())
        }
    }
}
