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
    #[octopux_info(path = "attachment")]
    #[graphql(complex)]
    pub struct Attachment {
        pub id: Id,
        pub filename: String,
        pub url: String,
        pub uploaded_by_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Attachment")]
    pub struct NewAttachment {
        pub filename: String,
        pub url: String,
        pub uploaded_by_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Attachment, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableAttachment {
        pub id: Id,
        pub filename: String,
        pub url: String,
        pub uploaded_by_id: i64,
    }

    // Registers the documented routes of the attachment endpoint
    // (octopux `openapi` feature), to mount with `.configure(attachment::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Attachment, NewAttachment, UpdatableAttachment)(cfg)
    }


    // Fields of the GraphQL Attachment type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Attachment {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The tasks of the attachment, paginated
        async fn tasks(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::task::Task>> {
            crate::attachment_tasks::resolve(ctx, self.id, offset, limit).await
        }
        /// The taskattachments of the attachment, paginated
        async fn taskattachments(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::taskattachment::Taskattachment>> {
            crate::attachment_taskattachments::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the attachment model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(attachment::AttachmentQuery, ...);`
    #[derive(Default)]
    pub struct AttachmentQuery;

    #[Object]
    impl AttachmentQuery {
        async fn attachment(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Attachment> {
            find(id, app_state(ctx)?).await
        }

        async fn attachments(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Attachment>> {
            Ok(Attachment::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the attachment model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(attachment::AttachmentMutation, ...);`
    #[derive(Default)]
    pub struct AttachmentMutation;

    #[Object]
    impl AttachmentMutation {
        async fn create_attachment(&self, ctx: &Context<'_>, input: NewAttachment) -> async_graphql::Result<Attachment> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_attachment(&self, ctx: &Context<'_>, input: UpdatableAttachment) -> async_graphql::Result<Attachment> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_attachment(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Attachment> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the attachment up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Attachment> {
        match Attachment::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    