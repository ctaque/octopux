//                         -----
//                     -------------
//                   -----  ----------
//                  ---  --------------
//                 ---  ----------------
//                 --- -----------------
//                 --- -----------------
//                 --- -----------------
//                 ---------------------
//       -----      -------------------       -----
//      -------      --  ---------- --      -------
//          ----      ---------------      -----
//           ---      ---------------      ----
//          ----     -----------------     ----
//        ------   ----------------------   ------
//    --------  ---------------------------  --------
//   ------   -------------------------- ----   -------
//  ----    -----  ---------- --- ------- -----    -----
// ----  ------  -------- --- --- ---- ---  ------  ----
// ----        ---- ----  --- ---- ---- -----       ----
//  ----   ------  ----  ---- ----  ----   ------  -----
//  ------      ------   ---- -----  ------      ------
//    ---------------    ----  ----    --------------
//      ----------       ----  ----      ----------
//                       ----  ----
//                 ---   ---- -----   --
//               ------  ---- ----- -------
//              -------  ---- ----- --------
//              ----    ----   -----    ----
//              -----------     -----------
//               ---------        --------

// The application state, declared (or re-exported) at the root of the crate
use crate::shared::AppState;
use serde::{Serialize, Deserialize};
use octopux::{
    HttpCreate,
    HttpFindListDelete,
    HttpUpdate,
    SqlxModel,
    SqlxFilter,
    SqlxNewModel,
    SqlxUpdatableModel,
    Model,
    NewModel,
    UpdatableModel,
    octopux_info,
    gen_documented_endpoint
};
// `Point` and `Polygon` are read from and written to PostGIS as EWKB, and sent to the client as
// GeoJSON geometries: `{"type": "Point", "coordinates": [2.2945, 48.8584]}`, in the REST routes
// as in GraphQL, where they are the `GeoJsonPoint` and `GeoJsonPolygon` scalars
use octopux::postgis::{Bbox, Near, Point, Polygon};
use async_graphql::{Context, InputObject, Object, SimpleObject};
use chrono::{DateTime, Utc};
use apistos::ApiComponent;
use schemars::JsonSchema;
use octopux::postgis;

#[derive(Default, Deserialize, JsonSchema, ApiComponent)]
pub struct FindQuery {}
// The spatial filters of the list, pushed as PostGIS conditions by `SqlxFilter`
#[derive(Deserialize, JsonSchema, ApiComponent, SqlxFilter)]
#[sqlx_filter(database = "postgres")]
pub struct ListQuery {
    /// Number of rows to skip
    pub offset: Option<usize>,
    /// Maximum number of rows to return (20 by default, 100 at most)
    pub limit: Option<usize>,
    /// The places whose location is in the box `west,south,east,north`: `ST_Intersects(location, ST_MakeEnvelope(...))`
    #[sqlx_filter(column = "location", op = "intersects")]
    pub bbox: Option<Bbox>,
    /// The places within a distance of a point, `longitude,latitude,meters`: `ST_DWithin(location::geography, ...)`
    #[sqlx_filter(column = "location", op = "dwithin")]
    pub near: Option<Near>,
    /// The places whose area contains this point, as GeoJSON: `{"type":"Point","coordinates":[2.2945,48.8584]}`,
    /// `ST_Contains(area, ...)`
    #[sqlx_filter(column = "area", op = "contains")]
    // a JSON string in the query string, not an object exploded into `type=...&coordinates=...`
    #[schemars(with = "Option<String>")]
    pub point: Option<Point>,
}
#[derive(Deserialize, JsonSchema, ApiComponent)]
pub struct DeleteQuery {}
#[derive(Deserialize, JsonSchema, ApiComponent)]
pub struct SaveQuery {}
#[derive(Deserialize, JsonSchema, ApiComponent)]
pub struct UpdateQuery {}
pub type Id = i64;

