use async_graphql::{ComplexObject, Context, EmptySubscription, Object, Schema};
use octopux::{HasMany, Model, NewModel, UpdatableModel};

use crate::recipe::{DeleteQuery, FindQuery, Id, ListQuery, NewRecipe, Recipe, SaveQuery, UpdatableRecipe, UpdateQuery};
use crate::recipe_search::{search_recipes, SearchQuery};
use crate::recipe_similar::{RecipeSimilar, RecipeSimilarQuery, SimilarRecipe};
use crate::shared::AppState;

// The GraphQL schema of the recipes, which never takes nor returns a vector
pub type RecipeSchema = Schema<RecipeQuery, RecipeMutation, EmptySubscription>;

// GraphQL queries of the recipes, the same as the REST routes
#[derive(Default)]
pub struct RecipeQuery;

#[Object]
impl RecipeQuery {
    async fn recipe(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Recipe> {
        find(id, app_state(ctx)?).await
    }

    /// A page of recipes, filtered and ordered as `GET /v1/recipe`
    #[allow(clippy::too_many_arguments)]
    async fn recipes(
        &self,
        ctx: &Context<'_>,
        offset: Option<usize>,
        limit: Option<usize>,
        cuisine: Option<String>,
        course: Option<String>,
        vegetarian: Option<bool>,
        minutes_lte: Option<i32>,
        sort: Option<String>,
    ) -> async_graphql::Result<Vec<Recipe>> {
        let query = ListQuery { offset, limit, cuisine, course, vegetarian, minutes_lte, near: None, sort };
        Ok(Recipe::list(&query, app_state(ctx)?).await?)
    }

    /// Searches the recipes by meaning, as `GET /v1/recipe/search`: `q` is embedded by the model
    /// of the recipes, the nearest recipes coming first
    #[allow(clippy::too_many_arguments)]
    async fn search_recipes(
        &self,
        ctx: &Context<'_>,
        q: String,
        offset: Option<usize>,
        limit: Option<usize>,
        cuisine: Option<String>,
        course: Option<String>,
        vegetarian: Option<bool>,
        minutes_lte: Option<i32>,
    ) -> async_graphql::Result<Vec<Recipe>> {
        let query = SearchQuery { q, offset, limit, cuisine, course, vegetarian, minutes_lte };
        Ok(search_recipes(query, app_state(ctx)?).await?)
    }
}

// The recipes similar to a recipe, a field of `Recipe` as `GET /v1/recipe/{id}/similar`
#[ComplexObject]
impl Recipe {
    /// The other recipes, the most similar first, with their cosine similarity
    async fn similar(
        &self,
        ctx: &Context<'_>,
        min_similarity: Option<f64>,
        other_cuisine: Option<bool>,
        limit: Option<usize>,
    ) -> async_graphql::Result<Vec<SimilarRecipe>> {
        let query = RecipeSimilarQuery { min_similarity, other_cuisine, limit };
        // `None` for a recipe without embedding yet: no similar recipe
        Ok(RecipeSimilar::list_related(self.id, &query, app_state(ctx)?).await?.unwrap_or_default())
    }
}

// GraphQL mutations of the recipes: the input has no embedding, `before_save` computes it
#[derive(Default)]
pub struct RecipeMutation;

#[Object]
impl RecipeMutation {
    async fn create_recipe(&self, ctx: &Context<'_>, input: NewRecipe) -> async_graphql::Result<Recipe> {
        Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
    }

    async fn update_recipe(&self, ctx: &Context<'_>, input: UpdatableRecipe) -> async_graphql::Result<Recipe> {
        let state = app_state(ctx)?;
        let id = input.id;
        find(id, state).await?;
        input.update(&UpdateQuery {}, state).await?;
        find(id, state).await
    }

    async fn delete_recipe(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Recipe> {
        let state = app_state(ctx)?;
        let model = find(id, state).await?;
        Ok(model.delete(&DeleteQuery {}, state).await?)
    }
}

// Builds the schema, the resolvers reading the state shared with the REST routes from its data
pub fn schema(state: actix_web::web::Data<AppState>) -> RecipeSchema {
    Schema::build(RecipeQuery, RecipeMutation, EmptySubscription).data(state).finish()
}

// Looks the recipe up, any error being ENTITY_NOT_FOUND as for the REST routes
async fn find(id: Id, state: &AppState) -> async_graphql::Result<Recipe> {
    match Recipe::find(id, &FindQuery::default(), state).await {
        Ok(model) => Ok(*model),
        Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
    }
}

// The state given to the schema with `Schema::build(...).data(state)`
fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
    Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
}
