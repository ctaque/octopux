# PostGIS example

Places with a `geometry(Point, 4326)` location and an optional `geometry(Polygon, 4326)` area,
served as GeoJSON by the sqlx derives (octopux `postgis` feature), on REST routes documented with
OpenAPI and in a GraphQL schema, where the geometries are GeoJSON scalars (octopux `graphql` feature).

```bash
docker compose up -d   # PostgreSQL with PostGIS on localhost:5432
cargo run              # migrates, seeds a few places in Paris, listens on 127.0.0.1:8085
```

| Route | Action |
| --- | --- |
| `GET /v1/place` | List a page of places, filtered by `bbox`, `near` and `point` |
| `GET /v1/place/{id}` | Find a place |
| `POST /v1/place` | Create a place |
| `PUT /v1/place/{id}` | Update a place |
| `DELETE /v1/place/{id}` | Delete a place |
| `GET /v1/place/{id}/nearby?radius=1500` | The other places within `radius` meters, the closest first |

Swagger UI is served on `/swagger`, GraphiQL on `GET /graphql`.

The list is filtered by the spatial filters of its query string (`SqlxFilter` derive), which can be
combined with each other and with `offset` and `limit`:

| Filter | Condition | Example |
| --- | --- | --- |
| `bbox=west,south,east,north` | `ST_Intersects(location, ST_MakeEnvelope(...))` | `?bbox=2.28,48.85,2.30,48.87` |
| `near=longitude,latitude,meters` | `ST_DWithin(location::geography, point, meters)` | `?near=2.2945,48.8584,800` |
| `point=<GeoJSON point>` | `ST_Contains(area, point)` | `?point={"type":"Point","coordinates":[2.2945,48.8584]}` |

```bash
curl -X POST localhost:8085/v1/place -H 'content-type: application/json' -d '{
  "name": "Sacré-Cœur",
  "location": { "type": "Point", "coordinates": [2.3431, 48.8867] },
  "area": null
}'

# the places within 1.5 km of the Eiffel Tower
curl 'localhost:8085/v1/place/1/nearby?radius=1500'

# the places in a box around the Trocadéro
curl 'localhost:8085/v1/place?bbox=2.28,48.85,2.30,48.87'

# the places within 800 m of a point
curl 'localhost:8085/v1/place?near=2.2945,48.8584,800'

# the places whose area contains a point, the GeoJSON being URL encoded by curl
curl -G localhost:8085/v1/place --data-urlencode 'point={"type":"Point","coordinates":[2.2945,48.8584]}'
```

## GraphQL

The schema is served on `POST /graphql`, next to the REST routes and on the same database. The
geometries are the `GeoJsonPoint` and `GeoJsonPolygon` scalars: the same GeoJSON as in the REST
routes, written as an object literal in the query (or its string, in the variables too).

### Queries

```graphql
# a place and its geometries
{ place(id: 1) { name location area } }

# the places whose area contains a point, and the places within 800 m of a point
{ places(contains: { type: "Point", coordinates: [2.2945, 48.8584] }) { name } }
{ places(around: { type: "Point", coordinates: [2.2945, 48.8584] }, radius: 800, limit: 5) { name location } }

# the point as a variable, of the GeoJsonPoint type
query Inside($point: GeoJsonPoint!) {
  places(contains: $point) { id name area }
}
```

```json
{ "point": { "type": "Point", "coordinates": [2.2945, 48.8584] } }
```

```json
{"data":{"places":[{"id":1,"name":"Tour Eiffel","area":{"type":"Polygon","coordinates":[[[2.2932,48.8578],[2.2952,48.8571],[2.2959,48.859],[2.2939,48.8597],[2.2932,48.8578]]]}}]}}
```

### Mutations

```graphql
# a place with a location, an area and a multipoint, the optional geometries left out being null
mutation {
  createPlace(input: {
    name: "Jardin du Luxembourg"
    location: { type: "Point", coordinates: [2.3372, 48.8462] }
    area: {
      type: "Polygon"
      coordinates: [[[2.3320, 48.8440], [2.3400, 48.8440], [2.3400, 48.8490], [2.3320, 48.8490], [2.3320, 48.8440]]]
    }
    multipoint: { type: "MultiPoint", coordinates: [[2.3368, 48.8478], [2.3390, 48.8455]] }
  }) { id name location area multipoint }
}

# the input as a variable; the update replaces every column, a geometry left out becomes null
mutation Move($input: UpdatablePlace!) {
  updatePlace(input: $input) { id name location multipolygon updatedAt }
}

# soft delete, the place is returned with its deletedAt
mutation { deletePlace(id: 7) { id name deletedAt } }
```

The variables of `Move`, a `MultiPolygon` of two polygons:

```json
{
  "input": {
    "id": 7,
    "name": "Luxembourg",
    "location": { "type": "Point", "coordinates": [2.3371, 48.8463] },
    "multipolygon": {
      "type": "MultiPolygon",
      "coordinates": [
        [[[2.3320, 48.8440], [2.3360, 48.8440], [2.3360, 48.8490], [2.3320, 48.8440]]],
        [[[2.3370, 48.8440], [2.3400, 48.8440], [2.3400, 48.8490], [2.3370, 48.8440]]]
      ]
    }
  }
}
```

### With curl

```bash
curl localhost:8085/graphql -H 'content-type: application/json' \
  -d '{"query": "{ place(id: 1) { name location } }"}'
# {"data":{"place":{"name":"Tour Eiffel","location":{"type":"Point","coordinates":[2.2945,48.8584]}}}}

curl localhost:8085/graphql -H 'content-type: application/json' -d '{
  "query": "query Inside($point: GeoJsonPoint!) { places(contains: $point) { name } }",
  "variables": { "point": { "type": "Point", "coordinates": [2.2945, 48.8584] } }
}'
# {"data":{"places":[{"name":"Tour Eiffel"}]}}

curl localhost:8085/graphql -H 'content-type: application/json' -d '{
  "query": "mutation New($input: NewPlace!) { createPlace(input: $input) { id location } }",
  "variables": { "input": { "name": "Sacré-Cœur", "location": { "type": "Point", "coordinates": [2.3431, 48.8867] } } }
}'
```

A geometry of another kind than the field is a GraphQL error:
`Failed to parse "GeoJsonPoint": expected a Point, found a LineString`.

The coordinates are `[longitude, latitude]` in WGS 84. A geometry of another kind than the column
(a `LineString` for `location`) or with an altitude is answered 400, as is an invalid filter
(`?bbox=1,2,3`, a latitude beyond 90, a negative distance).
