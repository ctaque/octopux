--                         -----
--                     -------------
--                   -----  ----------
--                  ---  --------------
--                 ---  ----------------
--                 --- -----------------
--                 --- -----------------
--                 --- -----------------
--                 ---------------------
--       -----      -------------------       -----
--      -------      --  ---------- --      -------
--          ----      ---------------      -----
--           ---      ---------------      ----
--          ----     -----------------     ----
--        ------   ----------------------   ------
--    --------  ---------------------------  --------
--   ------   -------------------------- ----   -------
--  ----    -----  ---------- --- ------- -----    -----
-- ----  ------  -------- --- --- ---- ---  ------  ----
-- ----        ---- ----  --- ---- ---- -----       ----
--  ----   ------  ----  ---- ----  ----   ------  -----
--  ------      ------   ---- -----  ------      ------
--    ---------------    ----  ----    --------------
--      ----------       ----  ----      ----------
--                       ----  ----
--                 ---   ---- -----   --
--               ------  ---- ----- -------
--              -------  ---- ----- --------
--              ----    ----   -----    ----
--              -----------     -----------
--               ---------        --------

CREATE EXTENSION IF NOT EXISTS postgis;
ALTER TABLE place ADD COLUMN area_2 geometry(Polygon, 4326);
ALTER TABLE place ADD COLUMN point2 geometry(Point, 4326);
ALTER TABLE place ADD COLUMN multipoint geometry(MultiPoint, 4326);
ALTER TABLE place ADD COLUMN multipolygon geometry(MultiPolygon, 4326);
CREATE INDEX IF NOT EXISTS place_area_2_idx ON place USING GIST (area_2);
CREATE INDEX IF NOT EXISTS place_point2_idx ON place USING GIST (point2);
CREATE INDEX IF NOT EXISTS place_multipoint_idx ON place USING GIST (multipoint);
CREATE INDEX IF NOT EXISTS place_multipolygon_idx ON place USING GIST (multipolygon);
