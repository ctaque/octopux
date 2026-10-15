-- The recipes are now embedded by multilingual E5 small, which keeps 384 dimensions but places the
-- texts elsewhere: the embeddings of BGE are dropped, and the app computes the new ones at its start
UPDATE recipe SET embedding = NULL;
