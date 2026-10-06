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
    use chrono::{DateTime, Utc};
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
    #[octopux_info(path = "invoice")]
    #[graphql(complex)]
    pub struct Invoice {
        pub id: Id,
        pub number: String,
        pub sales_order_id: i64,
        pub customer_id: i64,
        pub issued_at: DateTime<Utc>,
        pub total: f64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Invoice")]
    pub struct NewInvoice {
        pub number: String,
        pub sales_order_id: i64,
        pub customer_id: i64,
        pub issued_at: DateTime<Utc>,
        pub total: f64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Invoice, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableInvoice {
        pub id: Id,
        pub number: String,
        pub sales_order_id: i64,
        pub customer_id: i64,
        pub issued_at: DateTime<Utc>,
        pub total: f64,
    }

    // Registers the documented routes of the invoice endpoint
    // (octopux `openapi` feature), to mount with `.configure(invoice::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Invoice, NewInvoice, UpdatableInvoice)(cfg)
    }


    // Fields of the GraphQL Invoice type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Invoice {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The payments of the invoice, paginated
        async fn payments(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::payment::Payment>> {
            super::invoice_payments::resolve(ctx, self.id, offset, limit).await
        }
        /// The invoicelines of the invoice, paginated
        async fn invoicelines(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::invoiceline::Invoiceline>> {
            super::invoice_invoicelines::resolve(ctx, self.id, offset, limit).await
        }
        /// The creditnotes of the invoice, paginated
        async fn creditnotes(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::creditnote::Creditnote>> {
            super::invoice_creditnotes::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the invoice model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(invoice::InvoiceQuery, ...);`
    #[derive(Default)]
    pub struct InvoiceQuery;

    #[Object]
    impl InvoiceQuery {
        async fn invoice(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Invoice> {
            find(id, app_state(ctx)?).await
        }

        async fn invoices(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Invoice>> {
            Ok(Invoice::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the invoice model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(invoice::InvoiceMutation, ...);`
    #[derive(Default)]
    pub struct InvoiceMutation;

    #[Object]
    impl InvoiceMutation {
        async fn create_invoice(&self, ctx: &Context<'_>, input: NewInvoice) -> async_graphql::Result<Invoice> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_invoice(&self, ctx: &Context<'_>, input: UpdatableInvoice) -> async_graphql::Result<Invoice> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_invoice(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Invoice> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the invoice up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Invoice> {
        match Invoice::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    