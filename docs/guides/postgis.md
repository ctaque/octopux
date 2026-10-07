---
title: PostGIS
parent: Guides
nav_order: 7
---

# PostGIS

Enable the feature, with `postgres` in the features of sqlx:

```bash
cargo add octopux --features sqlx,postgis          # add graphql for the GraphQL scalars
```

## Geometry fields

With the `postgis` feature, the `geometry` and `geography` columns of PostgreSQL are fields of the sqlx models. They are read and written as EWKB, with their SRID, and sent to and received from the client as GeoJSON geometries:

```rust
use octopux::postgis::{self, Point, Polygon};

#[derive(Serialize, Deserialize, sqlx::FromRow, HttpFindListDelete, SqlxModel)]
#[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
#[sqlx_model(database = "postgres", filter)]
#[octopux_info(path = "place")]
pub struct Place {
    pub id: Id,
    pub name: String,
    pub location: Point,                  // geometry(Point, 4326) NOT NULL
    pub area: Option<Polygon>,            // geometry(Polygon, 4326)
    pub zone: Option<postgis::Geometry>,  // any kind of geometry
}
```

```json
{ "id": 1, "name": "Tour Eiffel", "location": { "type": "Point", "coordinates": [2.2945, 48.8584] }, "area": null, "zone": null }
```

`Point`, `LineString`, `Polygon`, `MultiPoint`, `MultiLineString`, `MultiPolygon` and `GeometryCollection` restrict a field to one kind of geometry, `Geometry` accepts any. They hold a [`geo-types`](https://docs.rs/geo-types) geometry (`Deref`, `into_inner`). A payload of another kind, or with a Z or M coordinate, answers 400 `BAD_REQUEST`, see [Errors]({{ '/guides/errors/' | relative_url }}).

{: .warning }
GeoJSON coordinates are WGS 84 `[longitude, latitude]`: a deserialized geometry gets the SRID 4326 (`postgis::WGS84`). Build the geometry with `Geometry::new(geometry, srid)` for a column of another SRID.

## Spatial filters

[`SqlxFilter`]({{ '/guides/filtering-and-sorting/' | relative_url }}) filters a list with the spatial operators of PostGIS, given by `op`:

```rust
#[derive(Deserialize, SqlxFilter)]
#[sqlx_filter(database = "postgres")]
pub struct ListQuery {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    #[sqlx_filter(column = "location", op = "intersects")]
    pub bbox: Option<postgis::Bbox>,      // ?bbox=2.28,48.85,2.30,48.87
    #[sqlx_filter(column = "location", op = "dwithin")]
    pub near: Option<postgis::Near>,      // ?near=2.2945,48.8584,800
    #[sqlx_filter(column = "area", op = "contains")]
    pub point: Option<postgis::Point>,    // ?point={"type":"Point","coordinates":[2.2945,48.8584]}
}
```

| `op` | Condition | Value |
| --- | --- | --- |
| `intersects` | `ST_Intersects(column, value)` | A `Bbox` (`west,south,east,north`) or a GeoJSON geometry |
| `within` | `ST_Within(column, value)`, the column inside the value | Same, `geometry` columns only |
| `contains` | `ST_Contains(column, value)` | Same, `geometry` columns only |
| `dwithin` | `ST_DWithin(column::geography, point::geography, meters)` | A `Near` (`longitude,latitude,meters`) |

`Bbox` and `Near` are in WGS 84, the column too. Index the columns with GiST, and `(column::geography)` for `dwithin` on a `geometry` column:

```sql
CREATE INDEX place_location_idx ON place USING GIST (location);
CREATE INDEX place_location_geography_idx ON place USING GIST ((location::geography));
```

## GraphQL and OpenAPI

With the `graphql` feature too, the geometries are async-graphql scalars, the same GeoJSON: `GeoJsonPoint`, `GeoJsonPolygon`..., `GeoJsonGeometry` for any kind. With `openapi`, they are documented as GeoJSON geometries.

The models derive `SimpleObject` and `InputObject` with their geometry fields, and a resolver takes a geometry as argument, here to filter the list as `ListQuery`:

```rust
#[Object]
impl PlaceQuery {
    async fn places(&self, ctx: &Context<'_>, contains: Option<Point>, around: Option<Point>, radius: Option<f64>) -> async_graphql::Result<Vec<Place>> {
        let near = match around {
            Some(point) => Some(Near::new(point.x(), point.y(), radius.unwrap_or(1000.0)).map_err(async_graphql::Error::new)?),
            None => None,
        };
        let query = ListQuery { offset: None, limit: None, bbox: None, near, point: contains };
        Ok(Place::list(&query, app_state(ctx)?).await?)
    }
}

#[Object]
impl PlaceMutation {
    async fn create_place(&self, ctx: &Context<'_>, input: NewPlace) -> async_graphql::Result<Place> {
        Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
    }
}
```

The geometries are written as GeoJSON object literals in the query, or in the variables:

```graphql
query Inside($point: GeoJsonPoint!) {
  places(contains: $point) { id name area }
}

{ places(around: { type: "Point", coordinates: [2.2945, 48.8584] }, radius: 800) { name location } }

mutation {
  createPlace(input: {
    name: "Jardin du Luxembourg"
    location: { type: "Point", coordinates: [2.3372, 48.8462] }
    area: {
      type: "Polygon"
      coordinates: [[[2.3320, 48.8440], [2.3400, 48.8440], [2.3400, 48.8490], [2.3320, 48.8490], [2.3320, 48.8440]]]
    }
  }) { id location area }
}
```

```json
{ "point": { "type": "Point", "coordinates": [2.2945, 48.8584] } }
```

```json
{"data":{"places":[{"id":1,"name":"Tour Eiffel","area":{"type":"Polygon","coordinates":[[[2.2932,48.8578],[2.2952,48.8571],[2.2959,48.859],[2.2939,48.8597],[2.2932,48.8578]]]}}]}}
```

A geometry of another kind than the argument is a GraphQL error: `Failed to parse "GeoJsonPoint": expected a Point, found a LineString`.

## CLI and reverse engineering

The CLI proposes the geometries in the field types with `--postgres` (`location:postgis::Point`): the migration creates the `postgis` extension, `geometry(Point, 4326)` columns and their GiST indexes. [`octopux-reverse`]({{ '/reverse-engineering/' | relative_url }}) maps the `geometry` and `geography` columns to these types, except the Z and M kinds. See the [`postgis`](https://github.com/ctaque/octopux/tree/main/examples/postgis) example.
