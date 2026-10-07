# PostGIS example

Places with a `geometry(Point, 4326)` location and an optional `geometry(Polygon, 4326)` area,
served as GeoJSON by the sqlx derives (octopux `postgis` feature).

```bash
docker compose up -d   # PostgreSQL with PostGIS on localhost:5432
cargo run              # migrates, seeds a few places in Paris, listens on 127.0.0.1:8085
```

| Route | Action |
| --- | --- |
| `GET /v1/place` | List a page of places |
| `GET /v1/place/{id}` | Find a place |
| `POST /v1/place` | Create a place |
| `PUT /v1/place/{id}` | Update a place |
| `DELETE /v1/place/{id}` | Delete a place |
| `GET /v1/place/{id}/nearby?radius=1500` | The other places within `radius` meters, the closest first |

Swagger UI is served on `/swagger`.

```bash
curl -X POST localhost:8085/v1/place -H 'content-type: application/json' -d '{
  "name": "Sacré-Cœur",
  "location": { "type": "Point", "coordinates": [2.3431, 48.8867] },
  "area": null
}'

# the places within 1.5 km of the Eiffel Tower
curl 'localhost:8085/v1/place/1/nearby?radius=1500'
```

The coordinates are `[longitude, latitude]` in WGS 84. A geometry of another kind than the column
(a `LineString` for `location`) or with an altitude is answered 400.
