use actix_web::web;
use apistos::app::{BuildConfig, OpenApiWrapper};
use apistos::spec::Spec;
use apistos::SwaggerUIConfig;
use async_graphql::http::GraphiQLSource;
use async_graphql::{EmptySubscription, Schema};
use async_graphql_actix_web::GraphQL;
use sqlx::postgres::PgPoolOptions;
#[path = "helpers.rs"]
mod shared;
mod place;
mod place_nearby;

// The GraphQL schema of the places, the geometries being GeoJSON scalars (octopux `graphql` feature)
type PlaceSchema = Schema<place::PlaceQuery, place::PlaceMutation, EmptySubscription>;

async fn graphiql() -> actix_web::HttpResponse {
    actix_web::HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(GraphiQLSource::build().endpoint("/graphql").finish())
}

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
    // The resolvers read the state from the data of the schema
    let schema: PlaceSchema = Schema::build(place::PlaceQuery, place::PlaceMutation, EmptySubscription)
        .data(state.clone())
        .finish();

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
            // GraphQL queries on POST /graphql, GraphiQL on GET /graphql
            .service(
                web::resource("/graphql")
                    .route(web::post().to(GraphQL::new(schema.clone())))
                    .route(web::get().to(graphiql)),
            )
    })
        .bind(("127.0.0.1", 8085))?
        .run()
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    // The geometries are GeoJSON scalars of the schema, checked without database
    #[test]
    fn the_schema_types_the_geometries_as_geojson_scalars() {
        let sdl = Schema::build(place::PlaceQuery, place::PlaceMutation, EmptySubscription).finish().sdl();
        for expected in [
            "scalar GeoJsonPoint",
            "scalar GeoJsonPolygon",
            "location: GeoJsonPoint!",
            "area: GeoJsonPolygon",
            "places(offset: Int, limit: Int, contains: GeoJsonPoint, around: GeoJsonPoint, radius: Float): [Place!]!",
            "createPlace(input: NewPlace!): Place!",
        ] {
            assert!(sdl.contains(expected), "`{}` missing from\n{}", expected, sdl);
        }
    }
}
