-- Speed up the spatial filters of GET /place: ST_Intersects on the location (?bbox=)
-- and ST_Contains on the area (?point=)
CREATE INDEX place_location_idx ON place USING GIST (location);
CREATE INDEX place_area_idx ON place USING GIST (area);
