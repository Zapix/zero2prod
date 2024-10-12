use actix_web::cookie::Cookie;
use actix_web::http::header::ContentType;
use actix_web::HttpResponse;
use actix_web_flash_messages::{IncomingFlashMessages, Level};
use std::fmt::Write;

pub async fn login_form(flash_messages: IncomingFlashMessages) -> HttpResponse {
    let mut error_html = String::new();
    for m in flash_messages.iter() {
        write!(error_html, "<p><i>{}</i></p>", m.content()).unwrap();
    }
    let mut response = HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"
            <!DOCTYPE html>
            <html lang="en">
<head>
    <meta http-equiv="content-type" content="text/html" charset="UTF-8">
    <title>Login</title>
</head>
<body>
{error_html}
<form method="post" action="/login">
    <label>Username: <input type="text" placeholder="Enter Username" name="username"/></label>
    <label>Password: <input type="password" placeholder="Enter Password" name="password"/></label>

    <button type="submit">Login</button>
</form>
</body>
</html>
            "#
        ));

    response
        .add_removal_cookie(&Cookie::new("_flash", ""))
        .unwrap();

    response
}
