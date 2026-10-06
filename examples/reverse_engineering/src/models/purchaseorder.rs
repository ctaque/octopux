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
    #[octopux_info(path = "purchaseorder")]
    #[graphql(complex)]
    pub struct Purchaseorder {
        pub id: Id,
        pub reference: String,
        pub supplier_id: i64,
        pub warehouse_id: i64,
        pub status: String,
        pub total: f64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Purchaseorder")]
    pub struct NewPurchaseorder {
        pub reference: String,
        pub supplier_id: i64,
        pub warehouse_id: i64,
        pub status: String,
        pub total: f64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Purchaseorder, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatablePurchaseorder {
        pub id: Id,
        pub reference: String,
        pub supplier_id: i64,
        pub warehouse_id: i64,
        pub status: String,
        pub total: f64,
    }

    // Registers the documented routes of the purchaseorder endpoint
    // (octopux `openapi` feature), to mount with `.configure(purchaseorder::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Purchaseorder, NewPurchaseorder, UpdatablePurchaseorder)(cfg)
    }


    // Fields of the GraphQL Purchaseorder type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Purchaseorder {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The purchaseorderlines of the purchaseorder, paginated
        async fn purchaseorderlines(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::purchaseorderline::Purchaseorderline>> {
            super::purchaseorder_purchaseorderlines::resolve(ctx, self.id, offset, limit).await
        }
        /// The goodsreceipts of the purchaseorder, paginated
        async fn goodsreceipts(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::goodsreceipt::Goodsreceipt>> {
            super::purchaseorder_goodsreceipts::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the purchaseorder model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(purchaseorder::PurchaseorderQuery, ...);`
    #[derive(Default)]
    pub struct PurchaseorderQuery;

    #[Object]
    impl PurchaseorderQuery {
        async fn purchaseorder(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Purchaseorder> {
            find(id, app_state(ctx)?).await
        }

        async fn purchaseorders(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Purchaseorder>> {
            Ok(Purchaseorder::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the purchaseorder model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(purchaseorder::PurchaseorderMutation, ...);`
    #[derive(Default)]
    pub struct PurchaseorderMutation;

    #[Object]
    impl PurchaseorderMutation {
        async fn create_purchaseorder(&self, ctx: &Context<'_>, input: NewPurchaseorder) -> async_graphql::Result<Purchaseorder> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_purchaseorder(&self, ctx: &Context<'_>, input: UpdatablePurchaseorder) -> async_graphql::Result<Purchaseorder> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_purchaseorder(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Purchaseorder> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the purchaseorder up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Purchaseorder> {
        match Purchaseorder::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    