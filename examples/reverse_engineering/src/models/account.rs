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
    #[octopux_info(path = "account")]
    #[graphql(complex)]
    pub struct Account {
        pub id: Id,
        pub email: String,
        pub display_name: String,
        pub language_id: i64,
        pub timezone_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Account")]
    pub struct NewAccount {
        pub email: String,
        pub display_name: String,
        pub language_id: i64,
        pub timezone_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Account, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableAccount {
        pub id: Id,
        pub email: String,
        pub display_name: String,
        pub language_id: i64,
        pub timezone_id: i64,
    }

    // Registers the documented routes of the account endpoint
    // (octopux `openapi` feature), to mount with `.configure(account::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Account, NewAccount, UpdatableAccount)(cfg)
    }


    // Fields of the GraphQL Account type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Account {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The ticketmessages of the account, paginated
        async fn ticketmessages(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::ticketmessage::Ticketmessage>> {
            super::account_ticketmessages::resolve(ctx, self.id, offset, limit).await
        }
        /// The taskcomments of the account, paginated
        async fn taskcomments(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::taskcomment::Taskcomment>> {
            super::account_taskcomments::resolve(ctx, self.id, offset, limit).await
        }
        /// The sessions of the account, paginated
        async fn sessions(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::session::Session>> {
            super::account_sessions::resolve(ctx, self.id, offset, limit).await
        }
        /// The notifications of the account, paginated
        async fn notifications(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::notification::Notification>> {
            super::account_notifications::resolve(ctx, self.id, offset, limit).await
        }
        /// The employees of the account, paginated
        async fn employees(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::employee::Employee>> {
            super::account_employees::resolve(ctx, self.id, offset, limit).await
        }
        /// The auditlogs of the account, paginated
        async fn auditlogs(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::auditlog::Auditlog>> {
            super::account_auditlogs::resolve(ctx, self.id, offset, limit).await
        }
        /// The attachments of the account, paginated
        async fn attachments(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::attachment::Attachment>> {
            super::account_attachments::resolve(ctx, self.id, offset, limit).await
        }
        /// The apikeys of the account, paginated
        async fn apikeys(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::apikey::Apikey>> {
            super::account_apikeys::resolve(ctx, self.id, offset, limit).await
        }
        /// The roles of the account, paginated
        async fn roles(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::role::Role>> {
            super::account_roles::resolve(ctx, self.id, offset, limit).await
        }
        /// The accountroles of the account, paginated
        async fn accountroles(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::accountrole::Accountrole>> {
            super::account_accountroles::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the account model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(account::AccountQuery, ...);`
    #[derive(Default)]
    pub struct AccountQuery;

    #[Object]
    impl AccountQuery {
        async fn account(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Account> {
            find(id, app_state(ctx)?).await
        }

        async fn accounts(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Account>> {
            Ok(Account::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the account model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(account::AccountMutation, ...);`
    #[derive(Default)]
    pub struct AccountMutation;

    #[Object]
    impl AccountMutation {
        async fn create_account(&self, ctx: &Context<'_>, input: NewAccount) -> async_graphql::Result<Account> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_account(&self, ctx: &Context<'_>, input: UpdatableAccount) -> async_graphql::Result<Account> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_account(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Account> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the account up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Account> {
        match Account::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    