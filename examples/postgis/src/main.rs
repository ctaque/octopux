use actix_web::web;
use apistos::app::{BuildConfig, OpenApiWrapper};
use apistos::spec::Spec;
use apistos::SwaggerUIConfig;
use sqlx::postgres::PgPoolOptions;
#[path = "helpers.rs"]
mod shared;
mod place;
mod place_nearby;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok(); // loads .env if present, never overrides existing vars

    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| std::io::Error::other("DATABASE_URL must be set, e.g. postgres://postgres:postgres@localhost:5432/places"))?;
    let pool = PgPoolOptions::new()
        .connect(&database_url)
        .await
        .map_err(std::io::Error::other)?;
    // creates the postgis extension, the place table and a few places in Paris
    sqlx::migrate!().run(&pool).await.map_err(std::io::Error::other)?;

    // One pool shared by every worker
    let state = web::Data::new(shared::AppState { pool });

    actix_web::HttpServer::new(move || {
        actix_web::App::new()
            .document(Spec::default())
            .service(apistos::web::scope("v1")
                .configure(place::configure)
                .configure(place_nearby::configure)
            )
            .app_data(state.clone())
            // serves the document on /openapi.json and Swagger UI on /swagger
            .build_with("/openapi.json", BuildConfig::default().with(SwaggerUIConfig::new(&"/swagger")))
    })
        .bind(("127.0.0.1", 8085))?
        .run()
        .await
}
