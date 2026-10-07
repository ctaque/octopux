//! The PostGIS `geometry` and `geography` columns (requires the `postgis` feature).
//!
//! [`Geometry`] is a field type of the models deriving the sqlx derives on PostgreSQL: it is read
//! from and written to the database as EWKB, the binary format of PostGIS, with its SRID, and
//! sent to and received from the client as a GeoJSON geometry (RFC 7946).
//!
//! `Geometry<G>` holds a [`geo_types`] geometry, any geometry by default. The aliases [`Point`],
//! [`LineString`], [`Polygon`]... restrict a field to one kind of geometry, the other kinds
//! failing to decode from the database and to deserialize from the payload.
//!
//! ```ignore
//!
//! use octopux::postgis::{self, Point};
//!
//! #[derive(Serialize, Deserialize, sqlx::FromRow, HttpFindListDelete, SqlxModel)]
//! #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
//! #[sqlx_model(database = "postgres")]
//! #[octopux_info(path = "place")]
//! struct Place {
//!     id: Id,
//!     name: String,
//!     location: Point,                          // geometry(Point, 4326) NOT NULL
//!     area: Option<postgis::Geometry>,          // geography
//! }
//! ```
//!
//! `location` is sent as `{"type": "Point", "coordinates": [2.3522, 48.8566]}`.
//!
//! GeoJSON has no SRID, its coordinates are WGS 84 longitudes and latitudes: a deserialized
//! geometry gets the SRID [`WGS84`], as does a geometry built with `From`. A column of another
//! SRID needs the geometry built with [`Geometry::new`].
//!
//! The geometries are two-dimensional: a geometry with a Z or M coordinate fails to decode, as
//! does an empty point, which [`geo_types::Point`] cannot represent.
//!
//! # GraphQL
//!
//! With the `graphql` feature, the geometries are async-graphql scalars, GeoJSON as in the REST
//! routes, so that the models deriving `SimpleObject` and `InputObject` can hold them. Each kind
//! is its own scalar: `GeoJsonPoint`, `GeoJsonPolygon`..., `GeoJsonGeometry` for any kind. An input
//! is a GeoJSON object or its string.
//!
//! # Spatial filters
//!
//! The `SqlxFilter` derive filters a list with the spatial operators of PostGIS, given by `op`:
//!
//! ```ignore
//!
//! #[derive(Deserialize, SqlxFilter)]
//! #[sqlx_filter(database = "postgres")]
//! struct ListQuery {
//!     offset: Option<usize>,
//!     limit: Option<usize>,
//!     #[sqlx_filter(column = "location", op = "intersects")]
//!     bbox: Option<postgis::Bbox>,          // ?bbox=2.25,48.81,2.42,48.90
//!     #[sqlx_filter(column = "location", op = "dwithin")]
//!     near: Option<postgis::Near>,          // ?near=2.3522,48.8566,1000
//!     #[sqlx_filter(column = "zone", op = "contains")]
//!     point: Option<postgis::Point>,        // ?point={"type":"Point","coordinates":[2.35,48.85]}
//! }
//! ```
//!
//! - `intersects`, `within` and `contains` filter `ST_Intersects(column, value)`,
//!   `ST_Within(column, value)` (the column inside the value) and `ST_Contains(column, value)`,
//!   the value being a [`Bbox`] or a [`Geometry`], given as GeoJSON in the query string.
//!   `within` and `contains` have no `geography` version in PostGIS, they need `geometry` columns
//! - `dwithin` keeps the rows within a distance in meters of a point, a [`Near`]:
//!   `ST_DWithin(column::geography, point::geography, distance)`. The cast of a `geometry` column
//!   needs it in WGS 84, and an index on `(column::geography)` to be fast
//!
//! [`Bbox`] and [`Near`] are in WGS 84, they need the column in WGS 84 too.

use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

use geo_types::Coord;
use serde::de::{self, Deserializer, SeqAccess, Visitor};
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::postgres::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueFormat, PgValueRef, Postgres};

pub use geo_types;

/// The SRID of WGS 84, the longitudes and latitudes of GPS and GeoJSON
pub const WGS84: i32 = 4326;

/// A PostGIS `geometry` or `geography` value with its SRID, see the [module](self).
///
/// It dereferences to its geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct Geometry<G = geo_types::Geometry<f64>> {
    pub geometry: G,
    /// `0` when the geometry has no SRID
    pub srid: i32,
}

pub type Point = Geometry<geo_types::Point<f64>>;
pub type LineString = Geometry<geo_types::LineString<f64>>;
pub type Polygon = Geometry<geo_types::Polygon<f64>>;
pub type MultiPoint = Geometry<geo_types::MultiPoint<f64>>;
pub type MultiLineString = Geometry<geo_types::MultiLineString<f64>>;
pub type MultiPolygon = Geometry<geo_types::MultiPolygon<f64>>;
pub type GeometryCollection = Geometry<geo_types::GeometryCollection<f64>>;

impl<G> Geometry<G> {
    pub fn new(geometry: G, srid: i32) -> Self {
        Geometry { geometry, srid }
    }

    pub fn into_inner(self) -> G {
        self.geometry
    }
}

/// The geometry in [`WGS84`]
impl<G> From<G> for Geometry<G> {
    fn from(geometry: G) -> Self {
        Geometry::new(geometry, WGS84)
    }
}

/// The empty geometry of the kind in [`WGS84`], the point `(0, 0)` for a [`Point`], so that the
/// models holding geometries can derive `Default`
impl<G: GeometryType> Default for Geometry<G> {
    fn default() -> Self {
        G::empty().into()
    }
}

impl<G> Deref for Geometry<G> {
    type Target = G;

    fn deref(&self) -> &G {
        &self.geometry
    }
}

impl<G> DerefMut for Geometry<G> {
    fn deref_mut(&mut self) -> &mut G {
        &mut self.geometry
    }
}

/// The [`geo_types`] geometries a [`Geometry`] can hold: [`geo_types::Geometry`] or one of the
/// seven kinds of the OGC, `Point`, `LineString`, `Polygon`, `MultiPoint`, `MultiLineString`,
/// `MultiPolygon` and `GeometryCollection`.
pub trait GeometryType: sealed::Sealed {}

mod sealed {
    use geo_types as g;

