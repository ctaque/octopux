-- The sparse vectors are written with their indices from 1, as in SQL: {1:1,7:1}/8 is pasta and recipe
INSERT INTO article (title, category, embedding, keywords, created_at, updated_at) VALUES
    ('Carbonara in ten minutes', 'cooking', '[0.95,0.05,0.1,0.1]', '{1:1,7:1}/8', NOW(), NOW()),
    ('Ramen in Tokyo', 'travel', '[0.7,0,0.05,0.7]', '{5:1,7:0.5}/8', NOW(), NOW()),
    ('Trail running in the Alps', 'sport', '[0,0.9,0.05,0.5]', '{6:1}/8', NOW(), NOW()),
    ('Postgres as a vector database', 'tech', '[0,0,1,0.05]', '{4:1,8:1}/8', NOW(), NOW()),
    ('Writing an API in Rust', 'tech', '[0,0.05,0.95,0]', '{3:1,4:0.5}/8', NOW(), NOW()),
    ('The World Cup final', 'sport', '[0,1,0.1,0.1]', '{2:1}/8', NOW(), NOW()),
    ('Hiking in the Japanese Alps', 'travel', '[0.1,0.6,0,0.8]', '{5:1,6:1}/8', NOW(), NOW()),
    ('Fine-tuning a language model', 'tech', '[0,0,0.9,0.1]', '{8:1}/8', NOW(), NOW());
