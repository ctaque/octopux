//! The errors of the generated routes, answered as a JSON body `{"code": "...", "message": "..."}`.

use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use serde::Serialize;
use std::fmt;

/// An error the client can act on, returned by the implementations of the model traits through
/// their `anyhow::Result`: the generated handlers answer it with its status and code.
///
/// ```ignore
///
/// async fn find(id: Id, _query: &FindQuery, state: &AppState) -> Result<Box<Item>> {
///     let item = state.items.get(&id).ok_or(octopux::Error::NotFound)?;
///     Ok(Box::new(item.clone()))
/// }
/// ```
///
/// Any other error answers 500 `INTERNAL_ERROR`, its message being logged instead of sent,
/// except the sqlx errors telling that the row is missing (404) or that a constraint of
/// the table is violated (400 or 409).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// 404 `ENTITY_NOT_FOUND`: no entity has this id
    NotFound,
    /// 400 `ID_MISMATCH`: the id of the payload differs from the id of the path
    IdMismatch,
    /// 400 `BAD_REQUEST`, the message telling the client what to fix
    BadRequest(String),
    /// 409 `CONFLICT`, the message telling which entity the request conflicts with
    Conflict(String),
}

impl Error {
    /// The `code` of the JSON body, stable for the clients to match on
    pub fn code(&self) -> &'static str {
        match self {
            Error::NotFound => "ENTITY_NOT_FOUND",
            Error::IdMismatch => "ID_MISMATCH",
            Error::BadRequest(_) => "BAD_REQUEST",
            Error::Conflict(_) => "CONFLICT",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFound => f.write_str("no entity has this id"),
            Error::IdMismatch => f.write_str("the payload id differs from the path id"),
            Error::BadRequest(message) | Error::Conflict(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for Error {}

impl ResponseError for Error {
    fn status_code(&self) -> StatusCode {
        match self {
            Error::NotFound => StatusCode::NOT_FOUND,
            Error::IdMismatch | Error::BadRequest(_) => StatusCode::BAD_REQUEST,
            Error::Conflict(_) => StatusCode::CONFLICT,
        }
    }

    fn error_response(&self) -> HttpResponse {
        json_error(self.status_code(), self.code(), self.to_string())
    }
}

/// The body of the error responses
#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: String,
}

pub(crate) const INTERNAL_ERROR: &str = "INTERNAL_ERROR";

fn json_error(status: StatusCode, code: &str, message: String) -> HttpResponse {
    HttpResponse::build(status).json(ErrorBody { code, message })
}

/// The response of an error returned by a model trait, see [`Error`].
pub(crate) fn error_response(err: anyhow::Error) -> HttpResponse {
    if let Some(error) = err.downcast_ref::<Error>() {
        return error.error_response();
    }
    #[cfg(feature = "sqlx")]
    if let Some(error) = err.downcast_ref::<sqlx::Error>().and_then(sqlx_error) {
        return error.error_response();
    }
    log::error!("{:#}", err);
    json_error(StatusCode::INTERNAL_SERVER_ERROR, INTERNAL_ERROR, "an internal error occurred".to_string())
}

/// The client errors among the sqlx errors. Their messages are generic, those of the database
/// naming its tables and constraints.
#[cfg(feature = "sqlx")]
fn sqlx_error(error: &sqlx::Error) -> Option<Error> {
    use sqlx::error::ErrorKind;

    match error {
        sqlx::Error::RowNotFound => Some(Error::NotFound),
        sqlx::Error::Database(db) => match db.kind() {
            ErrorKind::UniqueViolation => Some(Error::Conflict("a unique value is already taken".to_string())),
            ErrorKind::ForeignKeyViolation => {
                Some(Error::Conflict("the entity references a missing entity, or is referenced by another one".to_string()))
            }
            ErrorKind::NotNullViolation | ErrorKind::CheckViolation => {
                Some(Error::BadRequest("the payload violates a constraint of the table".to_string()))
            }
            _ => None,
        },
        _ => None,
    }
}
