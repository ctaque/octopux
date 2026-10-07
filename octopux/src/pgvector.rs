//! The pgvector `vector`, `halfvec` and `sparsevec` columns (requires the `pgvector` feature).
//!
//! [`Vector`], [`HalfVector`] and [`SparseVector`] are field types of the models deriving the sqlx
//! derives on PostgreSQL: they are read from and written to the database in the binary format of
//! pgvector, and sent to and received from the client as JSON.
//!
//! ```ignore
//!
//! use octopux::pgvector::{self, Vector};
//!
//! #[derive(Serialize, Deserialize, sqlx::FromRow, HttpFindListDelete, SqlxModel)]
//! #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
//! #[sqlx_model(database = "postgres", filter)]
//! #[octopux_info(path = "document")]
//! struct Document {
//!     id: Id,
//!     content: String,
//!     embedding: Vector,                        // vector(1536) NOT NULL
//!     keywords: Option<pgvector::SparseVector>, // sparsevec(30522)
//! }
//! ```
//!
//! - a [`Vector`] or a [`HalfVector`] is an array of numbers, `[0.12, -0.5, 0.33]`. A
//!   [`HalfVector`] holds `f32` values, rounded to half precision (`f16`) when written to the
//!   database, which halves the size of the column and of its index
//! - a [`SparseVector`] is an object of its number of dimensions and of its non-zero values with
//!   their indices, from 0: `{"dimensions": 5, "indices": [0, 3], "values": [1.5, 2.0]}`
//!
//! Read from a string, as in a query string, they are the literals of pgvector: `[0.12,-0.5,0.33]`
//! (or `0.12,-0.5,0.33`), and `{1:1.5,4:2}/5` for a sparse vector, its indices from 1 as in SQL.
//!
//! The values are finite numbers, and a vector has 1 to 16000 dimensions (1 to 1000000000 for a
//! sparse vector, with at most 16000 non-zero values), as pgvector requires. The number of
//! dimensions of the column, `vector(1536)`, is checked by the database: a vector of another size
//! is refused with 400 `BAD_REQUEST`.
//!
//! # GraphQL
//!
//! With the `graphql` feature, the vectors are async-graphql scalars, as in the REST routes, so that
//! the models deriving `SimpleObject` and `InputObject` can hold them: `Vector` and `HalfVector`
//! (a list of numbers), `SparseVector` (an object of `dimensions`, `indices` and `values`). An input
//! can also be the literal of pgvector.
//!
//! # Similarity search
//!
//! The `SqlxFilter` derive orders a list by the distance of a column to a vector, nearest first,
//! with `op = "nearest"`; `distance` is `cosine` (by default), `l2`, `inner_product` or `l1`:
//!
//! ```ignore
//!
//! #[derive(Deserialize, SqlxFilter)]
//! #[sqlx_filter(database = "postgres")]
//! struct ListQuery {
//!     offset: Option<usize>,
//!     limit: Option<usize>,
//!     #[sqlx_filter(column = "embedding", op = "nearest")]
//!     near: Option<pgvector::Vector>,           // ?near=[0.12,-0.5,0.33]: ORDER BY embedding <=> $1
//!     #[sqlx_filter(column = "embedding", op = "nearest", distance = "l2")]
//!     near_l2: Option<pgvector::Vector>,        // ORDER BY embedding <-> $1
//! }
//! ```
//!
//! The rows are ordered by distance before the columns of the `sort` field, and `limit` keeps the
//! `k` nearest ones: an HNSW index on the column with the operator class of the distance,
//! `USING hnsw (embedding vector_cosine_ops)`, finds them without reading the whole table.
//!
//! | `distance` | Operator | Operator class |
//! | --- | --- | --- |
//! | `cosine` | `<=>` | `vector_cosine_ops` |
//! | `l2` | `<->` | `vector_l2_ops` |
//! | `inner_product` | `<#>`, the negative inner product | `vector_ip_ops` |
//! | `l1` | `<+>` | `vector_l1_ops` |
//!
//! (`halfvec_cosine_ops`, `sparsevec_cosine_ops`... for the other types)

use std::ops::{Deref, DerefMut};

use serde::de::{self, Deserializer, SeqAccess, Visitor};
use serde::ser::{SerializeStruct, Serializer};
use serde::{Deserialize, Serialize};
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::postgres::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueFormat, PgValueRef, Postgres};

