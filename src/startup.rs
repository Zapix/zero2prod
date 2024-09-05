use std::net::TcpListener;
use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use sqlx::{PgPool};
use crate::routes::{health_check, subscribe};

pub fn run(tcp_listener: TcpListener, connection_pool: PgPool) -> Result<Server, std::io::Error> {
    let connection_pool = web::Data::new(connection_pool);

    let address = tcp_listener.local_addr().expect("Can't get address");
    let server = HttpServer::new(
        move || App::new()
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
            .app_data(connection_pool.clone())
    )
        .listen(tcp_listener)?
        .run();
    println!("Listening {}", address);
    Ok(server)
}