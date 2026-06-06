//! Bearer token validation and principal resolution (local token or HS256 JWT).

use crate::db::Db;
use crate::error::{ApiError, ApiResult};
use axum::{
    async_trait,
    extract::{FromRequestParts, Request, State},
    http::{header, request::Parts, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use houston_engine_protocol::{ErrorBody, ErrorCode, ErrorDetail};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use crate::config::AuthMode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String,
    pub email: Option<String>,
    pub role: Option<String>,
    pub exp: usize,
}

#[derive(Debug, Clone)]
pub struct Principal {
    pub user_id: Uuid,
    pub email: Option<String>,
    pub org_id: Uuid,
    pub org_role: String,
}

#[derive(Clone)]
pub struct AuthState {
    pub mode: AuthMode,
    pub jwt_secret: Option<String>,
    pub local_token: String,
    pub local_user_id: Uuid,
    pub local_email: Option<String>,
    pub db: Db,
}

pub async fn require_auth(
    State(state): State<Arc<crate::state::AppState>>,
    mut req: Request,
    next: Next,
) -> Response {
    let token = match extract_bearer(&req) {
        Some(t) => t,
        None => return unauthorized(),
    };
    let principal = match validate_and_resolve(&state.auth, &token).await {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };
    req.extensions_mut().insert(principal);
    next.run(req).await
}

pub async fn validate_and_resolve(auth: &AuthState, token: &str) -> ApiResult<Principal> {
    match auth.mode {
        AuthMode::Local => resolve_local(auth, token).await,
        AuthMode::Jwt => resolve_jwt(auth, token).await,
    }
}

async fn resolve_local(auth: &AuthState, token: &str) -> ApiResult<Principal> {
    if !local_token_matches(token, &auth.local_token) {
        return Err(ApiError::unauthorized("Invalid bearer token"));
    }
    let (org_id, org_role) = auth
        .db
        .ensure_user_org(auth.local_user_id, auth.local_email.as_deref())
        .await?;
    Ok(Principal {
        user_id: auth.local_user_id,
        email: auth.local_email.clone(),
        org_id,
        org_role,
    })
}

async fn resolve_jwt(auth: &AuthState, token: &str) -> ApiResult<Principal> {
    let secret = auth
        .jwt_secret
        .as_deref()
        .ok_or_else(|| ApiError::internal("JWT auth is not configured"))?;
    let claims = decode_jwt(token, secret)?;
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| ApiError::unauthorized("Invalid user id in token"))?;
    let (org_id, org_role) = auth.db.ensure_user_org(user_id, claims.email.as_deref()).await?;
    Ok(Principal {
        user_id,
        email: claims.email,
        org_id,
        org_role,
    })
}

fn local_token_matches(provided: &str, expected: &str) -> bool {
    use subtle::ConstantTimeEq;
    if provided.len() != expected.len() {
        return false;
    }
    provided.as_bytes().ct_eq(expected.as_bytes()).into()
}

fn decode_jwt(token: &str, secret: &str) -> ApiResult<JwtClaims> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    let data = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| ApiError::unauthorized(format!("Invalid bearer token: {e}")))?;
    if data.claims.role.as_deref() == Some("anon") {
        return Err(ApiError::unauthorized("Anonymous tokens are not allowed"));
    }
    Ok(data.claims)
}

fn extract_bearer(req: &Request) -> Option<String> {
    if let Some(v) = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
    {
        return Some(v.to_string());
    }
    // Browsers cannot set Authorization on WebSocket — match engine auth.
    if let Some(v) = req
        .headers()
        .get("sec-websocket-protocol")
        .and_then(|v| v.to_str().ok())
    {
        for p in v.split(',').map(str::trim) {
            if let Some(t) = p.strip_prefix("houston-bearer.") {
                return Some(t.to_string());
            }
        }
    }
    if let Some(q) = req.uri().query() {
        for kv in q.split('&') {
            if let Some(t) = kv.strip_prefix("token=") {
                return Some(urlencoding_decode(t));
            }
        }
    }
    None
}

fn urlencoding_decode(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(ErrorBody {
            error: ErrorDetail {
                code: ErrorCode::Unauthorized,
                message: "Missing or invalid bearer token".into(),
                details: None,
            },
        }),
    )
        .into_response()
}

pub fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(token.as_bytes());
    let out = h.finalize();
    out.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn principal(req: &Request) -> ApiResult<Principal> {
    req.extensions()
        .get::<Principal>()
        .cloned()
        .ok_or_else(|| ApiError::unauthorized("Missing auth context"))
}

#[async_trait]
impl<S> FromRequestParts<S> for Principal
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Principal>()
            .cloned()
            .ok_or_else(|| ApiError::unauthorized("Missing auth context"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use houston_engine_protocol::ErrorCode;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use uuid::Uuid;

    const SECRET: &str = "test-jwt-secret-at-least-32-bytes!!";

    fn mint_token(claims: JwtClaims) -> String {
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(SECRET.as_bytes()),
        )
        .expect("jwt encode")
    }

    fn sample_claims(role: Option<&str>, exp: usize) -> JwtClaims {
        JwtClaims {
            sub: Uuid::new_v4().to_string(),
            email: Some("user@example.com".into()),
            role: role.map(str::to_string),
            exp,
        }
    }

    #[test]
    fn decode_jwt_accepts_authenticated_role() {
        let exp = (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp() as usize;
        let token = mint_token(sample_claims(Some("authenticated"), exp));
        let claims = decode_jwt(&token, SECRET).expect("valid token");
        assert_eq!(claims.role.as_deref(), Some("authenticated"));
    }

    #[test]
    fn decode_jwt_rejects_anon_role() {
        let exp = (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp() as usize;
        let token = mint_token(sample_claims(Some("anon"), exp));
        let err = decode_jwt(&token, SECRET).unwrap_err();
        assert_eq!(err.status, StatusCode::UNAUTHORIZED);
        assert_eq!(err.code, ErrorCode::Unauthorized);
        assert!(err.message.contains("Anonymous"));
    }

    #[test]
    fn decode_jwt_rejects_wrong_secret() {
        let exp = (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp() as usize;
        let token = mint_token(sample_claims(Some("authenticated"), exp));
        assert!(decode_jwt(&token, "wrong-secret").is_err());
    }

    #[test]
    fn extract_bearer_from_authorization_header() {
        let req = Request::builder()
            .header(header::AUTHORIZATION, "Bearer abc.def.ghi")
            .body(Body::empty())
            .unwrap();
        assert_eq!(extract_bearer(&req).as_deref(), Some("abc.def.ghi"));
    }

    #[test]
    fn extract_bearer_missing_returns_none() {
        let req = Request::builder().body(Body::empty()).unwrap();
        assert!(extract_bearer(&req).is_none());
    }

    #[test]
    fn local_token_matches_rejects_wrong_token() {
        assert!(!super::local_token_matches("wrong", "expected"));
        assert!(super::local_token_matches("same", "same"));
    }

    #[test]
    fn extract_bearer_from_websocket_protocol() {
        let req = Request::builder()
            .header("sec-websocket-protocol", "houston-bearer.ws-token")
            .body(Body::empty())
            .unwrap();
        assert_eq!(extract_bearer(&req).as_deref(), Some("ws-token"));
    }
}
