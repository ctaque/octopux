//! End-to-end tests of the PostGIS geometries and the spatial filters, through the routes of models
//! deriving the sqlx derives, on a live PostgreSQL database with PostGIS.
//!
//! They are ignored by `cargo test`, and never run in the CI, even with `--ignored`. To run them:
//!
//! ```bash
//! docker run -d --rm -e POSTGRES_PASSWORD=postgres -p 5432:5432 postgis/postgis:17-3.5
//! DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres \
//!     cargo test -p octopux --features postgis --test postgis -- --ignored
//! ```

use actix_web::{test, web, App};
use octopux::postgis::{self, geo_types, Bbox, Near, Point, Polygon};
use octopux::{
    gen_endpoint, octopux_info, HttpCreate, HttpFindListDelete, HttpUpdate, SqlxFilter, SqlxModel, SqlxNewModel, SqlxUpdatableModel,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::postgres::PgPool;

const IGNORED: &str = "needs a PostgreSQL database with PostGIS in DATABASE_URL";

struct AppState {
    pool: PgPool,
}

type Id = i64;

#[derive(Default, Deserialize)]
struct FindQuery {}
#[derive(Deserialize)]
struct DeleteQuery {}
#[derive(Deserialize)]
struct SaveQuery {}
#[derive(Deserialize)]
struct UpdateQuery {}

#[derive(Deserialize, SqlxFilter)]
#[sqlx_filter(database = "postgres")]
struct ListQuery {
    offset: Option<usize>,
    limit: Option<usize>,
    name: Option<String>,
    #[sqlx_filter(column = "location", op = "intersects")]
    bbox: Option<Bbox>,
    #[sqlx_filter(column = "location", op = "dwithin")]
    near: Option<Near>,
    #[sqlx_filter(column = "area", op = "dwithin")]
    area_near: Option<Near>,
    #[sqlx_filter(column = "area", op = "intersects")]
    area_bbox: Option<Bbox>,
    #[sqlx_filter(column = "zone", op = "contains")]
    point: Option<Point>,
    #[sqlx_filter(column = "location", op = "within")]
    inside: Option<postgis::Geometry>,
}

// `location` a geometry point, `area` a geography of any kind, `zone` a geometry polygon
#[derive(Debug, Default, Serialize, Deserialize, sqlx::FromRow, HttpFindListDelete, SqlxModel)]
#[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
#[sqlx_model(database = "postgres", table = "octopux_spot", filter)]
#[octopux_info(path = "spot")]
struct Spot {
    id: Id,
    name: String,
    location: Point,
    area: Option<postgis::Geometry>,
    zone: Option<Polygon>,
}

#[derive(Serialize, Deserialize, HttpCreate, SqlxNewModel)]
#[http_create(SaveQuery, AppState)]
#[sqlx_model(database = "postgres", model = "Spot", table = "octopux_spot")]
struct NewSpot {
    name: String,
    location: Point,
    area: Option<postgis::Geometry>,
    zone: Option<Polygon>,
}

#[derive(Serialize, Deserialize, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
#[http_update(Id, UpdateQuery, Spot, FindQuery, AppState)]
#[sqlx_model(database = "postgres", table = "octopux_spot")]
struct UpdatableSpot {
    id: Id,
    location: Point,
}

// The pool of DATABASE_URL, None in the CI, where the PostGIS tests are forbidden
async fn pool() -> Option<PgPool> {
    if std::env::var_os("CI").is_some() {
        eprintln!("skipped: the PostGIS tests do not run in the CI");
        return None;
    }
    let url = std::env::var("DATABASE_URL").expect(IGNORED);
    Some(PgPool::connect(&url).await.expect("DATABASE_URL accepts connections"))
}

// The spot table, created again, the tests running one after the other on it
async fn reset(pool: &PgPool) {
    sqlx::raw_sql(
        "CREATE EXTENSION IF NOT EXISTS postgis;
         DROP TABLE IF EXISTS octopux_spot;
         CREATE TABLE octopux_spot (
             id BIGSERIAL PRIMARY KEY,
             name TEXT NOT NULL,
             location geometry(Point, 4326) NOT NULL,
             area geography,
             zone geometry(Polygon, 4326)
         )",
    )
    .execute(pool)
    .await
    .unwrap();
}

// The two tests share the table, they are run as one
#[actix_web::test]
#[ignore = "needs a PostgreSQL database with PostGIS in DATABASE_URL"]
async fn postgis() {
    let Some(pool) = pool().await else { return };
    reset(&pool).await;
    geometries_are_stored_and_read_back(&pool).await;
    reset(&pool).await;
    spatial_filters_select_the_rows(&pool).await;
}

async fn geometries_are_stored_and_read_back(pool: &PgPool) {
    let state = AppState { pool: pool.clone() };
    use octopux::{Model, NewModel, UpdatableModel};

    let new: NewSpot = serde_json::from_value(json!({
        "name": "Paris",
        "location": { "type": "Point", "coordinates": [2.3522, 48.8566] },
        "area": { "type": "MultiPolygon", "coordinates": [[[[2.2, 48.8], [2.4, 48.8], [2.4, 48.9], [2.2, 48.8]]]] },
        "zone": { "type": "Polygon", "coordinates": [[[0, 0], [1, 0], [1, 1], [0, 0]]] },
    }))
    .unwrap();
    let saved = new.save(&SaveQuery {}, &state).await.unwrap();
    assert_eq!(saved.location.srid, postgis::WGS84);
    assert_eq!(saved.area.as_ref().unwrap().srid, postgis::WGS84);

    // PostGIS reads what was written
    let (location, area): (String, String) = sqlx::query_as("SELECT ST_AsEWKT(location), ST_AsEWKT(area) FROM octopux_spot WHERE id = $1")
        .bind(saved.id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(location, "SRID=4326;POINT(2.3522 48.8566)");
    assert_eq!(area, "SRID=4326;MULTIPOLYGON(((2.2 48.8,2.4 48.8,2.4 48.9,2.2 48.8)))");

    let found = Spot::find(saved.id, &FindQuery {}, &state).await.unwrap();
    assert_eq!(found.location.geometry, geo_types::point!(x: 2.3522, y: 48.8566));
    let updated = UpdatableSpot { id: saved.id, location: geo_types::point!(x: -1.55, y: 47.21).into() }
        .update(&UpdateQuery {}, &state)
        .await
        .unwrap();
    assert_eq!(updated.location.x(), -1.55);

    // a SRID other than the one of the column is refused by PostGIS
    let other_srid = NewSpot { name: "x".into(), location: Point::new(geo_types::point!(x: 0.0, y: 0.0), 3857), area: None, zone: None };
    let err = other_srid.save(&SaveQuery {}, &state).await.unwrap_err();
    assert!(err.to_string().contains("SRID (3857) does not match column SRID (4326)"), "{}", err);

    // a geometry of another kind than the field fails to decode
    let err = sqlx::query_scalar::<_, Point>("SELECT 'SRID=4326;LINESTRING(0 0, 1 1)'::geometry").fetch_one(pool).await.unwrap_err();
    assert!(err.to_string().contains("expected a Point, found a LineString"), "{}", err);

    // every kind through the database and back, the text format and the arrays
    let collection: postgis::Geometry = geo_types::Geometry::GeometryCollection(geo_types::GeometryCollection(vec![
        geo_types::point!(x: 1.0, y: 2.0).into(),
        geo_types::Polygon::new(vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)].into(), vec![]).into(),
        geo_types::MultiPoint(vec![geo_types::point!(x: 3.0, y: 4.0)]).into(),
    ]))
    .into();
    let back: postgis::Geometry = sqlx::query_scalar("SELECT $1::geometry").bind(&collection).fetch_one(pool).await.unwrap();
    assert_eq!(back, collection);
    let geography: postgis::Geometry = sqlx::query_scalar("SELECT 'POINT(1 2)'::geography").fetch_one(pool).await.unwrap();
    assert_eq!(geography.srid, postgis::WGS84);
    let points: Vec<Point> = sqlx::query_scalar("SELECT ARRAY['SRID=4326;POINT(1 2)'::geometry, 'POINT(3 4)'::geometry]")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!((points.len(), points[1].srid), (2, 0));
    use sqlx::Row;
    let rows = sqlx::raw_sql("SELECT location FROM octopux_spot").fetch_all(pool).await.unwrap();
    let text_format: Point = rows[0].try_get(0).unwrap();
    assert_eq!(text_format.geometry, geo_types::point!(x: -1.55, y: 47.21));
}

