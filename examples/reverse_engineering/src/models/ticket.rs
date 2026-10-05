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
    #[octopux_info(path = "ticket")]
    #[graphql(complex)]
    pub struct Ticket {
        pub id: Id,
        pub reference: String,
        pub subject: String,
        pub customer_id: i64,
        pub assignee_id: Option<i64>,
        pub status: String,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Ticket")]
    pub struct NewTicket {
        pub reference: String,
        pub subject: String,
        pub customer_id: i64,
        pub assignee_id: Option<i64>,
        pub status: String,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Ticket, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableTicket {
        pub id: Id,
        pub reference: String,
        pub subject: String,
        pub customer_id: i64,
        pub assignee_id: Option<i64>,
        pub status: String,
    }

    // Registers the documented routes of the ticket endpoint
    // (octopux `openapi` feature), to mount with `.configure(ticket::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Ticket, NewTicket, UpdatableTicket)(cfg)
    }


    // Fields of the GraphQL Ticket type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Ticket {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The slas of the ticket, paginated
        async fn slas(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::sla::Sla>> {
            crate::ticket_slas::resolve(ctx, self.id, offset, limit).await
        }
        /// The ticketslas of the ticket, paginated
        async fn ticketslas(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::ticketsla::Ticketsla>> {
            crate::ticket_ticketslas::resolve(ctx, self.id, offset, limit).await
        }
        /// The ticketmessages of the ticket, paginated
        async fn ticketmessages(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::ticketmessage::Ticketmessage>> {
            crate::ticket_ticketmessages::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the ticket model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(ticket::TicketQuery, ...);`
    #[derive(Default)]
    pub struct TicketQuery;

    #[Object]
    impl TicketQuery {
        async fn ticket(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Ticket> {
            find(id, app_state(ctx)?).await
        }

        async fn tickets(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Ticket>> {
            Ok(Ticket::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the ticket model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(ticket::TicketMutation, ...);`
    #[derive(Default)]
    pub struct TicketMutation;

    #[Object]
    impl TicketMutation {
        async fn create_ticket(&self, ctx: &Context<'_>, input: NewTicket) -> async_graphql::Result<Ticket> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_ticket(&self, ctx: &Context<'_>, input: UpdatableTicket) -> async_graphql::Result<Ticket> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_ticket(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Ticket> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the ticket up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Ticket> {
        match Ticket::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    