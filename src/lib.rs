use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_web::dev::Server;
use std::net::TcpListener;

async fn greet(req: HttpRequest) -> impl Responder {
    let name = req.match_info().get("name").unwrap_or("World");
    format!("Hello {}!", &name)
}

async fn health_check() -> impl Responder {
    HttpResponse::Ok()
}

pub fn run(tcp_listener: TcpListener) -> Result<Server, std::io::Error> {
    let server = HttpServer::new(
        || App::new()
            .route("/", web::get().to(greet))
            .route("/health_check", web::get().to(health_check))
            .route("/{name}", web::get().to(greet))
    )
        .listen(tcp_listener)?
        .run();

    Ok(server)
}