async fn spatial_filters_select_the_rows(pool: &PgPool) {
    let app = test::init_service(App::new().app_data(web::Data::new(AppState { pool: pool.clone() })).configure(gen_endpoint!(Spot, NewSpot, UpdatableSpot))).await;
    let spots = [
        ("tour-eiffel", [2.2945, 48.8584]),
        ("louvre", [2.3376, 48.8606]),
        ("notre-dame", [2.3499, 48.8530]),
        ("versailles", [2.1204, 48.8049]),
        ("nantes", [-1.5536, 47.2184]),
    ];
    for (name, [x, y]) in spots {
        let zone = [[x - 0.01, y - 0.01], [x + 0.01, y - 0.01], [x + 0.01, y + 0.01], [x - 0.01, y + 0.01], [x - 0.01, y - 0.01]];
        let body = json!({
            "name": name,
            "location": { "type": "Point", "coordinates": [x, y] },
            "area": { "type": "Point", "coordinates": [x, y] },
            "zone": { "type": "Polygon", "coordinates": [zone] },
        });
        let res = test::call_service(&app, test::TestRequest::post().uri("/spot").set_json(body).to_request()).await;
        assert!(res.status().is_success(), "{}", res.status());
    }
    let list = |query: &str| {
        let req = test::TestRequest::get().uri(&format!("/spot?{}", query)).to_request();
        let app = &app;
        async move {
            let res = test::call_service(app, req).await;
            let status = res.status().as_u16();
            let body = test::read_body(res).await;
            match serde_json::from_slice::<Value>(&body) {
                Ok(Value::Array(spots)) => Ok(spots.iter().map(|s| s["name"].as_str().unwrap().to_string()).collect::<Vec<_>>()),
                _ => Err((status, String::from_utf8_lossy(&body).to_string())),
            }
        }
    };
    let encoded = |geojson: &str| geojson.bytes().map(|b| format!("%{:02X}", b)).collect::<String>();

    assert_eq!(list("bbox=2.25,48.84,2.36,48.87").await.unwrap(), ["tour-eiffel", "louvre", "notre-dame"]);
    assert_eq!(list("near=2.3376,48.8606,2000").await.unwrap(), ["louvre", "notre-dame"]);
    assert_eq!(list("near=2.3376,48.8606,4000&name=tour-eiffel").await.unwrap(), ["tour-eiffel"]);
    assert_eq!(list("near=2.3376,48.8606,4000&limit=1").await.unwrap(), ["tour-eiffel"]);
    // on the geography column
    assert_eq!(list("area_near=-1.5,47.2,10000").await.unwrap(), ["nantes"]);
    assert_eq!(list("area_bbox=-2,47,-1,48").await.unwrap(), ["nantes"]);
    let point = encoded(r#"{"type":"Point","coordinates":[2.295,48.859]}"#);
    assert_eq!(list(&format!("point={}", point)).await.unwrap(), ["tour-eiffel"]);
    let polygon = encoded(r#"{"type":"Polygon","coordinates":[[[2,48],[3,48],[3,49],[2,49],[2,48]]]}"#);
    assert_eq!(list(&format!("inside={}", polygon)).await.unwrap(), ["tour-eiffel", "louvre", "notre-dame", "versailles"]);
    // the invalid filters are client errors
    let (status, body) = list("bbox=3,2,1").await.unwrap_err();
    assert_eq!(status, 400);
    assert!(body.contains("expected a bounding box, west,south,east,north"), "{}", body);
    assert_eq!(list("point=nope").await.unwrap_err().0, 400);
}