    // A borrowed geometry of any kind, written to EWKB and GeoJSON
    #[derive(Clone, Copy)]
    pub enum GeometryRef<'a> {
        Point(&'a g::Point<f64>),
        Line(&'a g::Line<f64>),
        LineString(&'a g::LineString<f64>),
        Polygon(&'a g::Polygon<f64>),
        MultiPoint(&'a g::MultiPoint<f64>),
        MultiLineString(&'a g::MultiLineString<f64>),
        MultiPolygon(&'a g::MultiPolygon<f64>),
        GeometryCollection(&'a g::GeometryCollection<f64>),
        Rect(&'a g::Rect<f64>),
        Triangle(&'a g::Triangle<f64>),
    }

    impl<'a> From<&'a g::Geometry<f64>> for GeometryRef<'a> {
        fn from(geometry: &'a g::Geometry<f64>) -> Self {
            match geometry {
                g::Geometry::Point(g) => GeometryRef::Point(g),
                g::Geometry::Line(g) => GeometryRef::Line(g),
                g::Geometry::LineString(g) => GeometryRef::LineString(g),
                g::Geometry::Polygon(g) => GeometryRef::Polygon(g),
                g::Geometry::MultiPoint(g) => GeometryRef::MultiPoint(g),
                g::Geometry::MultiLineString(g) => GeometryRef::MultiLineString(g),
                g::Geometry::MultiPolygon(g) => GeometryRef::MultiPolygon(g),
                g::Geometry::GeometryCollection(g) => GeometryRef::GeometryCollection(g),
                g::Geometry::Rect(g) => GeometryRef::Rect(g),
                g::Geometry::Triangle(g) => GeometryRef::Triangle(g),
            }
        }
    }

    // OGC name of a geometry
    pub fn kind(geometry: &g::Geometry<f64>) -> &'static str {
        match geometry {
            g::Geometry::Point(_) => "Point",
            g::Geometry::Line(_) | g::Geometry::LineString(_) => "LineString",
            g::Geometry::Polygon(_) | g::Geometry::Rect(_) | g::Geometry::Triangle(_) => "Polygon",
            g::Geometry::MultiPoint(_) => "MultiPoint",
            g::Geometry::MultiLineString(_) => "MultiLineString",
            g::Geometry::MultiPolygon(_) => "MultiPolygon",
            g::Geometry::GeometryCollection(_) => "GeometryCollection",
        }
    }

    pub trait Sealed: Sized {
        // OGC name of the kind, `Geometry` for any kind
        const KIND: &'static str;
        fn geometry_ref(&self) -> GeometryRef<'_>;
        // fails with the kind of the geometry when it is not of this kind
        fn from_geometry(geometry: g::Geometry<f64>) -> Result<Self, String>;
        // the empty geometry of the kind, the point (0, 0) which cannot be empty
        fn empty() -> Self;
    }

    impl Sealed for g::Geometry<f64> {
        const KIND: &'static str = "Geometry";
        fn geometry_ref(&self) -> GeometryRef<'_> {
            self.into()
        }
        fn from_geometry(geometry: g::Geometry<f64>) -> Result<Self, String> {
            Ok(geometry)
        }
        fn empty() -> Self {
            g::Geometry::GeometryCollection(g::GeometryCollection(Vec::new()))
        }
    }

    macro_rules! kinds {
        ($($kind:ident => $empty:expr),+ $(,)?) => {$(
            impl Sealed for g::$kind<f64> {
                const KIND: &'static str = stringify!($kind);
                fn geometry_ref(&self) -> GeometryRef<'_> {
                    GeometryRef::$kind(self)
                }
                fn from_geometry(geometry: g::Geometry<f64>) -> Result<Self, String> {
                    match geometry {
                        g::Geometry::$kind(g) => Ok(g),
                        other => Err(format!("expected a {}, found a {}", stringify!($kind), kind(&other))),
                    }
                }
                fn empty() -> Self {
                    $empty
                }
            }
            impl super::GeometryType for g::$kind<f64> {}
        )+};
    }

    kinds!(
        Point => g::Point::new(0.0, 0.0),
        LineString => g::LineString(Vec::new()),
        Polygon => g::Polygon::new(g::LineString(Vec::new()), Vec::new()),
        MultiPoint => g::MultiPoint(Vec::new()),
        MultiLineString => g::MultiLineString(Vec::new()),
        MultiPolygon => g::MultiPolygon(Vec::new()),
        GeometryCollection => g::GeometryCollection(Vec::new()),
    );
}

impl GeometryType for geo_types::Geometry<f64> {}

use sealed::GeometryRef;

/// EWKB, the WKB of PostGIS: a byte order, a geometry type with the flags of the dimensions and
/// of the SRID, the SRID when it is set (only on the outermost geometry), then the coordinates.
mod ewkb {
    use super::sealed::GeometryRef;
    use geo_types::{self as g, Coord};
    use sqlx::error::BoxDynError;

    const Z_FLAG: u32 = 0x8000_0000;
    const M_FLAG: u32 = 0x4000_0000;
    const SRID_FLAG: u32 = 0x2000_0000;

    const POINT: u32 = 1;
    const LINE_STRING: u32 = 2;
    const POLYGON: u32 = 3;
    const MULTI_POINT: u32 = 4;
    const MULTI_LINE_STRING: u32 = 5;
    const MULTI_POLYGON: u32 = 6;
    const GEOMETRY_COLLECTION: u32 = 7;

    // Little endian, the SRID written when it is not 0
    pub fn write(buf: &mut Vec<u8>, geometry: GeometryRef<'_>, srid: i32) {
        let header = |buf: &mut Vec<u8>, kind: u32| {
            buf.push(1);
            if srid == 0 {
                buf.extend_from_slice(&kind.to_le_bytes());
            } else {
                buf.extend_from_slice(&(kind | SRID_FLAG).to_le_bytes());
                buf.extend_from_slice(&srid.to_le_bytes());
            }
        };
        match geometry {
            GeometryRef::Point(p) => {
                header(buf, POINT);
                coord(buf, p.0);
            }
            GeometryRef::Line(l) => {
                header(buf, LINE_STRING);
                coords(buf, &[l.start, l.end]);
            }
            GeometryRef::LineString(ls) => {
                header(buf, LINE_STRING);
                coords(buf, &ls.0);
            }
            GeometryRef::Polygon(p) => {
                header(buf, POLYGON);
                rings(buf, p);
            }
            GeometryRef::Rect(r) => write(buf, GeometryRef::Polygon(&r.to_polygon()), srid),
            GeometryRef::Triangle(t) => write(buf, GeometryRef::Polygon(&t.to_polygon()), srid),
            GeometryRef::MultiPoint(mp) => {
                header(buf, MULTI_POINT);
                count(buf, mp.0.len());
                mp.0.iter().for_each(|p| write(buf, GeometryRef::Point(p), 0));
            }
            GeometryRef::MultiLineString(mls) => {
                header(buf, MULTI_LINE_STRING);
                count(buf, mls.0.len());
                mls.0.iter().for_each(|ls| write(buf, GeometryRef::LineString(ls), 0));
            }
            GeometryRef::MultiPolygon(mp) => {
                header(buf, MULTI_POLYGON);
                count(buf, mp.0.len());
                mp.0.iter().for_each(|p| write(buf, GeometryRef::Polygon(p), 0));
            }
            GeometryRef::GeometryCollection(gc) => {
                header(buf, GEOMETRY_COLLECTION);
                count(buf, gc.0.len());
                gc.0.iter().for_each(|g| write(buf, g.into(), 0));
            }
        }
    }

    fn count(buf: &mut Vec<u8>, n: usize) {
        buf.extend_from_slice(&(n as u32).to_le_bytes());
    }

    fn coord(buf: &mut Vec<u8>, c: Coord<f64>) {
        buf.extend_from_slice(&c.x.to_le_bytes());
        buf.extend_from_slice(&c.y.to_le_bytes());
    }

    fn coords(buf: &mut Vec<u8>, cs: &[Coord<f64>]) {
        count(buf, cs.len());
        cs.iter().for_each(|c| coord(buf, *c));
    }

    // The empty polygon has no ring, not an empty exterior ring
    fn rings(buf: &mut Vec<u8>, p: &g::Polygon<f64>) {
        if p.exterior().0.is_empty() && p.interiors().is_empty() {
            count(buf, 0);
        } else {
            count(buf, 1 + p.interiors().len());
            coords(buf, &p.exterior().0);
            p.interiors().iter().for_each(|ring| coords(buf, &ring.0));
        }
    }

    /// The geometry and its SRID, `0` without one
    pub fn read(bytes: &[u8]) -> Result<(g::Geometry<f64>, i32), BoxDynError> {
        let mut reader = Reader { bytes, little_endian: true };
        let (geometry, srid) = reader.geometry()?;
        if !reader.bytes.is_empty() {
            return Err(format!("{} unexpected bytes after the EWKB geometry", reader.bytes.len()).into());
        }
        Ok((geometry, srid.unwrap_or(0)))
    }

    struct Reader<'a> {
        bytes: &'a [u8],
        little_endian: bool,
    }

    impl Reader<'_> {
        fn take<const N: usize>(&mut self) -> Result<[u8; N], BoxDynError> {
            let (head, rest) = self.bytes.split_first_chunk::<N>().ok_or("truncated EWKB geometry")?;
            self.bytes = rest;
            Ok(*head)
        }

        fn u32(&mut self) -> Result<u32, BoxDynError> {
            let b = self.take()?;
            Ok(if self.little_endian { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) })
        }

        fn f64(&mut self) -> Result<f64, BoxDynError> {
            let b = self.take()?;
            Ok(if self.little_endian { f64::from_le_bytes(b) } else { f64::from_be_bytes(b) })
        }

        // The number of elements of a list, each taking at least `min_size` bytes, checked against
        // the bytes left so that a corrupted count does not allocate a huge vector
        fn count(&mut self, min_size: usize) -> Result<usize, BoxDynError> {
            let n = self.u32()? as usize;
            if n.saturating_mul(min_size) > self.bytes.len() {
                return Err("truncated EWKB geometry".into());
            }
            Ok(n)
        }

        fn coord(&mut self) -> Result<Coord<f64>, BoxDynError> {
            Ok(Coord { x: self.f64()?, y: self.f64()? })
        }

        fn line_string(&mut self) -> Result<g::LineString<f64>, BoxDynError> {
            let n = self.count(16)?;
            (0..n).map(|_| self.coord()).collect::<Result<_, _>>().map(g::LineString)
        }

        fn polygon(&mut self) -> Result<g::Polygon<f64>, BoxDynError> {
            let n = self.count(4)?;
            let mut rings = (0..n).map(|_| self.line_string()).collect::<Result<Vec<_>, _>>()?.into_iter();
            let exterior = rings.next().unwrap_or_else(|| g::LineString(Vec::new()));
            Ok(g::Polygon::new(exterior, rings.collect()))
        }

        // A geometry of the kind `expected`, inside a multi geometry
        fn member<T>(&mut self, expected: u32, unwrap: fn(g::Geometry<f64>) -> Option<T>) -> Result<T, BoxDynError> {
            let (geometry, _) = self.geometry()?;
            let kind = super::sealed::kind(&geometry);
            unwrap(geometry).ok_or_else(|| format!("unexpected {} in a multi geometry of type {}", kind, expected).into())
        }

        fn geometry(&mut self) -> Result<(g::Geometry<f64>, Option<i32>), BoxDynError> {
            self.little_endian = match self.take::<1>()? {
                [0] => false,
                [1] => true,
                [b] => return Err(format!("invalid EWKB byte order {}", b).into()),
            };
            let header = self.u32()?;
            if header & (Z_FLAG | M_FLAG) != 0 {
                return Err("geometries with Z or M coordinates are not supported, only 2D ones".into());
            }
            let srid = if header & SRID_FLAG != 0 { Some(self.u32()? as i32) } else { None };
            let geometry = match header & !SRID_FLAG {
                POINT => {
                    let c = self.coord()?;
                    if c.x.is_nan() && c.y.is_nan() {
                        return Err("empty points are not supported".into());
                    }
                    g::Geometry::Point(g::Point(c))
                }
                LINE_STRING => g::Geometry::LineString(self.line_string()?),
                POLYGON => g::Geometry::Polygon(self.polygon()?),
                MULTI_POINT => {
                    let n = self.count(5)?;
                    let points = (0..n)
                        .map(|_| self.member(MULTI_POINT, |g| g::Point::try_from(g).ok()))
                        .collect::<Result<_, _>>()?;
                    g::Geometry::MultiPoint(g::MultiPoint(points))
                }
                MULTI_LINE_STRING => {
                    let n = self.count(5)?;
                    let lines = (0..n)
                        .map(|_| self.member(MULTI_LINE_STRING, |g| g::LineString::try_from(g).ok()))
                        .collect::<Result<_, _>>()?;
                    g::Geometry::MultiLineString(g::MultiLineString(lines))
                }
                MULTI_POLYGON => {
                    let n = self.count(5)?;
                    let polygons = (0..n)
                        .map(|_| self.member(MULTI_POLYGON, |g| g::Polygon::try_from(g).ok()))
                        .collect::<Result<_, _>>()?;
                    g::Geometry::MultiPolygon(g::MultiPolygon(polygons))
                }
                GEOMETRY_COLLECTION => {
                    let n = self.count(5)?;
                    let geometries = (0..n).map(|_| self.geometry().map(|(g, _)| g)).collect::<Result<_, _>>()?;
                    g::Geometry::GeometryCollection(g::GeometryCollection(geometries))
                }
                // the ISO codes of the 3D geometries (1001...) included
                other => return Err(format!("unsupported EWKB geometry type {}", other).into()),
            };
            Ok((geometry, srid))
        }
    }
}

impl<G: GeometryType> sqlx::Type<Postgres> for Geometry<G> {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::with_name("geometry")
    }

    // `geography` is sent as EWKB too, and a `geometry` parameter is cast implicitly to `geography`
    fn compatible(ty: &PgTypeInfo) -> bool {
        *ty == PgTypeInfo::with_name("geometry") || *ty == PgTypeInfo::with_name("geography")
    }
}

impl<G: GeometryType> PgHasArrayType for Geometry<G> {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::array_of("geometry")
    }