/// The most dimensions of a `vector` or a `halfvec`
pub const MAX_DIMENSIONS: usize = 16_000;
/// The most dimensions of a `sparsevec`
pub const SPARSE_MAX_DIMENSIONS: usize = 1_000_000_000;
/// The most non-zero values of a `sparsevec`
pub const SPARSE_MAX_NON_ZERO: usize = 16_000;

/// A pgvector `vector`, single precision values, see the [module](self).
///
/// It dereferences to its values. The default vector is empty, so that the models holding vectors
/// can derive `Default`, but pgvector refuses a vector without dimensions.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Vector(pub Vec<f32>);

/// A pgvector `halfvec`, half precision values held as `f32`, see the [module](self).
///
/// Its values are rounded to the nearest `f16` when written to the database. It dereferences to its
/// values. The default vector is empty, so that the models holding vectors can derive `Default`,
/// but pgvector refuses a vector without dimensions.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HalfVector(pub Vec<f32>);

macro_rules! dense {
    ($($ty:ident => $sql:literal),+ $(,)?) => {$(
        impl $ty {
            pub fn new(values: Vec<f32>) -> Self {
                $ty(values)
            }

            pub fn into_inner(self) -> Vec<f32> {
                self.0
            }
        }

        impl From<Vec<f32>> for $ty {
            fn from(values: Vec<f32>) -> Self {
                $ty(values)
            }
        }

        impl From<$ty> for Vec<f32> {
            fn from(vector: $ty) -> Self {
                vector.0
            }
        }

        impl Deref for $ty {
            type Target = Vec<f32>;

            fn deref(&self) -> &Vec<f32> {
                &self.0
            }
        }

        impl DerefMut for $ty {
            fn deref_mut(&mut self) -> &mut Vec<f32> {
                &mut self.0
            }
        }

        impl sqlx::Type<Postgres> for $ty {
            fn type_info() -> PgTypeInfo {
                PgTypeInfo::with_name($sql)
            }
        }

        impl PgHasArrayType for $ty {
            fn array_type_info() -> PgTypeInfo {
                PgTypeInfo::array_of($sql)
            }
        }

        impl Serialize for $ty {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                self.0.serialize(s)
            }
        }
    )+};
}

dense!(Vector => "vector", HalfVector => "halfvec");

/// A pgvector `sparsevec`: a number of dimensions and the non-zero values, by increasing index,
/// see the [module](self).
///
/// The indices start at 0, `{1:1.5,4:2}/5` in SQL being the indices `0` and `3`. The default
/// vector has no dimension, so that the models holding vectors can derive `Default`, but pgvector
/// refuses a vector without dimensions.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SparseVector {
    dimensions: usize,
    indices: Vec<usize>,
    values: Vec<f32>,
}

impl SparseVector {
    /// The vector of `dimensions` holding the values of `entries`, `(index, value)` in any order,
    /// the zeros being left out. Fails when an index is repeated or out of the dimensions, or
    /// when a value is not finite.
    pub fn new(dimensions: usize, entries: impl IntoIterator<Item = (usize, f32)>) -> Result<Self, String> {
        if !(1..=SPARSE_MAX_DIMENSIONS).contains(&dimensions) {
            return Err(format!("a sparse vector has 1 to {} dimensions, not {}", SPARSE_MAX_DIMENSIONS, dimensions));
        }
        let mut entries: Vec<(usize, f32)> = entries.into_iter().filter(|(_, value)| *value != 0.0).collect();
        entries.sort_by_key(|(index, _)| *index);
        if entries.len() > SPARSE_MAX_NON_ZERO {
            return Err(format!("a sparse vector has at most {} non-zero values, not {}", SPARSE_MAX_NON_ZERO, entries.len()));
        }
        for (i, (index, value)) in entries.iter().enumerate() {
            if *index >= dimensions {
                return Err(format!("the index {} is out of the {} dimensions of the sparse vector", index, dimensions));
            }
            if i > 0 && entries[i - 1].0 == *index {
                return Err(format!("the index {} of the sparse vector is repeated", index));
            }
            check_finite(*value)?;
        }
        let (indices, values) = entries.into_iter().unzip();
        Ok(SparseVector { dimensions, indices, values })
    }

    /// The sparse vector of the non-zero values of `values`
    pub fn from_dense(values: &[f32]) -> Result<Self, String> {
        SparseVector::new(values.len(), values.iter().copied().enumerate())
    }

    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    /// The indices of the non-zero values, increasing
    pub fn indices(&self) -> &[usize] {
        &self.indices
    }

