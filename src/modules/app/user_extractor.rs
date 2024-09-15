use std::convert::Infallible;

use axum::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::RequestPartsExt;

use crate::libs::gitlab_api::gitlab_api::Member;

#[derive(Clone)]
pub struct Authenticated(pub Member);

#[async_trait]
impl<S> FromRequestParts<S> for Authenticated
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut Parts,
        _: &S,
    ) -> Result<Self, Self::Rejection> {
        match parts.extract::<MaybeAuthenticated>().await {
            Ok(MaybeAuthenticated(Some(member))) => Ok(Authenticated(member)),

            _ => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to authenticate user",
            )),
        }
    }
}

impl std::ops::Deref for Authenticated {
    type Target = Member;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone)]
pub struct MaybeAuthenticated(pub Option<Member>);

#[async_trait]
impl<S> FromRequestParts<S> for MaybeAuthenticated
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        _: &S,
    ) -> Result<Self, Self::Rejection> {
        let member = parts.extensions.get::<Member>().cloned();

        Ok(MaybeAuthenticated(member))
    }
}

impl std::ops::Deref for MaybeAuthenticated {
    type Target = Option<Member>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