    fn array_compatible(ty: &PgTypeInfo) -> bool {
        *ty == PgTypeInfo::array_of("geometry") || *ty == PgTypeInfo::array_of("geography")
    }
}

impl<G: GeometryType> sqlx::Encode<'_, Postgres> for Geometry<G> {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        ewkb::write(buf, self.geometry.geometry_ref(), self.srid);
        Ok(IsNull::No)
    }
}

impl<'r, G: GeometryType> sqlx::Decode<'r, Postgres> for Geometry<G> {
    // the text format of PostGIS is the EWKB in hexadecimal
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        let bytes = match value.format() {
            PgValueFormat::Binary => Cow::Borrowed(value.as_bytes()?),
            PgValueFormat::Text => Cow::Owned(from_hex(value.as_str()?)?),
        };
        let (geometry, srid) = ewkb::read(&bytes)?;
        Ok(Geometry { geometry: G::from_geometry(geometry)?, srid })
    }
}

fn from_hex(hex: &str) -> Result<Vec<u8>, BoxDynError> {
    let digit = |b: u8| (b as char).to_digit(16).ok_or("invalid hexadecimal EWKB");
    hex.as_bytes()
        .chunks(2)
        .map(|pair| match pair {
            [h, l] => Ok((digit(*h)? * 16 + digit(*l)?) as u8),
            _ => Err("invalid hexadecimal EWKB".into()),
        })
        .collect()
}