    /// The non-zero values, in the order of their indices
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    /// `(index, value)` of the non-zero values
    pub fn entries(&self) -> impl Iterator<Item = (usize, f32)> + '_ {
        self.indices.iter().copied().zip(self.values.iter().copied())
    }

    /// All the values, the zeros included
    pub fn to_dense(&self) -> Vec<f32> {
        let mut dense = vec![0.0; self.dimensions];
        self.entries().for_each(|(index, value)| dense[index] = value);
        dense
    }
}

impl sqlx::Type<Postgres> for SparseVector {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::with_name("sparsevec")
    }
}

impl PgHasArrayType for SparseVector {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::array_of("sparsevec")
    }
}

fn check_finite(value: f32) -> Result<(), String> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(format!("invalid vector value {}, use a finite number", value))
    }
}

// The values of a `vector` or a `halfvec`
fn check_dense(values: &[f32], half: bool) -> Result<(), String> {
    if values.is_empty() || values.len() > MAX_DIMENSIONS {
        return Err(format!("a vector has 1 to {} dimensions, not {}", MAX_DIMENSIONS, values.len()));
    }
    for value in values {
        check_finite(*value)?;
        if half && !f16_to_f32(f32_to_f16(*value)).is_finite() {
            return Err(format!("the value {} is out of the range of a halfvec, -65504 to 65504", value));
        }
    }
    Ok(())
}

/// The bits of the `f16` nearest to `value`, ties to even, infinite beyond the range of `f16`
fn f32_to_f16(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xff) as i32;
    let mantissa = bits & 0x7f_ffff;
    if exponent == 0xff {
        // infinite, or NaN keeping a mantissa
        return sign | 0x7c00 | if mantissa != 0 { 0x0200 } else { 0 };
    }
    let half_exponent = exponent - 127 + 15;
    if half_exponent >= 0x1f {
        return sign | 0x7c00;
    }
    // the mantissa shifted by `shift` bits, rounded to the nearest, ties to even
    let round = |mantissa: u32, shift: u32| {
        let kept = mantissa >> shift;
        let rest = mantissa & ((1 << shift) - 1);
        let half = 1 << (shift - 1);
        if rest > half || (rest == half && kept & 1 == 1) { kept + 1 } else { kept }
    };
    if half_exponent <= 0 {
        // subnormal, `mantissa * 2^-24`, or zero
        if half_exponent < -10 {
            return sign;
        }
        return sign | round(mantissa | 0x80_0000, (14 - half_exponent) as u32) as u16;
    }
    // a carry of the rounding into the exponent gives the next power of 2, or infinity
    sign | round(((half_exponent as u32) << 23) | mantissa, 13) as u16
}

fn f16_to_f32(half: u16) -> f32 {
    let sign = ((half & 0x8000) as u32) << 16;
    let exponent = ((half >> 10) & 0x1f) as u32;
    let mantissa = (half & 0x3ff) as u32;
    match exponent {
        0 => {
            // zero or subnormal, `mantissa * 2^-24`
            let value = mantissa as f32 / 16_777_216.0;
            if sign != 0 { -value } else { value }
        }
        0x1f => f32::from_bits(sign | 0x7f80_0000 | (mantissa << 13)),
        _ => f32::from_bits(sign | ((exponent + 127 - 15) << 23) | (mantissa << 13)),
    }
}

/// The binary format of pgvector, in network byte order: `vector` and `halfvec` are their number of
/// dimensions (`u16`), an unused `u16`, then their values (`f32` or `f16`); `sparsevec` is its
/// number of dimensions, of non-zero values, an unused `i32`, the indices from 0 (`i32`) then the
/// values (`f32`).
mod binary {
    use super::*;

    pub fn write_dense(buf: &mut Vec<u8>, values: &[f32], half: bool) -> Result<(), BoxDynError> {
        check_dense(values, half)?;
        buf.extend_from_slice(&(values.len() as u16).to_be_bytes());
        buf.extend_from_slice(&0u16.to_be_bytes());
        for value in values {
            if half {
                buf.extend_from_slice(&f32_to_f16(*value).to_be_bytes());
            } else {
                buf.extend_from_slice(&value.to_be_bytes());
            }
        }
        Ok(())
    }

