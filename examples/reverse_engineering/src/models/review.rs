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
    use crate::AppState;
    use serde::{Serialize, Deserialize};
    use octopux::{
        HttpCreate,
        HttpFindListDelete,
        HttpUpdate,
        SqlxModel,
        SqlxNewModel,
        SqlxUpdatableModel,
        octopux_info,
        Model,
        NewModel,
        UpdatableModel,
    };
    use apistos::ApiComponent;
    use schemars::JsonSchema;
    use octopux::gen_documented_endpoint;
    use async_graphql::{ComplexObject, Context, InputObject, Object, SimpleObject};

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

    #[derive(Default, Serialize, Deserialize, JsonSchema, ApiComponent, SimpleObject, sqlx::FromRow, HttpFindListDelete, SqlxModel)]
    #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    #[octopux_info(path = "review")]
    #[graphql(complex)]
    pub struct Review {
        pub id: Id,
        pub rating: i32,
        pub comment: Option<String>,
        pub product_id: i64,
        pub customer_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Review")]
    pub struct NewReview {
        pub rating: i32,
        pub comment: Option<String>,
        pub product_id: i64,
        pub customer_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Review, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableReview {
        pub id: Id,
        pub rating: i32,
        pub comment: Option<String>,
        pub product_id: i64,
        pub customer_id: i64,
    }

    // Registers the documented routes of the review endpoint
    // (octopux `openapi` feature), to mount with `.configure(review::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Review, NewReview, UpdatableReview)(cfg)
    }


    // Fields of the GraphQL Review type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Review {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
    }

    // GraphQL queries of the review model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(review::ReviewQuery, ...);`
    #[derive(Default)]
    pub struct ReviewQuery;

    #[Object]
    impl ReviewQuery {
        async fn review(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Review> {
            find(id, app_state(ctx)?).await
        }

        async fn reviews(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Review>> {
            Ok(Review::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the review model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(review::ReviewMutation, ...);`
    #[derive(Default)]
    pub struct ReviewMutation;

    #[Object]
    impl ReviewMutation {
        async fn create_review(&self, ctx: &Context<'_>, input: NewReview) -> async_graphql::Result<Review> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_review(&self, ctx: &Context<'_>, input: UpdatableReview) -> async_graphql::Result<Review> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_review(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Review> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the review up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Review> {
        match Review::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    