/// GeoJSON: `{"type": "Point", "coordinates": [x, y]}`, `{"type": "GeometryCollection", "geometries": [...]}`
mod geojson {
    use super::*;

    pub struct GeoJson<'a>(pub GeometryRef<'a>);

    struct Position(Coord<f64>);
    struct Positions<'a>(&'a [Coord<f64>]);
    struct Rings<'a>(&'a geo_types::Polygon<f64>);

    impl Serialize for Position {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            [self.0.x, self.0.y].serialize(s)
        }
    }

    impl Serialize for Positions<'_> {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.collect_seq(self.0.iter().map(|c| Position(*c)))
        }
    }

    // The empty polygon has no ring
    impl Serialize for Rings<'_> {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            let p = self.0;
            if p.exterior().0.is_empty() && p.interiors().is_empty() {
                return s.collect_seq(std::iter::empty::<Positions>());
            }
            s.collect_seq(std::iter::once(p.exterior()).chain(p.interiors()).map(|ring| Positions(&ring.0)))
        }
    }

    impl Serialize for GeoJson<'_> {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            let mut map = s.serialize_map(Some(2))?;
            match self.0 {
                GeometryRef::Point(p) => {
                    map.serialize_entry("type", "Point")?;
                    map.serialize_entry("coordinates", &Position(p.0))?;
                }
                GeometryRef::Line(l) => {
                    map.serialize_entry("type", "LineString")?;
                    map.serialize_entry("coordinates", &Positions(&[l.start, l.end]))?;
                }
                GeometryRef::LineString(ls) => {
                    map.serialize_entry("type", "LineString")?;
                    map.serialize_entry("coordinates", &Positions(&ls.0))?;
                }
                GeometryRef::Polygon(p) => {
                    map.serialize_entry("type", "Polygon")?;
                    map.serialize_entry("coordinates", &Rings(p))?;
                }
                GeometryRef::Rect(r) => {
                    map.serialize_entry("type", "Polygon")?;
                    map.serialize_entry("coordinates", &Rings(&r.to_polygon()))?;
                }
                GeometryRef::Triangle(t) => {
                    map.serialize_entry("type", "Polygon")?;
                    map.serialize_entry("coordinates", &Rings(&t.to_polygon()))?;
                }
                GeometryRef::MultiPoint(mp) => {
                    map.serialize_entry("type", "MultiPoint")?;
                    map.serialize_entry("coordinates", &SerializeSeq(|| mp.0.iter().map(|p| Position(p.0))))?;
                }
                GeometryRef::MultiLineString(mls) => {
                    map.serialize_entry("type", "MultiLineString")?;
                    map.serialize_entry("coordinates", &SerializeSeq(|| mls.0.iter().map(|ls| Positions(&ls.0))))?;
                }
                GeometryRef::MultiPolygon(mp) => {
                    map.serialize_entry("type", "MultiPolygon")?;
                    map.serialize_entry("coordinates", &SerializeSeq(|| mp.0.iter().map(Rings)))?;
                }
                GeometryRef::GeometryCollection(gc) => {
                    map.serialize_entry("type", "GeometryCollection")?;
                    map.serialize_entry("geometries", &SerializeSeq(|| gc.0.iter().map(|g| GeoJson(g.into()))))?;
                }
            }
            map.end()
        }
    }

    // A sequence serialized from the items of an iterator
    struct SerializeSeq<F>(F);

    impl<F, I> Serialize for SerializeSeq<F>
    where
        F: Fn() -> I,
        I: Iterator,
        I::Item: Serialize,
    {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.collect_seq((self.0)())
        }
    }

    /// A deserialized GeoJSON geometry, the other members of the object (`bbox`...) being ignored
    #[derive(Deserialize)]
    #[serde(tag = "type")]
    pub enum Owned {
        Point { coordinates: OwnedPosition },
        LineString { coordinates: Vec<OwnedPosition> },
        Polygon { coordinates: Vec<Vec<OwnedPosition>> },
        MultiPoint { coordinates: Vec<OwnedPosition> },
        MultiLineString { coordinates: Vec<Vec<OwnedPosition>> },
        MultiPolygon { coordinates: Vec<Vec<Vec<OwnedPosition>>> },
        GeometryCollection { geometries: Vec<Owned> },
    }

    /// `[x, y]`, a position with an altitude being refused as the geometries are 2D
    pub struct OwnedPosition(Coord<f64>);

    impl<'de> Deserialize<'de> for OwnedPosition {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct PositionVisitor;

            impl<'de> Visitor<'de> for PositionVisitor {
                type Value = OwnedPosition;

                fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    f.write_str("a GeoJSON position, [x, y]")
                }

                fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<OwnedPosition, A::Error> {
                    let x = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(0, &self))?;
                    let y = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(1, &self))?;
                    if seq.next_element::<de::IgnoredAny>()?.is_some() {
                        return Err(de::Error::custom("positions with an altitude are not supported, only [x, y]"));
                    }
                    Ok(OwnedPosition(Coord { x, y }))
                }
            }

            d.deserialize_seq(PositionVisitor)
        }
    }

    fn line_string(positions: Vec<OwnedPosition>) -> geo_types::LineString<f64> {
        geo_types::LineString(positions.into_iter().map(|p| p.0).collect())
    }

    fn polygon(rings: Vec<Vec<OwnedPosition>>) -> geo_types::Polygon<f64> {
        let mut rings = rings.into_iter().map(line_string);
        let exterior = rings.next().unwrap_or_else(|| geo_types::LineString(Vec::new()));
        geo_types::Polygon::new(exterior, rings.collect())
    }

    impl From<Owned> for geo_types::Geometry<f64> {
        fn from(geojson: Owned) -> Self {
            use geo_types as g;
            match geojson {
                Owned::Point { coordinates } => g::Geometry::Point(g::Point(coordinates.0)),
                Owned::LineString { coordinates } => g::Geometry::LineString(line_string(coordinates)),
                Owned::Polygon { coordinates } => g::Geometry::Polygon(polygon(coordinates)),
                Owned::MultiPoint { coordinates } => {
                    g::Geometry::MultiPoint(g::MultiPoint(coordinates.into_iter().map(|p| g::Point(p.0)).collect()))
                }
                Owned::MultiLineString { coordinates } => {
                    g::Geometry::MultiLineString(g::MultiLineString(coordinates.into_iter().map(line_string).collect()))
                }
                Owned::MultiPolygon { coordinates } => {
                    g::Geometry::MultiPolygon(g::MultiPolygon(coordinates.into_iter().map(polygon).collect()))
                }
                Owned::GeometryCollection { geometries } => {
                    g::Geometry::GeometryCollection(g::GeometryCollection(geometries.into_iter().map(Into::into).collect()))
                }
            }
        }
    }
}

