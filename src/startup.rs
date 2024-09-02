use std::net::TcpListener;
use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use crate::routes::{health_check, subscribe};

pub fn run(tcp_listener: TcpListener) -> Result<Server, std::io::Error> {
    let address = tcp_listener.local_addr().expect("Can't get address");
    let server = HttpServer::new(
        || App::new()
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
    )
        .listen(tcp_listener)?
        .run();
    println!("Listening {}", address.to_string());
    Ok(server)
}