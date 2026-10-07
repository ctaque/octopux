//! End-to-end tests of the pgvector vectors and of the similarity search, through the routes of
//! models deriving the sqlx derives, on a live PostgreSQL database with pgvector.
//!
//! They are ignored by `cargo test`, and never run in the CI, even with `--ignored`. To run them:
//!
//! ```bash
//! docker run -d --rm -e POSTGRES_PASSWORD=postgres -p 5433:5432 pgvector/pgvector:pg17
//! DATABASE_URL=postgres://postgres:postgres@localhost:5433/postgres \
//!     cargo test -p octopux --features pgvector --test pgvector -- --ignored
//! ```

use actix_web::{test, web, App};
use octopux::pgvector::{HalfVector, SparseVector, Vector};
use octopux::{
    gen_endpoint, octopux_info, HttpCreate, HttpFindListDelete, HttpUpdate, SqlxFilter, SqlxModel, SqlxNewModel, SqlxUpdatableModel,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::postgres::PgPool;

const IGNORED: &str = "needs a PostgreSQL database with pgvector in DATABASE_URL";

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
    name_ne: Option<String>,
    #[sqlx_filter(column = "embedding", op = "nearest")]
    near: Option<Vector>,
    #[sqlx_filter(column = "embedding", op = "nearest", distance = "l2")]
    near_l2: Option<Vector>,
    #[sqlx_filter(column = "embedding", op = "nearest", distance = "inner_product")]
    near_ip: Option<Vector>,
    #[sqlx_filter(column = "embedding", op = "nearest", distance = "l1")]
    near_l1: Option<Vector>,
    #[sqlx_filter(column = "small", op = "nearest")]
    small_near: Option<HalfVector>,
    #[sqlx_filter(column = "keywords", op = "nearest", distance = "inner_product")]
    keywords_near: Option<SparseVector>,
    #[sqlx_filter(sort = "name")]
    sort: Option<String>,
}

// `embedding` a vector(3), `small` a halfvec(3), `keywords` a sparsevec(5)
#[derive(Debug, Default, Serialize, Deserialize, sqlx::FromRow, HttpFindListDelete, SqlxModel)]
#[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
#[sqlx_model(database = "postgres", table = "octopux_document", filter)]
#[octopux_info(path = "document")]
struct Document {
    id: Id,
    name: String,
    embedding: Vector,
    small: Option<HalfVector>,
    keywords: Option<SparseVector>,
}

#[derive(Serialize, Deserialize, HttpCreate, SqlxNewModel)]
#[http_create(SaveQuery, AppState)]
#[sqlx_model(database = "postgres", model = "Document", table = "octopux_document")]
struct NewDocument {
    name: String,
    embedding: Vector,
    small: Option<HalfVector>,
    keywords: Option<SparseVector>,
}

#[derive(Serialize, Deserialize, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
#[http_update(Id, UpdateQuery, Document, FindQuery, AppState)]
#[sqlx_model(database = "postgres", table = "octopux_document")]
struct UpdatableDocument {
    id: Id,
    embedding: Vector,
}

// The pool of DATABASE_URL, None in the CI, where the pgvector tests are forbidden
async fn pool() -> Option<PgPool> {
    if std::env::var_os("CI").is_some() {
        eprintln!("skipped: the pgvector tests do not run in the CI");
        return None;
    }
    let url = std::env::var("DATABASE_URL").expect(IGNORED);
    Some(PgPool::connect(&url).await.expect("DATABASE_URL accepts connections"))
}

// The document table, created again, the tests running one after the other on it
async fn reset(pool: &PgPool) {
    sqlx::raw_sql(
        "CREATE EXTENSION IF NOT EXISTS vector;
         DROP TABLE IF EXISTS octopux_document;
         CREATE TABLE octopux_document (
             id BIGSERIAL PRIMARY KEY,
             name TEXT NOT NULL,
             embedding vector(3) NOT NULL,
             small halfvec(3),
             keywords sparsevec(5)
         );
         CREATE INDEX ON octopux_document USING hnsw (embedding vector_cosine_ops);",
    )
    .execute(pool)
    .await
    .unwrap();
}

