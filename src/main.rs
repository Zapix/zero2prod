use std::net::TcpListener;
use secrecy::ExposeSecret;
use sqlx::PgPool;
use zero2prod::startup::run;
use zero2prod::configuration::get_configuration;
use zero2prod::telemetry::{get_subscriber, init_subscriber};

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);
    let configuration = get_configuration().expect("Failed to read configuration.");
    let connection_st = configuration.database.connection_string();
    let connection_pool = PgPool::connect_lazy(&connection_st.expose_secret())
        .expect("Failed to connect to Postgres.");
    let address = configuration.application.address();
    let listener = TcpListener::bind(address)
        .expect("Failed to bind random port");
    run(listener, connection_pool)?.await
}
