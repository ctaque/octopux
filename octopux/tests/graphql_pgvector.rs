//! The pgvector vectors as GraphQL scalars (`pgvector` and `graphql` features): lists of numbers
//! and objects in the objects, the input objects and the arguments of an async-graphql schema.

use async_graphql::{EmptySubscription, InputObject, Object, Schema, SimpleObject};
use octopux::pgvector::{HalfVector, SparseVector, Vector};
use serde_json::json;

#[derive(SimpleObject)]
struct Document {
    content: String,
    embedding: Vector,
    small: Option<HalfVector>,
    keywords: Option<SparseVector>,
}

#[derive(InputObject)]
struct NewDocument {
    content: String,
    embedding: Vector,
    keywords: Option<SparseVector>,
}

struct Query;

#[Object]
impl Query {
    async fn document(&self) -> Document {
        Document {
            content: "pgvector".into(),
            embedding: Vector(vec![0.1, -1.0]),
            small: Some(HalfVector(vec![1.0])),
            keywords: Some(SparseVector::new(4, [(2, 1.5)]).unwrap()),
        }
    }

    // the dimensions of the vector of the argument
    async fn dimensions(&self, near: Vector) -> usize {
        near.len()
    }
}

struct Mutation;

#[Object]
impl Mutation {
    // echoes the input
    async fn create_document(&self, input: NewDocument) -> Document {
        Document { content: input.content, embedding: input.embedding, small: None, keywords: input.keywords }
    }
}

fn schema() -> Schema<Query, Mutation, EmptySubscription> {
    Schema::new(Query, Mutation, EmptySubscription)
}

#[actix_web::test]
async fn vectors_are_scalars() {
    let res = schema().execute("{ document { content embedding small keywords } }").await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    assert_eq!(
        res.data.into_json().unwrap(),
        json!({ "document": {
            "content": "pgvector",
            // as in the REST routes, not the `f64` of the `f32`, 0.10000000149011612
            "embedding": [0.1, -1.0],
            "small": [1.0],
            "keywords": { "dimensions": 4, "indices": [2], "values": [1.5] },
        } })
    );
    let sdl = schema().sdl();
    for expected in ["scalar Vector", "scalar HalfVector", "scalar SparseVector", "embedding: Vector!", "keywords: SparseVector"] {
        assert!(sdl.contains(expected), "`{}` missing from\n{}", expected, sdl);
    }
}

#[actix_web::test]
async fn inputs_are_lists_objects_or_literals() {
    let res = schema()
        .execute(r#"mutation { createDocument(input: { content: "a", embedding: [1, 2.5], keywords: "{1:3}/2" }) { embedding keywords } }"#)
        .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    assert_eq!(
        res.data.into_json().unwrap(),
        json!({ "createDocument": { "embedding": [1.0, 2.5], "keywords": { "dimensions": 2, "indices": [0], "values": [3.0] } } })
    );
    let res = schema().execute(r#"{ dimensions(near: "[0.1,0.2,0.3]") }"#).await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    assert_eq!(res.data.into_json().unwrap(), json!({ "dimensions": 3 }));
    // an invalid vector is an error of the argument
    let res = schema().execute("{ dimensions(near: []) }").await;
    assert!(res.errors[0].message.contains("1 to 16000 dimensions"), "{:?}", res.errors);
}