/// The geometry as GeoJSON, without its SRID
impl<G: GeometryType> Serialize for Geometry<G> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        geojson::GeoJson(self.geometry.geometry_ref()).serialize(s)
    }
}

/// A GeoJSON geometry, in [`WGS84`], or the string of one, as in a query string
impl<'de, G: GeometryType> Deserialize<'de> for Geometry<G> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct GeoJsonVisitor;

        impl<'de> Visitor<'de> for GeoJsonVisitor {
            type Value = geojson::Owned;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a GeoJSON geometry")
            }

            fn visit_map<A: de::MapAccess<'de>>(self, map: A) -> Result<geojson::Owned, A::Error> {
                geojson::Owned::deserialize(de::value::MapAccessDeserializer::new(map))
            }

            fn visit_str<E: de::Error>(self, s: &str) -> Result<geojson::Owned, E> {
                serde_json::from_str(s).map_err(|e| E::custom(format!("invalid GeoJSON geometry: {}", e)))
            }
        }

        let geometry = d.deserialize_any(GeoJsonVisitor)?.into();
        G::from_geometry(geometry).map(Geometry::from).map_err(de::Error::custom)
    }
}

/// A bounding box in WGS 84, the value of a spatial filter of the `SqlxFilter` derive, see the
/// [module](self): `west,south,east,north` in a query string (`?bbox=2.25,48.81,2.42,48.90`),
/// or an array of these 4 numbers.
///
/// A box crossing the antimeridian (`west` greater than `east`) is refused.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bbox {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}

impl Bbox {
    pub fn new(west: f64, south: f64, east: f64, north: f64) -> Result<Self, String> {
        check_longitude(west)?;
        check_longitude(east)?;
        check_latitude(south)?;
        check_latitude(north)?;
        if west > east || south > north {
            return Err(format!("invalid bounding box {},{},{},{}, use west,south,east,north", west, south, east, north));
        }
        Ok(Bbox { west, south, east, north })
    }
}

/// A point in WGS 84 and a distance in meters, the value of the `dwithin` filter of the
/// `SqlxFilter` derive, see the [module](self): `longitude,latitude,distance` in a query string
/// (`?near=2.3522,48.8566,1000`), or an array of these 3 numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Near {
    pub longitude: f64,
    pub latitude: f64,
    /// In meters
    pub distance: f64,
}

impl Near {
    pub fn new(longitude: f64, latitude: f64, distance: f64) -> Result<Self, String> {
        check_longitude(longitude)?;
        check_latitude(latitude)?;
        if !(distance.is_finite() && distance >= 0.0) {
            return Err(format!("invalid distance {}, use a positive number of meters", distance));
        }
        Ok(Near { longitude, latitude, distance })
    }

