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
    #[octopux_info(path = "webhook")]
    #[graphql(complex)]
    pub struct Webhook {
        pub id: Id,
        pub url: String,
        pub event: String,
        pub active: bool,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Webhook")]
    pub struct NewWebhook {
        pub url: String,
        pub event: String,
        pub active: bool,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Webhook, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableWebhook {
        pub id: Id,
        pub url: String,
        pub event: String,
        pub active: bool,
    }

    // Registers the documented routes of the webhook endpoint
    // (octopux `openapi` feature), to mount with `.configure(webhook::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Webhook, NewWebhook, UpdatableWebhook)(cfg)
    }


    // Fields of the GraphQL Webhook type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Webhook {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The webhookdeliveries of the webhook, paginated
        async fn webhookdeliveries(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::webhookdelivery::Webhookdelivery>> {
            super::webhook_webhookdeliveries::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the webhook model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(webhook::WebhookQuery, ...);`
    #[derive(Default)]
    pub struct WebhookQuery;

    #[Object]
    impl WebhookQuery {
        async fn webhook(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Webhook> {
            find(id, app_state(ctx)?).await
        }

        async fn webhooks(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Webhook>> {
            Ok(Webhook::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the webhook model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(webhook::WebhookMutation, ...);`
    #[derive(Default)]
    pub struct WebhookMutation;

    #[Object]
    impl WebhookMutation {
        async fn create_webhook(&self, ctx: &Context<'_>, input: NewWebhook) -> async_graphql::Result<Webhook> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_webhook(&self, ctx: &Context<'_>, input: UpdatableWebhook) -> async_graphql::Result<Webhook> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_webhook(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Webhook> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the webhook up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Webhook> {
        match Webhook::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    