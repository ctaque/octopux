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
    #[octopux_info(path = "warehouse")]
    #[graphql(complex)]
    pub struct Warehouse {
        pub id: Id,
        pub code: String,
        pub name: String,
        pub address_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Warehouse")]
    pub struct NewWarehouse {
        pub code: String,
        pub name: String,
        pub address_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Warehouse, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableWarehouse {
        pub id: Id,
        pub code: String,
        pub name: String,
        pub address_id: i64,
    }

    // Registers the documented routes of the warehouse endpoint
    // (octopux `openapi` feature), to mount with `.configure(warehouse::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Warehouse, NewWarehouse, UpdatableWarehouse)(cfg)
    }


    // Fields of the GraphQL Warehouse type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Warehouse {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The storagelocations of the warehouse, paginated
        async fn storagelocations(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::storagelocation::Storagelocation>> {
            super::warehouse_storagelocations::resolve(ctx, self.id, offset, limit).await
        }
        /// The stockmovements_by_to_warehouse of the warehouse, paginated
        async fn stockmovements_by_to_warehouse(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::stockmovement::Stockmovement>> {
            super::warehouse_stockmovements_by_to_warehouse::resolve(ctx, self.id, offset, limit).await
        }
        /// The stockmovements of the warehouse, paginated
        async fn stockmovements(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::stockmovement::Stockmovement>> {
            super::warehouse_stockmovements::resolve(ctx, self.id, offset, limit).await
        }
        /// The shipments of the warehouse, paginated
        async fn shipments(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::shipment::Shipment>> {
            super::warehouse_shipments::resolve(ctx, self.id, offset, limit).await
        }
        /// The purchaseorders of the warehouse, paginated
        async fn purchaseorders(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::purchaseorder::Purchaseorder>> {
            super::warehouse_purchaseorders::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the warehouse model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(warehouse::WarehouseQuery, ...);`
    #[derive(Default)]
    pub struct WarehouseQuery;

    #[Object]
    impl WarehouseQuery {
        async fn warehouse(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Warehouse> {
            find(id, app_state(ctx)?).await
        }

        async fn warehouses(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Warehouse>> {
            Ok(Warehouse::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the warehouse model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(warehouse::WarehouseMutation, ...);`
    #[derive(Default)]
    pub struct WarehouseMutation;

    #[Object]
    impl WarehouseMutation {
        async fn create_warehouse(&self, ctx: &Context<'_>, input: NewWarehouse) -> async_graphql::Result<Warehouse> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_warehouse(&self, ctx: &Context<'_>, input: UpdatableWarehouse) -> async_graphql::Result<Warehouse> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_warehouse(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Warehouse> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the warehouse up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Warehouse> {
        match Warehouse::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    