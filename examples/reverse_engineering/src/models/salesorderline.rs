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
    #[octopux_info(path = "salesorderline")]
    #[graphql(complex)]
    pub struct Salesorderline {
        pub id: Id,
        pub sales_order_id: i64,
        pub product_variant_id: i64,
        pub quantity: i64,
        pub unit_price: f64,
        pub tax_rate_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Salesorderline")]
    pub struct NewSalesorderline {
        pub sales_order_id: i64,
        pub product_variant_id: i64,
        pub quantity: i64,
        pub unit_price: f64,
        pub tax_rate_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Salesorderline, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableSalesorderline {
        pub id: Id,
        pub sales_order_id: i64,
        pub product_variant_id: i64,
        pub quantity: i64,
        pub unit_price: f64,
        pub tax_rate_id: i64,
    }

    // Registers the documented routes of the salesorderline endpoint
    // (octopux `openapi` feature), to mount with `.configure(salesorderline::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Salesorderline, NewSalesorderline, UpdatableSalesorderline)(cfg)
    }


    // Fields of the GraphQL Salesorderline type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Salesorderline {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The shipmentitems of the salesorderline, paginated
        async fn shipmentitems(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::shipmentitem::Shipmentitem>> {
            super::salesorderline_shipmentitems::resolve(ctx, self.id, offset, limit).await
        }
        /// The returnitems of the salesorderline, paginated
        async fn returnitems(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::returnitem::Returnitem>> {
            super::salesorderline_returnitems::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the salesorderline model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(salesorderline::SalesorderlineQuery, ...);`
    #[derive(Default)]
    pub struct SalesorderlineQuery;

    #[Object]
    impl SalesorderlineQuery {
        async fn salesorderline(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Salesorderline> {
            find(id, app_state(ctx)?).await
        }

        async fn salesorderlines(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Salesorderline>> {
            Ok(Salesorderline::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the salesorderline model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(salesorderline::SalesorderlineMutation, ...);`
    #[derive(Default)]
    pub struct SalesorderlineMutation;

    #[Object]
    impl SalesorderlineMutation {
        async fn create_salesorderline(&self, ctx: &Context<'_>, input: NewSalesorderline) -> async_graphql::Result<Salesorderline> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_salesorderline(&self, ctx: &Context<'_>, input: UpdatableSalesorderline) -> async_graphql::Result<Salesorderline> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_salesorderline(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Salesorderline> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the salesorderline up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Salesorderline> {
        match Salesorderline::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    