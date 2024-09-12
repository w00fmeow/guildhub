use crate::libs::gitlab_api::gitlab_api::Member;
use crate::modules::app::controller::build_auth_cookie;
use axum::extract::{Request, State};
use axum::http::header::SET_COOKIE;
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;
use axum_extra::extract::cookie::CookieJar;
use std::sync::Arc;
use tracing::error;

use super::super::app::App;

pub async fn optional_auth(
    State(state): State<Arc<App>>,
    cookies: CookieJar,
    mut request: Request,
    next: Next,
) -> Response {
    let token = cookies.get("token").map(|c| c.value().to_string());

    if token.is_none() {
        return next.run(request).await;
    }

    let token_claims = match state.auth_service.decode_token(&token.unwrap()) {
        Ok(claims) => claims,
        Err(e) => {
            error!("{e}");
            return next.run(request).await;
        }
    };

    let member = state
        .gitlab_service
        .get_cached_member(&token_claims.sub)
        .await;

    if member.is_none() {
        return next.run(request).await;
    }

    request.extensions_mut().insert::<Member>(member.unwrap());

    let mut response = next.run(request).await;

    match state.auth_service.refresh_token_if_needed(token_claims) {
        Ok(Some(new_token)) => {
            let cookie = build_auth_cookie(&new_token);

            match HeaderValue::from_str(&cookie.to_string()) {
                Ok(cookie) => {
                    response.headers_mut().insert(SET_COOKIE, cookie);
                }
                Err(err) => {
                    error!("{err}")
                }
            }
        }
        _ => {}
    };

    return response;
}
