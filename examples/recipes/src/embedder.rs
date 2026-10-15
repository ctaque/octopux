use std::sync::{Arc, Mutex};

use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use octopux::anyhow::{anyhow, Result};
use octopux::pgvector::Vector;
use sqlx::PgPool;

/// E5 was trained with a prefix on each text: `query: ` before a search, `passage: ` before a
/// document it should find. Without them, the similarities are lower and the order worse
const QUERY_PREFIX: &str = "query: ";
const PASSAGE_PREFIX: &str = "passage: ";
/// Recipes embedded at once at the start
const BATCH_SIZE: usize = 64;

/// The embedding model, multilingual E5 small (384 dimensions), run locally on the CPU. It maps a
/// text and its translation close to each other, so that a search in French, "une soupe épicée",
/// finds the recipes written in English, "spicy soup".
/// `TextEmbedding::embed` takes `&mut self`, so the workers share it behind a mutex, and it runs
/// on the blocking threads of the runtime, a request computing its embedding in a few milliseconds.
#[derive(Clone)]
pub struct Embedder(Arc<Mutex<TextEmbedding>>);

impl Embedder {
    /// Loads the model, downloaded from Hugging Face on the first run into `.fastembed_cache`
    pub fn load() -> Result<Self> {
        let model = TextEmbedding::try_new(TextInitOptions::new(EmbeddingModel::MultilingualE5Small).with_show_download_progress(true))?;
        Ok(Embedder(Arc::new(Mutex::new(model))))
    }

    /// The embeddings of the texts, as the vectors of pgvector
    pub async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vector>> {
        let model = self.0.clone();
        let embeddings = actix_web::rt::task::spawn_blocking(move || {
            let mut model = model.lock().map_err(|_| anyhow!("the embedding model is poisoned"))?;
            Ok::<_, octopux::anyhow::Error>(model.embed(texts, Some(BATCH_SIZE))?)
        })
        .await??;
        Ok(embeddings.into_iter().map(Vector::new).collect())
    }

    /// The embedding of a search, closest to the embeddings of the recipes answering it
    pub async fn embed_query(&self, query: &str) -> Result<Vector> {
        let mut vectors = self.embed(vec![format!("{}{}", QUERY_PREFIX, query)]).await?;
        vectors.pop().ok_or_else(|| anyhow!("the model returned no embedding"))
    }
}

/// The text embedded for a recipe, as a passage: its name, its cuisine and course, then its
/// description, so that a search for "a japanese soup", or "une soupe japonaise", finds the
/// japanese soups
pub fn recipe_text(name: &str, cuisine: &str, course: &str, vegetarian: bool, description: &str) -> String {
    let diet = if vegetarian { "vegetarian " } else { "" };
    format!("{}{}, a {}{} {}. {}", PASSAGE_PREFIX, name, diet, cuisine, course, description)
}

/// Computes the embeddings the recipes lack, those of the seed migration, by batches.
/// Returns the number of recipes embedded.
pub async fn embed_missing(pool: &PgPool, embedder: &Embedder) -> Result<usize> {
    let mut count = 0;
    loop {
        let recipes: Vec<(i64, String, String, String, bool, String)> = sqlx::query_as(
            "SELECT id, name, cuisine, course, vegetarian, description FROM recipe WHERE embedding IS NULL ORDER BY id LIMIT $1",
        )
        .bind(BATCH_SIZE as i64)
        .fetch_all(pool)
        .await?;
        if recipes.is_empty() {
            return Ok(count);
        }
        let texts = recipes.iter().map(|(_, name, cuisine, course, vegetarian, description)| recipe_text(name, cuisine, course, *vegetarian, description)).collect();
        let embeddings = embedder.embed(texts).await?;
        // one UPDATE for the batch, the ids and the vectors unnested side by side
        let ids: Vec<i64> = recipes.iter().map(|recipe| recipe.0).collect();
        sqlx::query(
            "UPDATE recipe SET embedding = batch.embedding
             FROM UNNEST($1::int8[], $2::vector[]) AS batch(id, embedding)
             WHERE recipe.id = batch.id",
        )
        .bind(&ids)
        .bind(&embeddings)
        .execute(pool)
        .await?;
        count += recipes.len();
    }
}
