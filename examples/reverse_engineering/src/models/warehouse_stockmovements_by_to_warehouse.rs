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
    use crate::warehouse::{Warehouse, Id};
    use crate::stockmovement::Stockmovement;
    use serde::Deserialize;
    use octopux::{
        HasMany,
        anyhow::Result,
        async_trait,
    };
    use apistos::ApiComponent;
    use schemars::JsonSchema;
    use octopux::gen_documented_relation_endpoint;

    #[derive(Deserialize, JsonSchema, ApiComponent)]
    pub struct WarehouseStockmovementsByToWarehouseQuery {
        /// Number of rows to skip
        pub offset: Option<usize>,
        /// Maximum number of rows to return (20 by default, 100 at most)
        pub limit: Option<usize>,
    }
    /// Number of children returned when no limit is given
    const DEFAULT_LIMIT: i64 = 20;
    /// Maximum number of children returned
    const MAX_LIMIT: i64 = 100;

    /// The stockmovements_by_to_warehouse of a warehouse, served on `GET /warehouse/{id}/stockmovements_by_to_warehouse`
    pub struct WarehouseStockmovementsByToWarehouse;

    #[async_trait]
    impl HasMany for WarehouseStockmovementsByToWarehouse {
        type Parent = Warehouse;
        type Id = Id;
        type Query = WarehouseStockmovementsByToWarehouseQuery;
        type Result = Vec<Stockmovement>;
        type State = AppState;
        const RELATION: &'static str = "stockmovements_by_to_warehouse";

        async fn list_related(id: Id, query: &WarehouseStockmovementsByToWarehouseQuery, state: &AppState) -> Result<Option<Vec<Stockmovement>>> {
            let offset = query.offset.unwrap_or(0) as i64;
            let limit = query.limit.map_or(DEFAULT_LIMIT, |l| (l as i64).min(MAX_LIMIT));
            let models = sqlx::query_as::<_, Stockmovement>(
                "SELECT * FROM stockmovement WHERE to_warehouse_id = $1 ORDER BY id LIMIT $2 OFFSET $3",
            )
            .bind(id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&state.pool)
            .await?;
            // the parent is only looked up when the page is empty
            if models.is_empty() {
                let parent = sqlx::query_scalar::<_, Id>(
                    "SELECT id FROM warehouse WHERE id = $1",
                )
                .bind(id)
                .fetch_optional(&state.pool)
                .await?;
                if parent.is_none() {
                    return Ok(None);
                }
            }
            Ok(Some(models))
        }
    }

    // Registers the route of the stockmovements_by_to_warehouse of a warehouse, to mount with `.configure(warehouse_stockmovements_by_to_warehouse::configure)`
    // in the same scope as the warehouse routes
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_relation_endpoint!(WarehouseStockmovementsByToWarehouse)(cfg)
    }

    // Resolves the `stockmovements_by_to_warehouse` field of the GraphQL Warehouse type, declared in the `#[ComplexObject]` of its model
    pub async fn resolve(ctx: &async_graphql::Context<'_>, id: Id, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Stockmovement>> {
        let state = ctx.data::<actix_web::web::Data<AppState>>()?;
        let children = WarehouseStockmovementsByToWarehouse::list_related(id, &WarehouseStockmovementsByToWarehouseQuery { offset, limit }, state.get_ref()).await?;
        // the parent was resolved, so it exists
        Ok(children.unwrap_or_default())
    }
    