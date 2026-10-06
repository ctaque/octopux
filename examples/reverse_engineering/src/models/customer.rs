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
    #[octopux_info(path = "customer")]
    #[graphql(complex)]
    pub struct Customer {
        pub id: Id,
        pub email: String,
        pub name: String,
        pub phone: Option<String>,
        pub company_id: Option<i64>,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Customer")]
    pub struct NewCustomer {
        pub email: String,
        pub name: String,
        pub phone: Option<String>,
        pub company_id: Option<i64>,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Customer, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableCustomer {
        pub id: Id,
        pub email: String,
        pub name: String,
        pub phone: Option<String>,
        pub company_id: Option<i64>,
    }

    // Registers the documented routes of the customer endpoint
    // (octopux `openapi` feature), to mount with `.configure(customer::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Customer, NewCustomer, UpdatableCustomer)(cfg)
    }


    // Fields of the GraphQL Customer type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Customer {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The wishlists of the customer, paginated
        async fn wishlists(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::wishlist::Wishlist>> {
            super::customer_wishlists::resolve(ctx, self.id, offset, limit).await
        }
        /// The tickets of the customer, paginated
        async fn tickets(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::ticket::Ticket>> {
            super::customer_tickets::resolve(ctx, self.id, offset, limit).await
        }
        /// The salesorders of the customer, paginated
        async fn salesorders(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::salesorder::Salesorder>> {
            super::customer_salesorders::resolve(ctx, self.id, offset, limit).await
        }
        /// The reviews of the customer, paginated
        async fn reviews(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::review::Review>> {
            super::customer_reviews::resolve(ctx, self.id, offset, limit).await
        }
        /// The projects of the customer, paginated
        async fn projects(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::project::Project>> {
            super::customer_projects::resolve(ctx, self.id, offset, limit).await
        }
        /// The opportunities of the customer, paginated
        async fn opportunities(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::opportunity::Opportunity>> {
            super::customer_opportunities::resolve(ctx, self.id, offset, limit).await
        }
        /// The invoices of the customer, paginated
        async fn invoices(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::invoice::Invoice>> {
            super::customer_invoices::resolve(ctx, self.id, offset, limit).await
        }
        /// The customersegments of the customer, paginated
        async fn customersegments(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::customersegment::Customersegment>> {
            super::customer_customersegments::resolve(ctx, self.id, offset, limit).await
        }
        /// The customersegmentmembers of the customer, paginated
        async fn customersegmentmembers(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::customersegmentmember::Customersegmentmember>> {
            super::customer_customersegmentmembers::resolve(ctx, self.id, offset, limit).await
        }
        /// The customeraddresses of the customer, paginated
        async fn customeraddresses(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::customeraddress::Customeraddress>> {
            super::customer_customeraddresses::resolve(ctx, self.id, offset, limit).await
        }
        /// The carts of the customer, paginated
        async fn carts(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::cart::Cart>> {
            super::customer_carts::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the customer model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(customer::CustomerQuery, ...);`
    #[derive(Default)]
    pub struct CustomerQuery;

    #[Object]
    impl CustomerQuery {
        async fn customer(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Customer> {
            find(id, app_state(ctx)?).await
        }

        async fn customers(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Customer>> {
            Ok(Customer::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the customer model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(customer::CustomerMutation, ...);`
    #[derive(Default)]
    pub struct CustomerMutation;

    #[Object]
    impl CustomerMutation {
        async fn create_customer(&self, ctx: &Context<'_>, input: NewCustomer) -> async_graphql::Result<Customer> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_customer(&self, ctx: &Context<'_>, input: UpdatableCustomer) -> async_graphql::Result<Customer> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_customer(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Customer> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the customer up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Customer> {
        match Customer::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    