#[derive(Default, Serialize, Deserialize, JsonSchema, ApiComponent, SimpleObject, sqlx::FromRow, HttpFindListDelete, SqlxModel)]
#[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
#[sqlx_model(database = "postgres", timestamps, soft_delete, filter)]
#[octopux_info(path = "place")]
pub struct Place {
    pub id: Id,
    pub name: String,
    /// `geometry(Point, 4326)`
    pub location: Point,
    /// `geometry(Polygon, 4326)`, the outline of the place when it has one
    pub area: Option<Polygon>,
    pub area_2: Option<postgis::Polygon>,
    pub point2: Option<postgis::Point>,
    pub multipoint: Option<postgis::MultiPoint>,
    pub multipolygon: Option<postgis::MultiPolygon>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
#[http_create(SaveQuery, AppState)]
#[sqlx_model(database = "postgres", model = "Place", timestamps)]
pub struct NewPlace {
    pub name: String,
    pub location: Point,
    pub area: Option<Polygon>,
    pub area_2: Option<postgis::Polygon>,
    pub point2: Option<postgis::Point>,
    pub multipoint: Option<postgis::MultiPoint>,
    pub multipolygon: Option<postgis::MultiPolygon>,
}

#[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
#[http_update(Id, UpdateQuery, Place, FindQuery, AppState)]
#[sqlx_model(database = "postgres", timestamps, soft_delete)]
pub struct UpdatablePlace {
    pub id: Id,
    pub name: String,
    pub location: Point,
    pub area: Option<Polygon>,
    pub area_2: Option<postgis::Polygon>,
    pub point2: Option<postgis::Point>,
    pub multipoint: Option<postgis::MultiPoint>,
    pub multipolygon: Option<postgis::MultiPolygon>,
    pub updated_at: Option<DateTime<Utc>>,
}

// Registers the documented routes of the place endpoint
// (octopux `openapi` feature), to mount with `.configure(place::configure)`
pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
    gen_documented_endpoint!(Place, NewPlace, UpdatablePlace)(cfg)
}

/// Distance around a point, in meters, when the `places` query gives no radius
const DEFAULT_RADIUS: f64 = 1000.0;

// GraphQL queries of the place model (async-graphql), the geometries being GeoJSON scalars
#[derive(Default)]
pub struct PlaceQuery;

#[Object]
impl PlaceQuery {
    async fn place(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Place> {
        find(id, app_state(ctx)?).await
    }

    /// A page of places, filtered as `GET /v1/place`: `contains` keeps the places whose area contains
    /// the point, `around` the places within `radius` meters of the point (1000 by default)
    async fn places(
        &self,
        ctx: &Context<'_>,
        offset: Option<usize>,
        limit: Option<usize>,
        contains: Option<Point>,
        around: Option<Point>,
        radius: Option<f64>,
    ) -> async_graphql::Result<Vec<Place>> {
        let near = match around {
            Some(point) => Some(Near::new(point.x(), point.y(), radius.unwrap_or(DEFAULT_RADIUS)).map_err(async_graphql::Error::new)?),
            None => None,
        };
        let query = ListQuery { offset, limit, bbox: None, near, point: contains };
        Ok(Place::list(&query, app_state(ctx)?).await?)
    }
}

// GraphQL mutations of the place model, the geometries of the inputs being GeoJSON
#[derive(Default)]
pub struct PlaceMutation;

#[Object]
impl PlaceMutation {
    async fn create_place(&self, ctx: &Context<'_>, input: NewPlace) -> async_graphql::Result<Place> {
        Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
    }

    async fn update_place(&self, ctx: &Context<'_>, input: UpdatablePlace) -> async_graphql::Result<Place> {
        let state = app_state(ctx)?;
        let id = input.id;
        find(id, state).await?;
        input.update(&UpdateQuery {}, state).await?;
        find(id, state).await
    }

    async fn delete_place(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Place> {
        let state = app_state(ctx)?;
        let model = find(id, state).await?;
        Ok(model.delete(&DeleteQuery {}, state).await?)
    }
}

// Looks the place up, any error being ENTITY_NOT_FOUND as for the REST routes
async fn find(id: Id, state: &AppState) -> async_graphql::Result<Place> {
    match Place::find(id, &FindQuery::default(), state).await {
        Ok(model) => Ok(*model),
        Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
    }
}

// The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
    Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
}
