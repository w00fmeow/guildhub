use crate::modules::app::user_extractor::MaybeAuthenticated;
use askama_axum::IntoResponse;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{Redirect, Response};

pub async fn require_auth(
    MaybeAuthenticated(member): MaybeAuthenticated,
    request: Request,
    next: Next,
) -> Response {
    if member.is_none() {
        return Redirect::to("/login").into_response();
    }

    return next.run(request).await;
}
