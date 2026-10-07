// The application state, declared (or re-exported) at the root of the crate
use crate::shared::AppState;
use crate::place::{Place, Id};
use serde::Deserialize;
use octopux::{
    HasMany,
    anyhow::Result,
    async_trait,
};
use apistos::ApiComponent;
use schemars::JsonSchema;
use octopux::gen_documented_relation_endpoint;

#[derive(Deserialize, JsonSchema, ApiComponent)]
pub struct PlaceNearbyQuery {
    /// Distance from the place, in meters (1000 by default)
    pub radius: Option<f64>,
    /// Number of rows to skip
    pub offset: Option<usize>,
    /// Maximum number of rows to return (20 by default, 100 at most)
    pub limit: Option<usize>,
}
/// Radius, in meters, when none is given
const DEFAULT_RADIUS: f64 = 1000.0;
/// Number of places returned when no limit is given
const DEFAULT_LIMIT: i64 = 20;
/// Maximum number of places returned
const MAX_LIMIT: i64 = 100;

/// The other places within `radius` meters of a place, the closest first,
/// served on `GET /place/{id}/nearby`
pub struct PlaceNearby;

#[async_trait]
impl HasMany for PlaceNearby {
    type Parent = Place;
    type Id = Id;
    type Query = PlaceNearbyQuery;
    type Result = Vec<Place>;
    type State = AppState;
    const RELATION: &'static str = "nearby";

    async fn list_related(id: Id, query: &PlaceNearbyQuery, state: &AppState) -> Result<Option<Vec<Place>>> {
        let radius = query.radius.unwrap_or(DEFAULT_RADIUS);
        if !(radius >= 0.0) {
            return Err(octopux::Error::BadRequest("the radius must be a positive number of meters".into()).into());
        }
        let offset = query.offset.unwrap_or(0) as i64;
        let limit = query.limit.map_or(DEFAULT_LIMIT, |l| (l as i64).min(MAX_LIMIT));
        // The point of the place, `None` when it does not exist
        let Some(origin) = sqlx::query_scalar::<_, octopux::postgis::Point>(
            "SELECT location FROM place WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(&state.pool)
        .await? else {
            return Ok(None);
        };
        // The cast to `geography` measures the distances in meters on the ellipsoid, instead of
        // degrees; the `Point` is bound as a geometry parameter
        let places = sqlx::query_as::<_, Place>(
            "SELECT * FROM place
             WHERE id <> $1 AND deleted_at IS NULL
               AND ST_DWithin(location::geography, $2::geography, $3)
             ORDER BY location::geography <-> $2::geography
             LIMIT $4 OFFSET $5",
        )
        .bind(id)
        .bind(&origin)
        .bind(radius)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await?;
        Ok(Some(places))
    }
}

// Registers the route of the places near a place, to mount with `.configure(place_nearby::configure)`
// in the same scope as the place routes
pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
    gen_documented_relation_endpoint!(PlaceNearby)(cfg)
}
