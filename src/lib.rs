pub mod auth;
pub mod books;
pub mod config;
mod error;
pub mod signals;
pub mod simple_broker;
pub mod keyed_broker;
mod telemetry;

use actix_web::{guard, web, App, HttpMessage, HttpRequest, HttpResponse, HttpServer};
use async_graphql::extensions::{Tracing, TracingConfig};
use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql::{MergedObject, MergedSubscription, Schema};
use async_graphql_actix_web::{Request, Response, WSSubscription};
use books::{BookQueries, BookSubscriptions, Storage};
use futures;
use rand::{distributions::Alphanumeric, Rng};
use serde::Deserialize;
use signals::{MutationRoot, SignalQueries, SignalSubscriptions};
use sqlx::postgres::{PgPool, PgPoolOptions};
use tracing_actix_web::TracingLogger;
use tracing_log::LogTracer;

#[derive(MergedSubscription, Default)]
struct SubscriptionRoot(SignalSubscriptions, BookSubscriptions);

#[derive(MergedObject, Default)]
struct QueryRoot(SignalQueries, BookQueries);

type RootSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

async fn index(schema: web::Data<RootSchema>, req: HttpRequest, graphql_req: Request) -> Response {
    let token = req
        .cookie(auth::SessionId::cookie_name())
        .map(|value| auth::AUTH.session_id_from_cookie(value.value()))
        .flatten();

    let mut request = graphql_req.into_inner();
    if let Some(token) = token {
        request = request.data(token);
    }
    schema.execute(request).await.into()
}

#[derive(Deserialize)]
struct AuthToken {
    token: Option<String>,
}

async fn index_ws(
    schema: web::Data<RootSchema>,
    req: HttpRequest,
    payload: web::Payload,
    token_param: web::Query<AuthToken>,
) -> actix_web::Result<HttpResponse> {
    let token = req
        .cookie(auth::SessionId::cookie_name())
        .map(|value| auth::AUTH.session_id_from_cookie(value.value()))
        .flatten()
        .or_else(|| {
            token_param
                .0
                .token
                .map(|t| auth::AUTH.session_id_from_cookie(&t))
                .flatten()
            // auth::SessionId(
            //     rand::thread_rng()
            //         .sample_iter(&Alphanumeric)
            //         .take(24)
            //         .map(char::from)
            //         .collect(),
            // )
        });

    // dbg!(&user_id);

    // req.headers.("Set-Cookie", AUTH.make_cookie(user_id.clone()));

    WSSubscription::start_with_initializer(Schema::clone(&*schema), &req, payload, |value| {
        // let token = value.as_object().and_then(|v| v.get("token")?.as_str());
        let mut data = async_graphql::Data::default();
        dbg!(&value.as_object());
        dbg!(&value);
        // data.insert(token);
        let result = if let Some(token) = token {
            data.insert(token);
            Ok(data)
        } else {
            Ok(data)
        };
        futures::future::ready(result)
    })
}

async fn index_playground() -> actix_web::Result<HttpResponse> {
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(playground_source(
            GraphQLPlaygroundConfig::new("/").subscription_endpoint("/"),
        )))
}

pub struct TestApp {
    pub port: u16,
    pub pool: PgPool,
}

impl TestApp {
    pub fn base_url(&self) -> String {
        format!("http://localhost:{}", self.port)
    }
}

pub async fn spawn_test_app() -> std::io::Result<TestApp> {
    let config = config::get_config();
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

    let listener = std::net::TcpListener::bind("0.0.0.0:0").expect("Falied to bind random port");
    let address = listener.local_addr().unwrap();
    let server = run(listener, pool.clone()).expect("Failed to bind address");
    actix_rt::spawn(async {
        server.await.unwrap();
    });
    Ok(TestApp {
        port: address.port(),
        pool,
    })
}

lazy_static::lazy_static! {
    static ref TRACING: () = {
        // let filter = if std::env::var("TEST_LOG").is_ok() { "debug" } else { "" };
        // let subscriber = get_subscriber("test".into(), filter.into());
        // LogTracer::init().expect("Failed to set logger");
        // a builder for `FmtSubscriber`.
        let subscriber = tracing_subscriber::FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
            .with_max_level(tracing::Level::TRACE)
        // completes the builder.
            .finish();

        tracing::subscriber::set_global_default(subscriber)
            .expect("setting defualt subscriber failed");
    };
}

pub fn run(
    listener: std::net::TcpListener,
    pool: PgPool,
) -> std::io::Result<actix_web::dev::Server> {
    let schema = Schema::build(
        QueryRoot::default(),
        MutationRoot,
        SubscriptionRoot::default(),
    )
    .data(Storage::default())
    .data(pool)
    .extension(Tracing::default())
    .finish();
    lazy_static::initialize(&TRACING);
    tracing::info!("dwdw");

    let server = HttpServer::new(move || {
        let cors = actix_cors::Cors::permissive();
        App::new()
            .wrap(cors)
            .wrap(TracingLogger)
            .data(schema.clone())
            .service(web::resource("/").guard(guard::Post()).to(index))
            .service(
                web::resource("/")
                    .guard(guard::Get())
                    .guard(guard::Header("upgrade", "websocket"))
                    .to(index_ws),
            )
            .service(web::resource("/").guard(guard::Get()).to(index_playground))
    })
    .listen(listener)?
    .run();

    Ok(server)
}