// The two tests share the table, they are run as one
#[actix_web::test]
#[ignore = "needs a PostgreSQL database with pgvector in DATABASE_URL"]
async fn pgvector() {
    let Some(pool) = pool().await else { return };
    reset(&pool).await;
    vectors_are_stored_and_read_back(&pool).await;
    reset(&pool).await;
    nearest_orders_the_rows_by_distance(&pool).await;
}

async fn vectors_are_stored_and_read_back(pool: &PgPool) {
    let state = AppState { pool: pool.clone() };
    use octopux::{Model, NewModel, UpdatableModel};

    let new: NewDocument = serde_json::from_value(json!({
        "name": "a",
        "embedding": [1, -2.5, 0.125],
        "small": [0.1, 1, 65504],
        "keywords": { "dimensions": 5, "indices": [3, 0], "values": [2, 1.5] },
    }))
    .unwrap();
    let saved = new.save(&SaveQuery {}, &state).await.unwrap();
    assert_eq!(saved.embedding, Vector(vec![1.0, -2.5, 0.125]));
    // rounded to half precision
    assert_eq!(saved.small, Some(HalfVector(vec![0.099975586, 1.0, 65504.0])));
    assert_eq!(saved.keywords.as_ref().unwrap().to_dense(), [1.5, 0.0, 0.0, 2.0, 0.0]);

    // pgvector reads what was written
    let (embedding, small, keywords): (String, String, String) =
        sqlx::query_as("SELECT embedding::text, small::text, keywords::text FROM octopux_document WHERE id = $1")
            .bind(saved.id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(embedding, "[1,-2.5,0.125]");
    assert_eq!(small, "[0.099975586,1,65504]");
    assert_eq!(keywords, "{1:1.5,4:2}/5");

    let found = Document::find(saved.id, &FindQuery {}, &state).await.unwrap();
    assert_eq!(found.keywords, saved.keywords);
    let updated = UpdatableDocument { id: saved.id, embedding: Vector(vec![0.0, 0.0, 1.0]) }.update(&UpdateQuery {}, &state).await.unwrap();
    assert_eq!(updated.embedding, Vector(vec![0.0, 0.0, 1.0]));

    // a vector of other dimensions than its column is refused by pgvector
    let other = NewDocument { name: "x".into(), embedding: Vector(vec![1.0, 2.0]), small: None, keywords: None };
    let err = other.save(&SaveQuery {}, &state).await.unwrap_err();
    assert!(err.to_string().contains("expected 3 dimensions, not 2"), "{}", err);

    // the text format and the arrays
    use sqlx::Row;
    let rows = sqlx::raw_sql("SELECT embedding, small, keywords FROM octopux_document").fetch_all(pool).await.unwrap();
    assert_eq!(rows[0].try_get::<Vector, _>(0).unwrap(), Vector(vec![0.0, 0.0, 1.0]));
    assert_eq!(rows[0].try_get::<HalfVector, _>(1).unwrap(), saved.small.clone().unwrap());
    assert_eq!(rows[0].try_get::<SparseVector, _>(2).unwrap(), saved.keywords.clone().unwrap());
    let vectors: Vec<Vector> = sqlx::query_scalar("SELECT ARRAY['[1,2]'::vector, '[3]'::vector]").fetch_one(pool).await.unwrap();
    assert_eq!(vectors, [Vector(vec![1.0, 2.0]), Vector(vec![3.0])]);
    let sparse: SparseVector = sqlx::query_scalar("SELECT $1::sparsevec").bind(SparseVector::new(3, []).unwrap()).fetch_one(pool).await.unwrap();
    assert_eq!((sparse.dimensions(), sparse.indices().len()), (3, 0));
}

async fn nearest_orders_the_rows_by_distance(pool: &PgPool) {
    let app = test::init_service(
        App::new().app_data(web::Data::new(AppState { pool: pool.clone() })).configure(gen_endpoint!(Document, NewDocument, UpdatableDocument)),
    )
    .await;
    // to [1, 0, 0], cosine: a, b, e, c, d; l2: b, c, d, e, a; inner product: a, e, b, c, d; l1: b, c = d, e, a
    let documents = [
        ("a", [10.0, 1.0, 0.0], [0, 1]),
        ("b", [1.0, 0.5, 0.0], [1, 2]),
        ("c", [0.0, 1.0, 0.0], [2, 3]),
        ("d", [-1.0, 0.0, 0.0], [3, 4]),
        ("e", [2.0, 2.0, 0.0], [0, 4]),
    ];
    for (i, (name, embedding, keywords)) in documents.into_iter().enumerate() {
        let body = json!({
            "name": name,
            "embedding": embedding,
            "small": embedding,
            "keywords": { "dimensions": 5, "indices": keywords, "values": [1, i + 1] },
        });
        let res = test::call_service(&app, test::TestRequest::post().uri("/document").set_json(body).to_request()).await;
        assert!(res.status().is_success(), "{}", res.status());
    }
    let list = |query: &str| {
        let req = test::TestRequest::get().uri(&format!("/document?{}", query)).to_request();
        let app = &app;
        async move {
            let res = test::call_service(app, req).await;
            let status = res.status().as_u16();
            let body = test::read_body(res).await;
            match serde_json::from_slice::<Value>(&body) {
                Ok(Value::Array(documents)) => Ok(documents.iter().map(|d| d["name"].as_str().unwrap().to_string()).collect::<Vec<_>>()),
                _ => Err((status, String::from_utf8_lossy(&body).to_string())),
            }
        }
    };

    assert_eq!(list("near=%5B1,0,0%5D").await.unwrap(), ["a", "b", "e", "c", "d"]);
    assert_eq!(list("near=1,0,0&limit=2").await.unwrap(), ["a", "b"]);
    assert_eq!(list("near=1,0,0&name_ne=a&limit=2").await.unwrap(), ["b", "e"]);
    assert_eq!(list("near_l2=1,0,0").await.unwrap(), ["b", "c", "d", "e", "a"]);
    assert_eq!(list("near_ip=1,0,0").await.unwrap(), ["a", "e", "b", "c", "d"]);
    // the sort columns order the rows at the same distance
    assert_eq!(list("near_l1=1,0,0&sort=-name").await.unwrap(), ["b", "d", "c", "e", "a"]);
    assert_eq!(list("near_l1=1,0,0&sort=name").await.unwrap(), ["b", "c", "d", "e", "a"]);
    assert_eq!(list("small_near=1,0,0").await.unwrap(), ["a", "b", "e", "c", "d"]);
    // the largest inner product with the keyword 4 (index 3): c (value 3), then d (value 1)
    assert_eq!(list("keywords_near=%7B4:1%7D/5&limit=2").await.unwrap(), ["c", "d"]);
    // without vector, by id
    assert_eq!(list("sort=-name&limit=1").await.unwrap(), ["e"]);
    assert_eq!(list("").await.unwrap(), ["a", "b", "c", "d", "e"]);

    // the invalid vectors are client errors
    let (status, body) = list("near=1,x").await.unwrap_err();
    assert_eq!(status, 400);
    assert!(body.contains("invalid vector"), "{}", body);
    // a vector of other dimensions than the column is refused by pgvector, a client error too
    let (status, body) = list("near=1,0").await.unwrap_err();
    assert_eq!(status, 400, "{}", body);
    assert!(body.contains("BAD_REQUEST"), "{}", body);
    let res = test::call_service(
        &app,
        test::TestRequest::post().uri("/document").set_json(json!({ "name": "f", "embedding": [1, 2, 3, 4] })).to_request(),
    )
    .await;
    assert_eq!(res.status().as_u16(), 400);
}