    /// Pushes `ST_DWithin(column::geography, point::geography, distance)`, called by the `SqlxFilter` derive
    #[doc(hidden)]
    pub fn push_dwithin(&self, qb: &mut sqlx::QueryBuilder<Postgres>, column: &str) {
        qb.push("ST_DWithin(")
            .push(column)
            .push("::geography, ST_SetSRID(ST_MakePoint(")
            .push_bind(self.longitude)
            .push(", ")
            .push_bind(self.latitude)
            .push("), 4326)::geography, ")
            .push_bind(self.distance)
            .push(")");
    }
}

fn check_longitude(longitude: f64) -> Result<(), String> {
    if (-180.0..=180.0).contains(&longitude) {
        Ok(())
    } else {
        Err(format!("invalid longitude {}, use a number between -180 and 180", longitude))
    }
}

fn check_latitude(latitude: f64) -> Result<(), String> {
    if (-90.0..=90.0).contains(&latitude) {
        Ok(())
    } else {
        Err(format!("invalid latitude {}, use a number between -90 and 90", latitude))
    }
}

// `N` numbers separated by commas in a string, as in a query string, or an array of them
struct Numbers<const N: usize>(&'static str);

impl<'de, const N: usize> Visitor<'de> for Numbers<N> {
    type Value = [f64; N];

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(self.0)
    }

    fn visit_str<E: de::Error>(self, s: &str) -> Result<[f64; N], E> {
        let invalid = || E::invalid_value(de::Unexpected::Str(s), &self);
        let numbers: Vec<f64> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>().map_err(|_| invalid())?;
        numbers.try_into().map_err(|_| invalid())
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<[f64; N], A::Error> {
        let mut numbers = [0.0; N];
        for (i, n) in numbers.iter_mut().enumerate() {
            *n = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(i, &self))?;
        }
        if seq.next_element::<de::IgnoredAny>()?.is_some() {
            return Err(de::Error::invalid_length(N + 1, &self));
        }
        Ok(numbers)
    }
}

impl<'de> Deserialize<'de> for Bbox {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let [west, south, east, north] = d.deserialize_any(Numbers::<4>("a bounding box, west,south,east,north"))?;
        Bbox::new(west, south, east, north).map_err(de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for Near {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let [longitude, latitude, distance] = d.deserialize_any(Numbers::<3>("a point and a distance, longitude,latitude,meters"))?;
        Near::new(longitude, latitude, distance).map_err(de::Error::custom)
    }
}

/// The geometry argument of the `intersects`, `within` and `contains` filters of the `SqlxFilter`
/// derive: a [`Geometry`] or a [`Bbox`]
#[doc(hidden)]
pub trait SpatialArgument {
    fn push_argument(&self, qb: &mut sqlx::QueryBuilder<Postgres>);
}

impl<G: GeometryType> SpatialArgument for Geometry<G> {
    fn push_argument(&self, qb: &mut sqlx::QueryBuilder<Postgres>) {
        qb.push_bind(self);
    }
}

impl SpatialArgument for Bbox {
    fn push_argument(&self, qb: &mut sqlx::QueryBuilder<Postgres>) {
        qb.push("ST_MakeEnvelope(")
            .push_bind(self.west)
            .push(", ")
            .push_bind(self.south)
            .push(", ")
            .push_bind(self.east)
            .push(", ")
            .push_bind(self.north)
            .push(", 4326)");
    }
}

/// The schema of the GeoJSON geometry, of the kind of `G` (requires the `openapi` feature)
#[cfg(feature = "openapi")]
impl<G: GeometryType> schemars::JsonSchema for Geometry<G> {
    fn schema_name() -> String {
        format!("GeoJson{}", G::KIND)
    }

