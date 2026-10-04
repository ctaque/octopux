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
    #[octopux_info(path = "address")]
    #[graphql(complex)]
    pub struct Address {
        pub id: Id,
        pub line1: String,
        pub line2: Option<String>,
        pub city_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Address")]
    pub struct NewAddress {
        pub line1: String,
        pub line2: Option<String>,
        pub city_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Address, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableAddress {
        pub id: Id,
        pub line1: String,
        pub line2: Option<String>,
        pub city_id: i64,
    }

    // Registers the documented routes of the address endpoint
    // (octopux `openapi` feature), to mount with `.configure(address::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Address, NewAddress, UpdatableAddress)(cfg)
    }


    // Fields of the GraphQL Address type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Address {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The warehouses of the address, paginated
        async fn warehouses(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::warehouse::Warehouse>> {
            crate::address_warehouses::resolve(ctx, self.id, offset, limit).await
        }
        /// The salesorders_by_shipping_address of the address, paginated
        async fn salesorders_by_shipping_address(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::salesorder::Salesorder>> {
            crate::address_salesorders_by_shipping_address::resolve(ctx, self.id, offset, limit).await
        }
        /// The salesorders of the address, paginated
        async fn salesorders(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::salesorder::Salesorder>> {
            crate::address_salesorders::resolve(ctx, self.id, offset, limit).await
        }
        /// The customeraddresses of the address, paginated
        async fn customeraddresses(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::customeraddress::Customeraddress>> {
            crate::address_customeraddresses::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the address model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(address::AddressQuery, ...);`
    #[derive(Default)]
    pub struct AddressQuery;

    #[Object]
    impl AddressQuery {
        async fn address(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Address> {
            find(id, app_state(ctx)?).await
        }

        async fn addresses(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Address>> {
            Ok(Address::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the address model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(address::AddressMutation, ...);`
    #[derive(Default)]
    pub struct AddressMutation;

    #[Object]
    impl AddressMutation {
        async fn create_address(&self, ctx: &Context<'_>, input: NewAddress) -> async_graphql::Result<Address> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_address(&self, ctx: &Context<'_>, input: UpdatableAddress) -> async_graphql::Result<Address> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_address(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Address> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the address up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Address> {
        match Address::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    