CREATE EXTENSION IF NOT EXISTS vector;

-- `embedding` is the embedding of the name and of the description of the recipe, computed by the
-- app with the BGE small model (384 dimensions): NULL until the app computes it, at its start for
-- the seeded recipes, before the insert or the update for the others
CREATE TABLE recipe (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    cuisine TEXT NOT NULL,
    course TEXT NOT NULL,
    vegetarian BOOLEAN NOT NULL,
    minutes INTEGER NOT NULL,
    description TEXT NOT NULL,
    embedding vector(384),
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);

-- Finds the nearest recipes by cosine distance, the default of the `nearest` filter,
-- without reading the whole table
CREATE INDEX recipe_embedding_idx ON recipe USING hnsw (embedding vector_cosine_ops);
