use actix_web::http::header::ContentType;
use actix_web::HttpResponse;
use actix_web_flash_messages::IncomingFlashMessages;
use std::fmt::Write;

pub async fn change_password_form(
    flash_messages: IncomingFlashMessages,
) -> Result<HttpResponse, actix_web::Error> {
    let mut html_message = String::new();
    for message in flash_messages.iter() {
        let _ = write!(html_message, "<p><i>{}</i></p>", message.content());
    }
    Ok(HttpResponse::Ok().content_type(ContentType::html()).body(
        format!(r#"
        <!DOCTYPE html>
            <html lang="en">
<head>
    <meta http-equiv="content-type" content="text/html" charset="UTF-8">
    <title>Login</title>
</head>
<body>
{html_message}
<form method="post" action="/admin/password">
    <label>Current password: <input type="password" placeholder="Enter current password" name="current_password" /></label>
    <label>New password: <input type="password" placeholder="Enter new password" name="new_password"/></label>
    <label>Confirm new password: <input type="password" placeholder="Enter new password again" name="new_password_check"/></label>
    <button type="submit">Login</button>
</form>
<p><a href="/admin/dashboard">&lt;- Back</a></p>
</body>
</html>
    "#,
        )))
}
