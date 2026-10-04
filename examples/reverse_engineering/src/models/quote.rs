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
    #[octopux_info(path = "quote")]
    #[graphql(complex)]
    pub struct Quote {
        pub id: Id,
        pub number: String,
        pub opportunity_id: i64,
        pub total: f64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Quote")]
    pub struct NewQuote {
        pub number: String,
        pub opportunity_id: i64,
        pub total: f64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Quote, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableQuote {
        pub id: Id,
        pub number: String,
        pub opportunity_id: i64,
        pub total: f64,
    }

    // Registers the documented routes of the quote endpoint
    // (octopux `openapi` feature), to mount with `.configure(quote::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Quote, NewQuote, UpdatableQuote)(cfg)
    }


    // Fields of the GraphQL Quote type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Quote {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The quotelines of the quote, paginated
        async fn quotelines(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::quoteline::Quoteline>> {
            crate::quote_quotelines::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the quote model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(quote::QuoteQuery, ...);`
    #[derive(Default)]
    pub struct QuoteQuery;

    #[Object]
    impl QuoteQuery {
        async fn quote(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Quote> {
            find(id, app_state(ctx)?).await
        }

        async fn quotes(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Quote>> {
            Ok(Quote::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the quote model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(quote::QuoteMutation, ...);`
    #[derive(Default)]
    pub struct QuoteMutation;

    #[Object]
    impl QuoteMutation {
        async fn create_quote(&self, ctx: &Context<'_>, input: NewQuote) -> async_graphql::Result<Quote> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_quote(&self, ctx: &Context<'_>, input: UpdatableQuote) -> async_graphql::Result<Quote> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_quote(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Quote> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the quote up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Quote> {
        match Quote::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    