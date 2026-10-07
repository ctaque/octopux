INSERT INTO place (name, location, area, created_at, updated_at) VALUES
    ('Tour Eiffel', ST_SetSRID(ST_MakePoint(2.2945, 48.8584), 4326),
        ST_GeomFromText('POLYGON((2.2932 48.8578, 2.2952 48.8571, 2.2959 48.8590, 2.2939 48.8597, 2.2932 48.8578))', 4326),
        NOW(), NOW()),
    ('Champ-de-Mars', ST_SetSRID(ST_MakePoint(2.2986, 48.8556), 4326), NULL, NOW(), NOW()),
    ('Trocadéro', ST_SetSRID(ST_MakePoint(2.2876, 48.8616), 4326), NULL, NOW(), NOW()),
    ('Arc de Triomphe', ST_SetSRID(ST_MakePoint(2.2950, 48.8738), 4326), NULL, NOW(), NOW()),
    ('Louvre', ST_SetSRID(ST_MakePoint(2.3376, 48.8606), 4326), NULL, NOW(), NOW()),
    ('Notre-Dame', ST_SetSRID(ST_MakePoint(2.3499, 48.8530), 4326), NULL, NOW(), NOW());
