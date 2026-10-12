use std::{collections::HashSet, sync::Arc};
use actix_web::{
    body::{BoxBody, MessageBody},
    dev::{ServiceRequest, ServiceResponse},
    http::header::AUTHORIZATION,
    middleware::Next,
    web, HttpMessage, HttpRequest, HttpResponse,
};
use async_graphql::{
    extensions::{Extension, ExtensionContext, ExtensionFactory, NextResolve, ResolveInfo},
    ServerError, ServerResult, Value,
};
use jsonwebtoken::{DecodingKey, Validation};
use serde::Deserialize;


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role { User, Admin }   // order matters for `>=` in guards

// The claims of the JWT, `exp` being checked by the default validation
#[derive(Debug, Deserialize)]
struct Claims {
    sub: String,
    role: Role,
}

// The authenticated user, put in the data of the request when its token is valid
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: String,
    pub role: Role,
}

// The key verifying the HS256 signature of the tokens
pub struct JwtKey(DecodingKey);

impl JwtKey {
    pub fn from_secret(secret: &[u8]) -> Self {
        Self(DecodingKey::from_secret(secret))
    }

    // Reads `Authorization: Bearer <token>`, a missing or invalid token leaving the request anonymous
    pub fn authenticate(&self, req: &HttpRequest) -> Option<AuthUser> {
        let token = req.headers().get(AUTHORIZATION)?.to_str().ok()?.strip_prefix("Bearer ")?;
        let claims = jsonwebtoken::decode::<Claims>(token, &self.0, &Validation::default()).ok()?.claims;
        Some(AuthUser { id: claims.sub, role: claims.role })
    }
}

// Refuses the REST routes it wraps to the requests without a valid token (401), the user of the
// token being put in the extensions of the request. Without a `JwtKey` in the app data, every request is refused.
pub async fn require_token(req: ServiceRequest, next: Next<impl MessageBody + 'static>) -> Result<ServiceResponse<BoxBody>, actix_web::Error> {
    let user = req.app_data::<web::Data<JwtKey>>().and_then(|key| key.authenticate(req.request()));
    match user {
        Some(user) => {
            req.extensions_mut().insert(user);
            Ok(next.call(req).await?.map_into_boxed_body())
        }
        None => Ok(req.into_response(
            HttpResponse::Unauthorized()
                .content_type("application/json")
                .body(r#"{"code":"UNAUTHORIZED","message":"a valid bearer token is required"}"#),
        )),
    }
}


// Refuses the root fields of the schema to anonymous requests, except the public ones
pub struct RequireAuth {
    public: Arc<HashSet<(&'static str, &'static str)>>, // (parent type, field)
}

impl RequireAuth {
    pub fn new(public: &[(&'static str, &'static str)]) -> Self {
        Self { public: Arc::new(public.iter().copied().collect()) }
    }
}

impl ExtensionFactory for RequireAuth {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(RequireAuthExt { public: self.public.clone() })
    }
}

struct RequireAuthExt {
    public: Arc<HashSet<(&'static str, &'static str)>>,
}

#[async_trait::async_trait]
impl Extension for RequireAuthExt {
    async fn resolve(
        &self,
        ctx: &ExtensionContext<'_>,
        info: ResolveInfo<'_>,
        next: NextResolve<'_>,
    ) -> ServerResult<Option<Value>> {
        // Only the root fields are guarded, the nested ones being reachable through them only.
        // `__schema` and `__type` are root fields like the others, public only when listed
        let is_root = info.path_node.parent.is_none();

        if is_root
            && !info.is_for_introspection
            && !self.public.contains(&(info.parent_type, info.name))
            && ctx.data_opt::<AuthUser>().is_none()
        {
            let mut err = ServerError::new("Unauthorized", None);
            err.path = vec![async_graphql::PathSegment::Field(info.name.to_string())];
            return Err(err);
        }

        next.run(ctx, info).await
    }
}
