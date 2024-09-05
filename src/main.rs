use std::net::TcpListener;
use sqlx::{PgPool};

use zero2prod::startup::run;
use zero2prod::configuration::get_configuration;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let configuration = get_configuration().expect("Failed to read configuration.");
    let connection_st = configuration.database.connection_string();
    let connection_pool = PgPool::connect(&connection_st)
        .await
        .expect("Failed to connect to Postgres.");
    let address = format!("127.0.0.1:{}", configuration.application_port);
    let listener = TcpListener::bind(address)
        .expect("Failed to bind random port");
    run(listener, connection_pool)?.await
}
