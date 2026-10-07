CREATE EXTENSION IF NOT EXISTS vector;

-- `embedding` places an article on 4 axes, [cooking, sport, tech, travel], as an embedding model
-- would on hundreds of them; `keywords` holds the weights of the words of a vocabulary of 8:
-- pasta, football, rust, database, japan, mountain, recipe, ai
CREATE TABLE article (
    id BIGSERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    category TEXT NOT NULL,
    embedding vector(4) NOT NULL,
    keywords sparsevec(8),
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);

-- Finds the nearest articles by cosine distance, the default of the `nearest` filter,
-- without reading the whole table
CREATE INDEX article_embedding_idx ON article USING hnsw (embedding vector_cosine_ops);
