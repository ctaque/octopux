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
    #[octopux_info(path = "role")]
    #[graphql(complex)]
    pub struct Role {
        pub id: Id,
        pub name: String,
        pub description: Option<String>,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Role")]
    pub struct NewRole {
        pub name: String,
        pub description: Option<String>,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Role, FindQuery, AppState)]
    #[sqlx_model(database = "postgres")]
    pub struct UpdatableRole {
        pub id: Id,
        pub name: String,
        pub description: Option<String>,
    }

    // Registers the documented routes of the role endpoint
    // (octopux `openapi` feature), to mount with `.configure(role::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Role, NewRole, UpdatableRole)(cfg)
    }


    // Fields of the GraphQL Role type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Role {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The permissions of the role, paginated
        async fn permissions(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::permission::Permission>> {
            super::role_permissions::resolve(ctx, self.id, offset, limit).await
        }
        /// The rolepermissions of the role, paginated
        async fn rolepermissions(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::rolepermission::Rolepermission>> {
            super::role_rolepermissions::resolve(ctx, self.id, offset, limit).await
        }
        /// The accounts of the role, paginated
        async fn accounts(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::account::Account>> {
            super::role_accounts::resolve(ctx, self.id, offset, limit).await
        }
        /// The accountroles of the role, paginated
        async fn accountroles(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<super::accountrole::Accountrole>> {
            super::role_accountroles::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the role model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(role::RoleQuery, ...);`
    #[derive(Default)]
    pub struct RoleQuery;

    #[Object]
    impl RoleQuery {
        async fn role(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Role> {
            find(id, app_state(ctx)?).await
        }

        async fn roles(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Role>> {
            Ok(Role::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the role model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(role::RoleMutation, ...);`
    #[derive(Default)]
    pub struct RoleMutation;

    #[Object]
    impl RoleMutation {
        async fn create_role(&self, ctx: &Context<'_>, input: NewRole) -> async_graphql::Result<Role> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_role(&self, ctx: &Context<'_>, input: UpdatableRole) -> async_graphql::Result<Role> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_role(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Role> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the role up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Role> {
        match Role::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    