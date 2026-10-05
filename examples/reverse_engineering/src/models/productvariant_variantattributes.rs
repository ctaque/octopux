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
    use crate::productvariant::{Productvariant, Id};
    use crate::variantattribute::Variantattribute;
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
    pub struct ProductvariantVariantattributesQuery {
        /// Number of rows to skip
        pub offset: Option<usize>,
        /// Maximum number of rows to return (20 by default, 100 at most)
        pub limit: Option<usize>,
    }
    /// Number of children returned when no limit is given
    const DEFAULT_LIMIT: i64 = 20;
    /// Maximum number of children returned
    const MAX_LIMIT: i64 = 100;

    /// The variantattributes of a productvariant, served on `GET /productvariant/{id}/variantattributes`
    pub struct ProductvariantVariantattributes;

    #[async_trait]
    impl HasMany for ProductvariantVariantattributes {
        type Parent = Productvariant;
        type Id = Id;
        type Query = ProductvariantVariantattributesQuery;
        type Result = Vec<Variantattribute>;
        type State = AppState;
        const RELATION: &'static str = "variantattributes";

        async fn list_related(id: Id, query: &ProductvariantVariantattributesQuery, state: &AppState) -> Result<Option<Vec<Variantattribute>>> {
            let offset = query.offset.unwrap_or(0) as i64;
            let limit = query.limit.map_or(DEFAULT_LIMIT, |l| (l as i64).min(MAX_LIMIT));
            let models = sqlx::query_as::<_, Variantattribute>(
                "SELECT * FROM variantattribute WHERE product_variant_id = $1 ORDER BY id LIMIT $2 OFFSET $3",
            )
            .bind(id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&state.pool)
            .await?;
            // the parent is only looked up when the page is empty
            if models.is_empty() {
                let parent = sqlx::query_scalar::<_, Id>(
                    "SELECT id FROM productvariant WHERE id = $1",
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

    // Registers the route of the variantattributes of a productvariant, to mount with `.configure(productvariant_variantattributes::configure)`
    // in the same scope as the productvariant routes
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_relation_endpoint!(ProductvariantVariantattributes)(cfg)
    }

    // Resolves the `variantattributes` field of the GraphQL Productvariant type, declared in the `#[ComplexObject]` of its model
    pub async fn resolve(ctx: &async_graphql::Context<'_>, id: Id, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Variantattribute>> {
        let state = ctx.data::<actix_web::web::Data<AppState>>()?;
        let children = ProductvariantVariantattributes::list_related(id, &ProductvariantVariantattributesQuery { offset, limit }, state.get_ref()).await?;
        // the parent was resolved, so it exists
        Ok(children.unwrap_or_default())
    }
    