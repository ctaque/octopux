//! The PostGIS geometries as GraphQL scalars (`postgis` and `graphql` features): GeoJSON in the
//! objects, the input objects and the arguments of an async-graphql schema.

use async_graphql::{EmptySubscription, InputObject, Object, Schema, SimpleObject};
use octopux::postgis::{self, geo_types, Point, Polygon};
use serde_json::json;

#[derive(SimpleObject)]
struct Place {
    name: String,
    location: Point,
    area: Option<Polygon>,
    shape: postgis::Geometry,
}

#[derive(InputObject)]
struct NewPlace {
    name: String,
    location: Point,
    area: Option<Polygon>,
}

struct Query;

#[Object]
impl Query {
    async fn place(&self) -> Place {
        Place {
            name: "Tour Eiffel".into(),
            location: geo_types::point!(x: 2.2945, y: 48.8584).into(),
            area: None,
            shape: geo_types::Geometry::LineString(vec![(0.0, 0.0), (1.0, 1.0)].into()).into(),
        }
    }
}

struct Mutation;

#[Object]
impl Mutation {
    // echoes the input, the SRID of a GeoJSON input being WGS 84
    async fn create_place(&self, input: NewPlace) -> async_graphql::Result<Place> {
        assert_eq!(input.location.srid, postgis::WGS84);
        Ok(Place { name: input.name, shape: geo_types::Geometry::Point(input.location.geometry).into(), location: input.location, area: input.area })
    }
}

fn schema() -> Schema<Query, Mutation, EmptySubscription> {
    Schema::new(Query, Mutation, EmptySubscription)
}

#[actix_web::test]
async fn geometries_are_geojson_scalars() {
    let res = schema().execute("{ place { name location area shape } }").await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    assert_eq!(
        res.data.into_json().unwrap(),
        json!({ "place": {
            "name": "Tour Eiffel",
            "location": { "type": "Point", "coordinates": [2.2945, 48.8584] },
            "area": null,
            "shape": { "type": "LineString", "coordinates": [[0.0, 0.0], [1.0, 1.0]] },
        } })
    );
}

#[actix_web::test]
async fn geometries_are_read_from_the_inputs_and_the_variables() {
    // an object literal in the query
    let res = schema()
        .execute(r#"mutation { createPlace(input: { name: "A", location: { type: "Point", coordinates: [1, 2] } }) { location area } }"#)
        .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    assert_eq!(res.data.into_json().unwrap()["createPlace"]["location"], json!({ "type": "Point", "coordinates": [1.0, 2.0] }));
    // a variable, the GeoJSON being an object or its string
    let request = async_graphql::Request::new("mutation($input: NewPlace!) { createPlace(input: $input) { area } }").variables(
        async_graphql::Variables::from_json(json!({ "input": {
            "name": "B",
            "location": r#"{"type":"Point","coordinates":[3,4]}"#,
            "area": { "type": "Polygon", "coordinates": [[[0, 0], [1, 0], [1, 1], [0, 0]]] },
        } })),
    );
    let res = schema().execute(request).await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    assert_eq!(res.data.into_json().unwrap()["createPlace"]["area"]["type"], "Polygon");
}

#[actix_web::test]
async fn invalid_geometries_are_refused() {
    let res = schema()
        .execute(r#"mutation { createPlace(input: { name: "A", location: { type: "LineString", coordinates: [[0, 0], [1, 1]] } }) { name } }"#)
        .await;
    assert!(res.errors[0].message.contains("expected a Point, found a LineString"), "{:?}", res.errors);
    let res = schema().execute(r#"mutation { createPlace(input: { name: "A", location: { type: "Point", coordinates: [1, 2, 3] } }) { name } }"#).await;
    assert!(res.errors[0].message.contains("altitude"), "{:?}", res.errors);
}

#[actix_web::test]
async fn geojson_keeps_its_member_order() {
    let res = schema().execute("{ place { location } }").await;
    let json = serde_json::to_string(&res.data).unwrap();
    assert!(json.contains(r#"{"type":"Point","coordinates":[2.2945,48.8584]}"#), "{}", json);
}

#[test]
fn each_kind_is_its_own_scalar() {
    let sdl = schema().sdl();
    for scalar in ["scalar GeoJsonPoint", "scalar GeoJsonPolygon", "scalar GeoJsonGeometry"] {
        assert!(sdl.contains(scalar), "{} missing from\n{}", scalar, sdl);
    }
    assert!(sdl.contains("location: GeoJsonPoint!"), "{}", sdl);
    assert!(sdl.contains("area: GeoJsonPolygon"), "{}", sdl);
    assert!(sdl.contains("A GeoJSON geometry (RFC 7946)"), "{}", sdl);
}
