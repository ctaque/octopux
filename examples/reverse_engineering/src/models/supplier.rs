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
    #[octopux_info(path = "supplier")]
    #[graphql(complex)]
    pub struct Supplier {
        pub id: Id,
        pub name: String,
        pub vat_number: String,
        pub country_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Supplier")]
    pub struct NewSupplier {
        pub name: String,
        pub vat_number: String,
        pub country_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Supplier, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableSupplier {
        pub id: Id,
        pub name: String,
        pub vat_number: String,
        pub country_id: i64,
    }

    // Registers the documented routes of the supplier endpoint
    // (octopux `openapi` feature), to mount with `.configure(supplier::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Supplier, NewSupplier, UpdatableSupplier)(cfg)
    }


    // Fields of the GraphQL Supplier type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Supplier {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The suppliercontacts of the supplier, paginated
        async fn suppliercontacts(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::suppliercontact::Suppliercontact>> {
            crate::supplier_suppliercontacts::resolve(ctx, self.id, offset, limit).await
        }
        /// The purchaseorders of the supplier, paginated
        async fn purchaseorders(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::purchaseorder::Purchaseorder>> {
            crate::supplier_purchaseorders::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the supplier model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(supplier::SupplierQuery, ...);`
    #[derive(Default)]
    pub struct SupplierQuery;

    #[Object]
    impl SupplierQuery {
        async fn supplier(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Supplier> {
            find(id, app_state(ctx)?).await
        }

        async fn suppliers(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Supplier>> {
            Ok(Supplier::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the supplier model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(supplier::SupplierMutation, ...);`
    #[derive(Default)]
    pub struct SupplierMutation;

    #[Object]
    impl SupplierMutation {
        async fn create_supplier(&self, ctx: &Context<'_>, input: NewSupplier) -> async_graphql::Result<Supplier> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_supplier(&self, ctx: &Context<'_>, input: UpdatableSupplier) -> async_graphql::Result<Supplier> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_supplier(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Supplier> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the supplier up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Supplier> {
        match Supplier::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    