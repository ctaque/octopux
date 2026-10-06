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
    #[octopux_info(path = "salesorder")]
    #[graphql(complex)]
    pub struct Salesorder {
        pub id: Id,
        pub reference: String,
        pub customer_id: i64,
        pub billing_address_id: i64,
        pub shipping_address_id: i64,
        pub currency_id: i64,
        pub status: String,
        pub total: f64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Salesorder")]
    pub struct NewSalesorder {
        pub reference: String,
        pub customer_id: i64,
        pub billing_address_id: i64,
        pub shipping_address_id: i64,
        pub currency_id: i64,
        pub status: String,
        pub total: f64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Salesorder, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableSalesorder {
        pub id: Id,
        pub reference: String,
        pub customer_id: i64,
        pub billing_address_id: i64,
        pub shipping_address_id: i64,
        pub currency_id: i64,
        pub status: String,
        pub total: f64,
    }

    // Registers the documented routes of the salesorder endpoint
    // (octopux `openapi` feature), to mount with `.configure(salesorder::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Salesorder, NewSalesorder, UpdatableSalesorder)(cfg)
    }


    // Fields of the GraphQL Salesorder type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Salesorder {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The shipments of the salesorder, paginated
        async fn shipments(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::shipment::Shipment>> {
            super::salesorder_shipments::resolve(ctx, self.id, offset, limit).await
        }
        /// The salesorderlines of the salesorder, paginated
        async fn salesorderlines(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::salesorderline::Salesorderline>> {
            super::salesorder_salesorderlines::resolve(ctx, self.id, offset, limit).await
        }
        /// The returnrequests of the salesorder, paginated
        async fn returnrequests(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::returnrequest::Returnrequest>> {
            super::salesorder_returnrequests::resolve(ctx, self.id, offset, limit).await
        }
        /// The coupons of the salesorder, paginated
        async fn coupons(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::coupon::Coupon>> {
            super::salesorder_coupons::resolve(ctx, self.id, offset, limit).await
        }
        /// The ordercoupons of the salesorder, paginated
        async fn ordercoupons(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::ordercoupon::Ordercoupon>> {
            super::salesorder_ordercoupons::resolve(ctx, self.id, offset, limit).await
        }
        /// The invoices of the salesorder, paginated
        async fn invoices(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::invoice::Invoice>> {
            super::salesorder_invoices::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the salesorder model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(salesorder::SalesorderQuery, ...);`
    #[derive(Default)]
    pub struct SalesorderQuery;

    #[Object]
    impl SalesorderQuery {
        async fn salesorder(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Salesorder> {
            find(id, app_state(ctx)?).await
        }

        async fn salesorders(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Salesorder>> {
            Ok(Salesorder::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the salesorder model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(salesorder::SalesorderMutation, ...);`
    #[derive(Default)]
    pub struct SalesorderMutation;

    #[Object]
    impl SalesorderMutation {
        async fn create_salesorder(&self, ctx: &Context<'_>, input: NewSalesorder) -> async_graphql::Result<Salesorder> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_salesorder(&self, ctx: &Context<'_>, input: UpdatableSalesorder) -> async_graphql::Result<Salesorder> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_salesorder(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Salesorder> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the salesorder up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Salesorder> {
        match Salesorder::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    