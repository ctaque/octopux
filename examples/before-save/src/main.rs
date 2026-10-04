#[path = "my_model.rs"]
mod my_model;
#[path = "helpers.rs"]
mod shared;
mod hooks;

use apistos::app::{BuildConfig, OpenApiWrapper};
use apistos::info::Info;
use apistos::spec::Spec;
use apistos::SwaggerUIConfig;
use sqlx::postgres::PgPoolOptions;
use std::default::Default;
use actix_web;
use actix_web::{middleware::Logger};


// The OpenAPI document is served on /openapi.json, and browsable on /swagger
#[actix_web::main]
async fn main() -> anyhow::Result<()>{
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    // The PostgreSQL database is migrated on launch
    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL must be set, e.g. postgres://user:password@localhost:5432/db or sqlite://data.db?mode=rwc"))?;
    let pool = PgPoolOptions::new().connect(&database_url).await?;
    sqlx::migrate!().run(&pool).await?;

    // One pool shared by every worker
    let state = actix_web::web::Data::new(shared::AppState { pool });


    actix_web::HttpServer::new(move || {
        let spec = Spec {
            info: Info {
                title: "MyApp API".to_string(),
                version: "1.0.0".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        actix_web::App::new()
            .wrap(Logger::default())
            .document(spec)
            // Actix does not fall through between scopes sharing a prefix,
            // so resources living under the same scope must be registered together
            .service(
                apistos::web::scope("v1")
                    .configure(my_model::configure)
            )
            .app_data(state.clone())
            .build_with(
                "/openapi.json",
                BuildConfig::default().with(SwaggerUIConfig::new(&"/swagger")),
            )
    })
        .bind(("127.0.0.1", 8085))?
        .run()
        .await?;
    Ok(())
}