    pub fn write_sparse(buf: &mut Vec<u8>, vector: &SparseVector) -> Result<(), BoxDynError> {
        if vector.dimensions == 0 {
            return Err("a sparse vector has at least 1 dimension".into());
        }
        buf.extend_from_slice(&(vector.dimensions as i32).to_be_bytes());
        buf.extend_from_slice(&(vector.indices.len() as i32).to_be_bytes());
        buf.extend_from_slice(&0i32.to_be_bytes());
        vector.indices.iter().for_each(|index| buf.extend_from_slice(&(*index as i32).to_be_bytes()));
        vector.values.iter().for_each(|value| buf.extend_from_slice(&value.to_be_bytes()));
        Ok(())
    }

    struct Reader<'a>(&'a [u8]);

    impl Reader<'_> {
        fn take<const N: usize>(&mut self) -> Result<[u8; N], BoxDynError> {
            let (head, rest) = self.0.split_first_chunk::<N>().ok_or("truncated vector")?;
            self.0 = rest;
            Ok(*head)
        }

        // a count of elements of `size` bytes, checked against the bytes left so that a corrupted
        // count does not allocate a huge vector
        fn count(&self, n: usize, size: usize) -> Result<usize, BoxDynError> {
            if n.saturating_mul(size) > self.0.len() {
                return Err("truncated vector".into());
            }
            Ok(n)
        }

        fn end(&self) -> Result<(), BoxDynError> {
            if self.0.is_empty() {
                Ok(())
            } else {
                Err(format!("{} unexpected bytes after the vector", self.0.len()).into())
            }
        }
    }

    pub fn read_dense(bytes: &[u8], half: bool) -> Result<Vec<f32>, BoxDynError> {
        let mut reader = Reader(bytes);
        let dimensions = u16::from_be_bytes(reader.take()?) as usize;
        reader.take::<2>()?;
        let n = reader.count(dimensions, if half { 2 } else { 4 })?;
        let values = (0..n)
            .map(|_| Ok(if half { f16_to_f32(u16::from_be_bytes(reader.take()?)) } else { f32::from_be_bytes(reader.take()?) }))
            .collect::<Result<_, BoxDynError>>()?;
        reader.end()?;
        Ok(values)
    }

    pub fn read_sparse(bytes: &[u8]) -> Result<SparseVector, BoxDynError> {
        let mut reader = Reader(bytes);
        let dimensions = i32::from_be_bytes(reader.take()?);
        let non_zero = i32::from_be_bytes(reader.take()?);
        reader.take::<4>()?;
        let non_zero = reader.count(usize::try_from(non_zero).map_err(|_| "invalid sparse vector")?, 8)?;
        let indices: Vec<i32> = (0..non_zero).map(|_| reader.take().map(i32::from_be_bytes)).collect::<Result<_, _>>()?;
        let values: Vec<f32> = (0..non_zero).map(|_| reader.take().map(f32::from_be_bytes)).collect::<Result<_, _>>()?;
        reader.end()?;
        let dimensions = usize::try_from(dimensions).map_err(|_| "invalid sparse vector")?;
        let indices = indices.into_iter().map(|i| usize::try_from(i).map_err(|_| "invalid sparse vector"));
        let entries = indices.zip(values).map(|(index, value)| index.map(|index| (index, value))).collect::<Result<Vec<_>, _>>()?;
        Ok(SparseVector::new(dimensions, entries)?)
    }
}

/// The literals of pgvector: `[1,2,3]` for `vector` and `halfvec`, `{1:1.5,4:2}/5` for `sparsevec`,
/// its indices from 1
mod text {
    use super::*;

    // the brackets are optional, as in a query string
    pub fn read_dense(literal: &str) -> Result<Vec<f32>, String> {
        let trimmed = literal.trim();
        let inner = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')).unwrap_or(trimmed);
        if inner.trim().is_empty() {
            return Ok(Vec::new());
        }
        inner
            .split(',')
            .map(|n| n.trim().parse::<f32>().map_err(|_| format!("invalid vector `{}`, use [1.5,-2,0.3]", literal)))
            .collect()
    }