    fn json_schema(generator: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        use serde_json::{json, Value};

        // the coordinates of each kind, a position nested in `depth` arrays
        fn coordinates(depth: usize) -> Value {
            let position = json!({ "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 });
            (0..depth).fold(position, |items, _| json!({ "type": "array", "items": items }))
        }
        let object = |kind: &str, member: &str, value: Value| {
            json!({
                "type": "object",
                "required": ["type", member],
                "properties": { "type": { "type": "string", "enum": [kind] }, member: value },
            })
        };
        let kinds = [
            ("Point", 0),
            ("LineString", 1),
            ("Polygon", 2),
            ("MultiPoint", 1),
            ("MultiLineString", 2),
            ("MultiPolygon", 3),
        ];
        let mut collection = || {
            let any = generator.subschema_for::<Geometry>();
            object("GeometryCollection", "geometries", json!({ "type": "array", "items": any }))
        };
        let schema = match G::KIND {
            "GeometryCollection" => collection(),
            "Geometry" => {
                let mut variants: Vec<Value> = kinds.iter().map(|(k, depth)| object(k, "coordinates", coordinates(*depth))).collect();
                variants.push(collection());
                json!({ "oneOf": variants })
            }
            kind => {
                let depth = kinds.iter().find(|(k, _)| *k == kind).map_or(0, |(_, depth)| *depth);
                object(kind, "coordinates", coordinates(depth))
            }
        };
        let mut schema: schemars::schema::SchemaObject = serde_json::from_value(schema).expect("valid GeoJSON schema");
        schema.metadata().description = Some(format!("A GeoJSON {} (RFC 7946), [longitude, latitude] in WGS 84", G::KIND));
        schema.into()
    }
}

// The GraphQL scalars of the geometries (requires the `graphql` feature): GeoJSON as in the REST
// routes, an object or its string. Each kind is its own scalar, `GeoJsonPoint`..., GraphQL naming
// its types uniquely, `GeoJsonGeometry` for any kind
#[cfg(feature = "graphql")]
macro_rules! graphql_scalars {
    ($($kind:ty => $name:literal),+ $(,)?) => {$(
        /// A GeoJSON geometry (RFC 7946), [longitude, latitude] in WGS 84
        #[async_graphql::Scalar(name = $name, specified_by_url = "https://datatracker.ietf.org/doc/html/rfc7946")]
        impl async_graphql::ScalarType for Geometry<$kind> {
            fn parse(value: async_graphql::Value) -> async_graphql::InputValueResult<Self> {
                let json = value.into_json().map_err(async_graphql::InputValueError::custom)?;
                serde_json::from_value(json).map_err(async_graphql::InputValueError::custom)
            }

            fn to_value(&self) -> async_graphql::Value {
                // serialized directly, keeping `type` before `coordinates` as in the REST routes
                async_graphql::to_value(self).unwrap_or_default()
            }
        }
    )+};
}

#[cfg(feature = "graphql")]
graphql_scalars!(
    geo_types::Geometry<f64> => "GeoJsonGeometry",
    geo_types::Point<f64> => "GeoJsonPoint",
    geo_types::LineString<f64> => "GeoJsonLineString",
    geo_types::Polygon<f64> => "GeoJsonPolygon",
    geo_types::MultiPoint<f64> => "GeoJsonMultiPoint",
    geo_types::MultiLineString<f64> => "GeoJsonMultiLineString",
    geo_types::MultiPolygon<f64> => "GeoJsonMultiPolygon",
    geo_types::GeometryCollection<f64> => "GeoJsonGeometryCollection",
);

// The schema of the string of a query string, `N` numbers separated by commas
#[cfg(feature = "openapi")]
fn numbers_schema(n: usize, description: &str, example: &str) -> schemars::schema::Schema {
    let number = r"-?\d+(\.\d+)?";
    let pattern = format!(r"^{}(,{}){{{}}}$", number, number, n - 1);
    let schema: schemars::schema::SchemaObject = serde_json::from_value(serde_json::json!({
        "type": "string",
        "pattern": pattern,
        "description": description,
        "examples": [example],
    }))
    .expect("valid schema");
    schema.into()
}

/// A string, `west,south,east,north` (requires the `openapi` feature)
#[cfg(feature = "openapi")]
impl schemars::JsonSchema for Bbox {
    fn schema_name() -> String {
        "Bbox".to_string()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        numbers_schema(4, "A bounding box in WGS 84, west,south,east,north", "2.25,48.81,2.42,48.90")
    }
}

/// A string, `longitude,latitude,meters` (requires the `openapi` feature)
#[cfg(feature = "openapi")]
impl schemars::JsonSchema for Near {
    fn schema_name() -> String {
        "Near".to_string()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        numbers_schema(3, "A point in WGS 84 and a distance in meters, longitude,latitude,meters", "2.3522,48.8566,1000")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo_types::{coord, line_string, point, polygon};
    use serde_json::json;

    fn ewkb<G: GeometryType>(geometry: &Geometry<G>) -> Vec<u8> {
        let mut buf = Vec::new();
        ewkb::write(&mut buf, geometry.geometry.geometry_ref(), geometry.srid);
        buf
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02X}", b)).collect()
    }

    #[test]
    fn point_is_written_as_the_ewkb_of_postgis() {
        // SELECT ST_AsEWKB('SRID=4326;POINT(1 2)'::geometry)
        let p: Point = point!(x: 1.0, y: 2.0).into();
        assert_eq!(hex(&ewkb(&p)), "0101000020E6100000000000000000F03F0000000000000040");
        // SELECT ST_AsEWKB('POINT(1 2)'::geometry)
        let p = Point::new(point!(x: 1.0, y: 2.0), 0);
        assert_eq!(hex(&ewkb(&p)), "0101000000000000000000F03F0000000000000040");
    }

    #[test]
    fn ewkb_is_read_in_both_byte_orders_and_from_hex() {
        let little = from_hex("0101000020E6100000000000000000F03F0000000000000040").unwrap();
        let big = from_hex("0020000001000010E63FF00000000000004000000000000000").unwrap();
        for bytes in [little, big] {
            let (geometry, srid) = ewkb::read(&bytes).unwrap();
            assert_eq!(geometry, geo_types::Geometry::Point(point!(x: 1.0, y: 2.0)));
            assert_eq!(srid, WGS84);
        }
    }

    #[test]
    fn every_kind_survives_an_ewkb_round_trip() {
        let square = polygon![(x: 0.0, y: 0.0), (x: 4.0, y: 0.0), (x: 4.0, y: 4.0), (x: 0.0, y: 4.0)];
        let with_hole = geo_types::Polygon::new(
            square.exterior().clone(),
            vec![line_string![(x: 1.0, y: 1.0), (x: 2.0, y: 1.0), (x: 2.0, y: 2.0), (x: 1.0, y: 1.0)]],
        );
        let geometries: Vec<geo_types::Geometry<f64>> = vec![
            point!(x: -1.5, y: 47.2).into(),
            line_string![(x: 0.0, y: 0.0), (x: 1.0, y: 1.0)].into(),
            with_hole.clone().into(),
            geo_types::Polygon::new(geo_types::LineString(vec![]), vec![]).into(),
            geo_types::MultiPoint(vec![point!(x: 1.0, y: 2.0), point!(x: 3.0, y: 4.0)]).into(),
            geo_types::MultiLineString(vec![line_string![(x: 0.0, y: 0.0), (x: 1.0, y: 1.0)]]).into(),
            geo_types::MultiPolygon(vec![square.clone(), with_hole]).into(),
            geo_types::Geometry::GeometryCollection(geo_types::GeometryCollection(vec![point!(x: 1.0, y: 2.0).into(), square.into()])),
        ];
        for geometry in geometries {
            let g = Geometry::new(geometry.clone(), 3857);
            assert_eq!(ewkb::read(&ewkb(&g)).unwrap(), (geometry, 3857));
        }
    }

    #[test]
    fn unsupported_ewkb_fails_to_decode() {
        // POINT Z (1 2 3)
        let z = from_hex("0101000080000000000000F03F00000000000000400000000000000840").unwrap();
        assert!(ewkb::read(&z).unwrap_err().to_string().contains("Z or M"));
        // POINT EMPTY
        let empty = from_hex("0101000000000000000000F87F000000000000F87F").unwrap();
        assert!(ewkb::read(&empty).unwrap_err().to_string().contains("empty points"));
        // a line string announcing a billion points
        let truncated = from_hex("010200000000CA9A3B").unwrap();
        assert!(ewkb::read(&truncated).unwrap_err().to_string().contains("truncated"));
    }

    #[test]
    fn a_kind_rejects_the_other_kinds() {
        let line = geo_types::Geometry::LineString(line_string![(x: 0.0, y: 0.0), (x: 1.0, y: 1.0)]);
        let err = <geo_types::Point<f64> as sealed::Sealed>::from_geometry(line).unwrap_err();
        assert_eq!(err, "expected a Point, found a LineString");
    }

    #[test]
    fn geometries_are_serialized_as_geojson() {
        let p: Point = point!(x: 2.35, y: 48.85).into();
        assert_eq!(serde_json::to_value(&p).unwrap(), json!({ "type": "Point", "coordinates": [2.35, 48.85] }));
        let p: Polygon = polygon![(x: 0.0, y: 0.0), (x: 1.0, y: 0.0), (x: 1.0, y: 1.0)].into();
        assert_eq!(
            serde_json::to_value(&p).unwrap(),
            json!({ "type": "Polygon", "coordinates": [[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0]]] })
        );
        let gc: Geometry = geo_types::Geometry::GeometryCollection(geo_types::GeometryCollection(vec![point!(x: 1.0, y: 2.0).into()])).into();
        assert_eq!(
            serde_json::to_value(&gc).unwrap(),
            json!({ "type": "GeometryCollection", "geometries": [{ "type": "Point", "coordinates": [1.0, 2.0] }] })
        );
        let rect: Geometry = geo_types::Geometry::Rect(geo_types::Rect::new(coord! { x: 0.0, y: 0.0 }, coord! { x: 1.0, y: 1.0 })).into();
        assert_eq!(serde_json::to_value(&rect).unwrap()["type"], "Polygon");
    }

    #[test]
    fn geojson_is_deserialized_in_wgs84() {
        let value = json!({ "type": "MultiPoint", "coordinates": [[1.0, 2.0], [3, 4]], "bbox": [1, 2, 3, 4] });
        let mp: Geometry = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(mp.srid, WGS84);
        assert_eq!(mp.geometry, geo_types::MultiPoint(vec![point!(x: 1.0, y: 2.0), point!(x: 3.0, y: 4.0)]).into());
        assert_eq!(serde_json::to_value(&mp).unwrap()["coordinates"], json!([[1.0, 2.0], [3.0, 4.0]]));
        let err = serde_json::from_value::<Point>(value).unwrap_err();
        assert!(err.to_string().contains("expected a Point, found a MultiPoint"), "{}", err);
        let err = serde_json::from_value::<Point>(json!({ "type": "Point", "coordinates": [1, 2, 3] })).unwrap_err();
        assert!(err.to_string().contains("altitude"), "{}", err);
        let err = serde_json::from_value::<Point>(json!({ "type": "Circle", "coordinates": [1, 2] })).unwrap_err();
        assert!(err.to_string().contains("unknown variant `Circle`"), "{}", err);
    }

    #[test]
    fn default_is_the_empty_geometry_of_the_kind() {
        assert_eq!(Geometry::default(), Geometry::from(geo_types::Geometry::GeometryCollection(geo_types::GeometryCollection(vec![]))));
        assert_eq!(Polygon::default().srid, WGS84);
        assert_eq!(serde_json::to_value(Polygon::default()).unwrap(), json!({ "type": "Polygon", "coordinates": [] }));
        assert_eq!(Point::default().geometry, point!(x: 0.0, y: 0.0));
    }

    #[test]
    fn empty_polygon_has_no_ring() {
        let empty: Polygon = geo_types::Polygon::new(geo_types::LineString(vec![]), vec![]).into();
        assert_eq!(serde_json::to_value(&empty).unwrap(), json!({ "type": "Polygon", "coordinates": [] }));
        assert_eq!(serde_json::from_value::<Polygon>(json!({ "type": "Polygon", "coordinates": [] })).unwrap(), empty);
    }

    #[derive(Deserialize)]
    struct ListQuery {
        bbox: Option<Bbox>,
        near: Option<Near>,
        point: Option<Point>,
    }

    fn query(s: &str) -> Result<ListQuery, String> {
        actix_web::web::Query::<ListQuery>::from_query(s).map(|q| q.into_inner()).map_err(|e| e.to_string())
    }

    #[test]
    fn spatial_filters_are_read_from_the_query_string() {
        let q = query("bbox=2.25,48.81,2.42,48.9&near=2.3522,%2048.8566,1000&point=%7B%22type%22%3A%22Point%22%2C%22coordinates%22%3A%5B2.35%2C48.85%5D%7D").unwrap();
        assert_eq!(q.bbox, Some(Bbox { west: 2.25, south: 48.81, east: 2.42, north: 48.9 }));
        assert_eq!(q.near, Some(Near { longitude: 2.3522, latitude: 48.8566, distance: 1000.0 }));
        assert_eq!(q.point.unwrap().geometry, point!(x: 2.35, y: 48.85));
        let empty = query("").unwrap();
        assert!(empty.bbox.is_none() && empty.near.is_none() && empty.point.is_none());
        // and from JSON arrays
        let bbox: Bbox = serde_json::from_value(json!([-1, -2, 3, 4])).unwrap();
        assert_eq!(bbox, Bbox { west: -1.0, south: -2.0, east: 3.0, north: 4.0 });
    }

    #[test]
    fn invalid_spatial_filters_are_refused() {
        for (q, expected) in [
            ("bbox=1,2,3", "a bounding box, west,south,east,north"),
            ("bbox=1,2,3,4,5", "a bounding box"),
            ("bbox=1,2,x,4", "a bounding box"),
            ("bbox=3,2,1,4", "invalid bounding box 3,2,1,4"),
            ("bbox=0,-91,1,1", "invalid latitude -91"),
            ("near=181,0,10", "invalid longitude 181"),
            ("near=0,0,-1", "invalid distance -1"),
            ("near=0,0,NaN", "invalid distance NaN"),
            ("point=nope", "invalid GeoJSON geometry"),
            ("point=%7B%22type%22%3A%22LineString%22%2C%22coordinates%22%3A%5B%5D%7D", "expected a Point, found a LineString"),
        ] {
            let err = query(q).err().unwrap_or_else(|| panic!("{} was accepted", q));
            assert!(err.contains(expected), "{}: {}", q, err);
        }
    }

    #[cfg(feature = "openapi")]
    #[test]
    fn schema_describes_the_geojson_of_the_kind() {
        let point = serde_json::to_value(schemars::schema_for!(Point)).unwrap();
        assert_eq!(point["properties"]["type"]["enum"], json!(["Point"]));
        assert_eq!(point["properties"]["coordinates"]["maxItems"], 2);
        let polygon = serde_json::to_value(schemars::schema_for!(Polygon)).unwrap();
        assert_eq!(polygon["properties"]["coordinates"]["items"]["items"]["minItems"], 2);
        let any = serde_json::to_value(schemars::schema_for!(Geometry)).unwrap();
        assert_eq!(any["oneOf"].as_array().unwrap().len(), 7);
        assert!(any["description"].as_str().unwrap().contains("GeoJSON Geometry"));
    }
}
