use actix_web::http::header::ContentType;
use actix_web::HttpResponse;
use actix_web_flash_messages::IncomingFlashMessages;
use std::fmt::Write;

pub async fn newsletter_form(
    flash_messages: IncomingFlashMessages,
) -> Result<HttpResponse, actix_web::Error> {
    let mut html_message = String::new();
    let idempotency_key = uuid::Uuid::new_v4();
    for message in flash_messages.iter() {
        let _ = write!(html_message, "<p><i>{}</i></p>", message.content());
    }
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"
        <!DOCTYPE html>
        <html lang="en">
<head>
    <meta http-equiv="content-type" content="text/html" charset="UTF-8">
    <title>Write Newsletter</title>
</head>
<body>
{html_message}
<form method="post" action="/admin/newsletters">
    <div>
        <label>Title: <input type="text" placeholder="Enter title" name="title" /></label>
    </div>
    <div>
        <label>Text Content</label>
        <textarea name="content_text"></textarea>
    </div>
    <div>
        <label>Html Context</label>
        <textarea name="content_html"></textarea>
    </div>
    <input hidden type="text" name="idempotency_key" value="{idempotency_key}"
    <button type="submit">Send</button>
</form>
<p><a href="/admin/dashboard">&lt;- Back</a></p>
</body>
</html>
    "#,
        )))
}