    pub fn read_sparse(literal: &str) -> Result<SparseVector, String> {
        let invalid = || format!("invalid sparse vector `{}`, use {{1:1.5,4:2}}/5, the indices from 1", literal);
        let (entries, dimensions) = literal.trim().split_once('/').ok_or_else(invalid)?;
        let dimensions: usize = dimensions.trim().parse().map_err(|_| invalid())?;
        let entries = entries.trim().strip_prefix('{').and_then(|s| s.strip_suffix('}')).ok_or_else(invalid)?;
        let entries = entries
            .split(',')
            .filter(|entry| !entry.trim().is_empty())
            .map(|entry| {
                let (index, value) = entry.split_once(':').ok_or_else(invalid)?;
                let index: usize = index.trim().parse().map_err(|_| invalid())?;
                let value: f32 = value.trim().parse().map_err(|_| invalid())?;
                // from 1 in the literal
                Ok((index.checked_sub(1).ok_or_else(invalid)?, value))
            })
            .collect::<Result<Vec<_>, String>>()?;
        SparseVector::new(dimensions, entries)
    }
}

impl sqlx::Encode<'_, Postgres> for Vector {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        binary::write_dense(buf, &self.0, false)?;
        Ok(IsNull::No)
    }
}

impl sqlx::Encode<'_, Postgres> for HalfVector {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        binary::write_dense(buf, &self.0, true)?;
        Ok(IsNull::No)
    }
}

impl sqlx::Encode<'_, Postgres> for SparseVector {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        binary::write_sparse(buf, self)?;
        Ok(IsNull::No)
    }
}

impl<'r> sqlx::Decode<'r, Postgres> for Vector {
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(Vector(match value.format() {
            PgValueFormat::Binary => binary::read_dense(value.as_bytes()?, false)?,
            PgValueFormat::Text => text::read_dense(value.as_str()?)?,
        }))
    }
}

impl<'r> sqlx::Decode<'r, Postgres> for HalfVector {
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(HalfVector(match value.format() {
            PgValueFormat::Binary => binary::read_dense(value.as_bytes()?, true)?,
            PgValueFormat::Text => text::read_dense(value.as_str()?)?,
        }))
    }
}

impl<'r> sqlx::Decode<'r, Postgres> for SparseVector {
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(match value.format() {
            PgValueFormat::Binary => binary::read_sparse(value.as_bytes()?)?,
            PgValueFormat::Text => text::read_sparse(value.as_str()?)?,
        })
    }
}

// The values of a vector: an array of numbers, or the literal of pgvector, as in a query string
struct DenseVisitor;

impl<'de> Visitor<'de> for DenseVisitor {
    type Value = Vec<f32>;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a vector, an array of numbers")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<f32>, A::Error> {
        let mut values = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(MAX_DIMENSIONS));
        while let Some(value) = seq.next_element()? {
            values.push(value);
        }
        Ok(values)
    }

    fn visit_str<E: de::Error>(self, s: &str) -> Result<Vec<f32>, E> {
        text::read_dense(s).map_err(E::custom)
    }
}

impl<'de> Deserialize<'de> for Vector {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let values = d.deserialize_any(DenseVisitor)?;
        check_dense(&values, false).map_err(de::Error::custom)?;
        Ok(Vector(values))
    }
}

impl<'de> Deserialize<'de> for HalfVector {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let values = d.deserialize_any(DenseVisitor)?;
        check_dense(&values, true).map_err(de::Error::custom)?;
        Ok(HalfVector(values))
    }
}

/// `{"dimensions": 5, "indices": [0, 3], "values": [1.5, 2.0]}`
impl Serialize for SparseVector {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut object = s.serialize_struct("SparseVector", 3)?;
        object.serialize_field("dimensions", &self.dimensions)?;
        object.serialize_field("indices", &self.indices)?;
        object.serialize_field("values", &self.values)?;
        object.end()
    }
}

/// The object of its dimensions, indices and values, or the literal of pgvector, `{1:1.5,4:2}/5`
impl<'de> Deserialize<'de> for SparseVector {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Object {
            dimensions: usize,
            indices: Vec<usize>,
            values: Vec<f32>,
        }

        struct SparseVisitor;

        impl<'de> Visitor<'de> for SparseVisitor {
            type Value = SparseVector;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a sparse vector, {\"dimensions\": 5, \"indices\": [0, 3], \"values\": [1.5, 2.0]}")
            }

