// The application state, declared (or re-exported) at the root of the crate
use crate::shared::AppState;
use serde::{Serialize, Deserialize};
use octopux::{
    HttpCreate,
    HttpFindListDelete,
    HttpUpdate,
    SqlxModel,
    SqlxNewModel,
    SqlxUpdatableModel,
    octopux_info,
    gen_documented_endpoint
};
// `Point` and `Polygon` are read from and written to PostGIS as EWKB, and sent to the client as
// GeoJSON geometries: `{"type": "Point", "coordinates": [2.2945, 48.8584]}`
use octopux::postgis::{Point, Polygon};
use chrono::{DateTime, Utc};
use apistos::ApiComponent;
use schemars::JsonSchema;

#[derive(Default, Deserialize, JsonSchema, ApiComponent)]
pub struct FindQuery {}
#[derive(Deserialize, JsonSchema, ApiComponent)]
pub struct ListQuery {
    /// Number of rows to skip
    pub offset: Option<usize>,
    /// Maximum number of rows to return (20 by default, 100 at most)
    pub limit: Option<usize>,
}
#[derive(Deserialize, JsonSchema, ApiComponent)]
pub struct DeleteQuery {}
#[derive(Deserialize, JsonSchema, ApiComponent)]
pub struct SaveQuery {}
#[derive(Deserialize, JsonSchema, ApiComponent)]
pub struct UpdateQuery {}
pub type Id = i64;

#[derive(Default, Serialize, Deserialize, JsonSchema, ApiComponent, sqlx::FromRow, HttpFindListDelete, SqlxModel)]
#[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
#[sqlx_model(database = "postgres", timestamps, soft_delete)]
#[octopux_info(path = "place")]
pub struct Place {
    pub id: Id,
    pub name: String,
    /// `geometry(Point, 4326)`
    pub location: Point,
    /// `geometry(Polygon, 4326)`, the outline of the place when it has one
    pub area: Option<Polygon>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, JsonSchema, ApiComponent, HttpCreate, SqlxNewModel)]
#[http_create(SaveQuery, AppState)]
#[sqlx_model(database = "postgres", model = "Place", timestamps)]
pub struct NewPlace {
    pub name: String,
    pub location: Point,
    pub area: Option<Polygon>,
}

#[derive(Serialize, Deserialize, JsonSchema, ApiComponent, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
#[http_update(Id, UpdateQuery, Place, FindQuery, AppState)]
#[sqlx_model(database = "postgres", timestamps, soft_delete)]
pub struct UpdatablePlace {
    pub id: Id,
    pub name: String,
    pub location: Point,
    pub area: Option<Polygon>,
    pub updated_at: Option<DateTime<Utc>>,
}

// Registers the documented routes of the place endpoint
// (octopux `openapi` feature), to mount with `.configure(place::configure)`
pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
    gen_documented_endpoint!(Place, NewPlace, UpdatablePlace)(cfg)
}
