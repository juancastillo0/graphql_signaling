use graphql_test::config::get_config;
use graphql_test::run;
use sqlx::postgres::PgPoolOptions;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    let config = get_config();

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database.url())
        .await
        .unwrap();

    // Make a simple query to return the given parameter
    let row: (i64,) = sqlx::query_as("SELECT $1")
        .bind(150_i64)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(row.0, 150_i64);

    let listener = std::net::TcpListener::bind(format!("0.0.0.0:{}", config.application_port))
        .expect("Failed to bind random port");

    println!(
        "Playground: http://localhost:{}/",
        listener.local_addr().unwrap().port()
    );
    run(listener, pool)?.await
}