            fn visit_map<A: de::MapAccess<'de>>(self, map: A) -> Result<SparseVector, A::Error> {
                let object = Object::deserialize(de::value::MapAccessDeserializer::new(map))?;
                if object.indices.len() != object.values.len() {
                    return Err(de::Error::custom(format!(
                        "the sparse vector has {} indices for {} values",
                        object.indices.len(),
                        object.values.len()
                    )));
                }
                SparseVector::new(object.dimensions, object.indices.into_iter().zip(object.values)).map_err(de::Error::custom)
            }

            fn visit_str<E: de::Error>(self, s: &str) -> Result<SparseVector, E> {
                text::read_sparse(s).map_err(E::custom)
            }
        }

        d.deserialize_any(SparseVisitor)
    }
}

/// The vector argument of the `nearest` filter of the `SqlxFilter` derive: a [`Vector`], a
/// [`HalfVector`] or a [`SparseVector`]
#[doc(hidden)]
pub trait NearestArgument {
    fn push_argument(&self, qb: &mut sqlx::QueryBuilder<Postgres>);
}

impl NearestArgument for Vector {
    fn push_argument(&self, qb: &mut sqlx::QueryBuilder<Postgres>) {
        qb.push_bind(self);
    }
}

impl NearestArgument for HalfVector {
    fn push_argument(&self, qb: &mut sqlx::QueryBuilder<Postgres>) {
        qb.push_bind(self);
    }
}

impl NearestArgument for SparseVector {
    fn push_argument(&self, qb: &mut sqlx::QueryBuilder<Postgres>) {
        qb.push_bind(self);
    }
}

/// An array of numbers, of 1 to 16000 items (requires the `openapi` feature)
#[cfg(feature = "openapi")]
fn dense_schema(name: &str) -> schemars::schema::Schema {
    let schema: schemars::schema::SchemaObject = serde_json::from_value(serde_json::json!({
        "type": "array",
        "items": { "type": "number", "format": "float" },
        "minItems": 1,
        "maxItems": MAX_DIMENSIONS,
        "description": format!("A pgvector {}, its values", name),
        "examples": [[0.12, -0.5, 0.33]],
    }))
    .expect("valid schema");
    schema.into()
}

/// An array of numbers (requires the `openapi` feature)
#[cfg(feature = "openapi")]
impl schemars::JsonSchema for Vector {
    fn schema_name() -> String {
        "Vector".to_string()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        dense_schema("vector")
    }
}

/// An array of numbers (requires the `openapi` feature)
#[cfg(feature = "openapi")]
impl schemars::JsonSchema for HalfVector {
    fn schema_name() -> String {
        "HalfVector".to_string()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        dense_schema("halfvec, rounded to half precision")
    }
}

/// An object of its dimensions, indices and values (requires the `openapi` feature)
#[cfg(feature = "openapi")]
impl schemars::JsonSchema for SparseVector {
    fn schema_name() -> String {
        "SparseVector".to_string()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let schema: schemars::schema::SchemaObject = serde_json::from_value(serde_json::json!({
            "type": "object",
            "required": ["dimensions", "indices", "values"],
            "properties": {
                "dimensions": { "type": "integer", "minimum": 1, "maximum": SPARSE_MAX_DIMENSIONS },
                "indices": { "type": "array", "items": { "type": "integer", "minimum": 0 }, "maxItems": SPARSE_MAX_NON_ZERO },
                "values": { "type": "array", "items": { "type": "number", "format": "float" }, "maxItems": SPARSE_MAX_NON_ZERO },
            },
            "description": "A pgvector sparsevec, its non-zero values and their indices, from 0",
            "examples": [{ "dimensions": 5, "indices": [0, 3], "values": [1.5, 2.0] }],
        }))
        .expect("valid schema");
        schema.into()
    }
}

// The GraphQL scalars of the vectors (requires the `graphql` feature), as in the REST routes: a
// list of numbers or an object, or the literal of pgvector
#[cfg(feature = "graphql")]
macro_rules! graphql_scalars {
    ($($ty:ty => $name:literal: $description:literal),+ $(,)?) => {$(
        #[doc = $description]
        #[async_graphql::Scalar(name = $name, specified_by_url = "https://github.com/pgvector/pgvector")]
        impl async_graphql::ScalarType for $ty {
            fn parse(value: async_graphql::Value) -> async_graphql::InputValueResult<Self> {
                let json = value.into_json().map_err(async_graphql::InputValueError::custom)?;
                serde_json::from_value(json).map_err(async_graphql::InputValueError::custom)
            }

            // through the JSON text, where the `f32` values are written as they are in the REST routes,
            // `0.1` rather than the `0.10000000149011612` of their `f64`
            fn to_value(&self) -> async_graphql::Value {
                serde_json::to_string(self)
                    .and_then(|json| serde_json::from_str(&json))
                    .map(async_graphql::Value::from_json)
                    .ok()
                    .and_then(Result::ok)
                    .unwrap_or_default()
            }
        }
    )+};
}

