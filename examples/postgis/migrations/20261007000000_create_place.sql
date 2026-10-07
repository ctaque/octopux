CREATE EXTENSION IF NOT EXISTS postgis;

CREATE TABLE place (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    location geometry(Point, 4326) NOT NULL,
    area geometry(Polygon, 4326),
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);

-- Speeds up ST_DWithin on the geography of the location, used by GET /place/{id}/nearby
CREATE INDEX place_location_geography_idx ON place USING GIST ((location::geography));