#[cfg(feature = "graphql")]
graphql_scalars!(
    Vector => "Vector": "A pgvector vector, a list of numbers",
    HalfVector => "HalfVector": "A pgvector halfvec, a list of numbers rounded to half precision",
    SparseVector => "SparseVector": "A pgvector sparsevec, {dimensions, indices, values}, the indices from 0",
);

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn encoded<T: for<'q> sqlx::Encode<'q, Postgres>>(value: &T) -> Result<Vec<u8>, BoxDynError> {
        let mut buf = PgArgumentBuffer::default();
        let _ = value.encode_by_ref(&mut buf)?;
        Ok(buf.to_vec())
    }

    #[test]
    fn vector_is_written_in_the_binary_format_of_pgvector() {
        // SELECT vector_send('[1,2]')
        let bytes = encoded(&Vector(vec![1.0, 2.0])).unwrap();
        assert_eq!(bytes, [0, 2, 0, 0, 0x3f, 0x80, 0, 0, 0x40, 0, 0, 0]);
        assert_eq!(binary::read_dense(&bytes, false).unwrap(), [1.0, 2.0]);
        // SELECT halfvec_send('[1,-2]')
        let bytes = encoded(&HalfVector(vec![1.0, -2.0])).unwrap();
        assert_eq!(bytes, [0, 2, 0, 0, 0x3c, 0, 0xc0, 0]);
        assert_eq!(binary::read_dense(&bytes, true).unwrap(), [1.0, -2.0]);
    }

    #[test]
    fn sparse_vector_is_written_in_the_binary_format_of_pgvector() {
        // SELECT sparsevec_send('{1:1.5,4:2}/5')
        let vector = SparseVector::new(5, [(3, 2.0), (0, 1.5), (2, 0.0)]).unwrap();
        let bytes = encoded(&vector).unwrap();
        assert_eq!(
            bytes,
            [0, 0, 0, 5, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0x3f, 0xc0, 0, 0, 0x40, 0, 0, 0]
        );
        assert_eq!(binary::read_sparse(&bytes).unwrap(), vector);
        assert_eq!(vector.to_dense(), [1.5, 0.0, 0.0, 2.0, 0.0]);
    }

    #[test]
    fn invalid_vectors_fail_to_encode_and_decode() {
        assert!(encoded(&Vector(vec![])).unwrap_err().to_string().contains("1 to 16000 dimensions"));
        assert!(encoded(&Vector(vec![f32::NAN])).unwrap_err().to_string().contains("finite"));
        assert!(encoded(&HalfVector(vec![70000.0])).unwrap_err().to_string().contains("range of a halfvec"));
        assert!(encoded(&SparseVector::default()).is_err());
        // a vector announcing 1000 dimensions
        assert!(binary::read_dense(&[0x03, 0xe8, 0, 0, 0, 0, 0, 0], false).unwrap_err().to_string().contains("truncated"));
        assert!(binary::read_dense(&[0, 1, 0, 0, 0, 0, 0, 0, 1], false).unwrap_err().to_string().contains("unexpected bytes"));
    }

    #[test]
    fn half_precision_is_rounded_to_the_nearest() {
        for (value, bits) in [
            (0.0, 0x0000),
            (-0.0, 0x8000),
            (1.0, 0x3c00),
            (-2.0, 0xc000),
            (0.1, 0x2e66),
            (65504.0, 0x7bff),
            (65519.0, 0x7bff),
            (65520.0, 0x7c00),
            (5.960_464_5e-8, 0x0001),
            (6.097_555_2e-5, 0x03ff),
            (1e-9, 0x0000),
            (f32::INFINITY, 0x7c00),
        ] {
            assert_eq!(f32_to_f16(value), bits, "{}", value);
        }
        // ties to even: 1 + 2^-11 is between 1 and 1 + 2^-10
        assert_eq!(f32_to_f16(1.0 + 1.0 / 2048.0), 0x3c00);
        assert_eq!(f32_to_f16(1.0 + 3.0 / 2048.0), 0x3c02);
        assert!(f16_to_f32(f32_to_f16(f32::NAN)).is_nan());
        for bits in [0x0001, 0x03ff, 0x0400, 0x3c00, 0x2e66, 0x7bff, 0xc000, 0x8001] {
            assert_eq!(f32_to_f16(f16_to_f32(bits)), bits, "{:x}", bits);
        }
    }

    #[test]
    fn literals_are_read() {
        assert_eq!(text::read_dense("[1,-2.5, 3e2]").unwrap(), [1.0, -2.5, 300.0]);
        assert_eq!(text::read_dense("1,2").unwrap(), [1.0, 2.0]);
        assert!(text::read_dense("[1,x]").unwrap_err().contains("invalid vector `[1,x]`"));
        let sparse = text::read_sparse("{1:1.5,4:2}/5").unwrap();
        assert_eq!((sparse.dimensions(), sparse.indices(), sparse.values()), (5, &[0, 3][..], &[1.5, 2.0][..]));
        assert_eq!(text::read_sparse("{}/3").unwrap().dimensions(), 3);
        for invalid in ["{0:1}/5", "{6:1}/5", "{1:1,1:2}/5", "{1:1}", "1:1/5"] {
            assert!(text::read_sparse(invalid).is_err(), "{}", invalid);
        }
    }

    #[test]
    fn vectors_are_json() {
        let vector: Vector = serde_json::from_value(json!([1, -0.5])).unwrap();
        assert_eq!(vector, Vector(vec![1.0, -0.5]));
        assert_eq!(serde_json::to_value(&vector).unwrap(), json!([1.0, -0.5]));
        let sparse: SparseVector = serde_json::from_value(json!({ "dimensions": 5, "indices": [3, 0], "values": [2, 1.5] })).unwrap();
        assert_eq!(serde_json::to_value(&sparse).unwrap(), json!({ "dimensions": 5, "indices": [0, 3], "values": [1.5, 2.0] }));
        for (value, expected) in [
            (json!([]), "1 to 16000 dimensions, not 0"),
            (json!(["a"]), "invalid type"),
            (json!({ "dimensions": 5, "indices": [0], "values": [] }), "1 indices for 0 values"),
            (json!({ "dimensions": 5, "indices": [5], "values": [1] }), "index 5 is out of the 5 dimensions"),
            (json!({ "dimensions": 0, "indices": [], "values": [] }), "1 to 1000000000 dimensions, not 0"),
        ] {
            let err = match value.is_array() {
                true => serde_json::from_value::<Vector>(value).unwrap_err(),
                false => serde_json::from_value::<SparseVector>(value).unwrap_err(),
            };
            assert!(err.to_string().contains(expected), "{}", err);
        }
        let err = serde_json::from_value::<HalfVector>(json!([1e6])).unwrap_err();
        assert!(err.to_string().contains("range of a halfvec"), "{}", err);
    }

    #[derive(Debug, Deserialize)]
    struct ListQuery {
        near: Option<Vector>,
        sparse: Option<SparseVector>,
    }

    fn query(s: &str) -> Result<ListQuery, String> {
        actix_web::web::Query::<ListQuery>::from_query(s).map(|q| q.into_inner()).map_err(|e| e.to_string())
    }

    #[test]
    fn vectors_are_read_from_the_query_string() {
        let q = query("near=%5B0.1,-0.2,0.3%5D&sparse=%7B1:1.5,4:2%7D/5").unwrap();
        assert_eq!(q.near, Some(Vector(vec![0.1, -0.2, 0.3])));
        assert_eq!(q.sparse.unwrap().indices(), [0, 3]);
        assert_eq!(query("near=0.1,0.2").unwrap().near, Some(Vector(vec![0.1, 0.2])));
        let empty = query("").unwrap();
        assert!(empty.near.is_none() && empty.sparse.is_none());
        assert!(query("near=").unwrap_err().contains("1 to 16000 dimensions"));
        assert!(query("near=nope").unwrap_err().contains("invalid vector"));
    }

    #[cfg(feature = "openapi")]
    #[test]
    fn schemas_describe_the_json_of_the_vectors() {
        let vector = serde_json::to_value(schemars::schema_for!(Vector)).unwrap();
        assert_eq!(vector["type"], "array");
        assert_eq!(vector["maxItems"], 16000);
        let sparse = serde_json::to_value(schemars::schema_for!(SparseVector)).unwrap();
        assert_eq!(sparse["required"], json!(["dimensions", "indices", "values"]));
    }
}
