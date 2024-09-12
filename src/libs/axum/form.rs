use std::fmt::Debug;

use axum::{
    async_trait,
    body::Bytes,
    extract::{FromRequest, Request},
    response::{IntoResponse, Response},
};
use serde::{de::DeserializeOwned, Deserialize};
use serde_qs::Config;

use crate::modules::app::AppError;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Deserialize)]
pub struct Form<T>(pub T);

#[async_trait]
impl<S, T> FromRequest<S> for Form<T>
where
    Bytes: FromRequest<S>,
    S: Send + Sync,
    T: DeserializeOwned + Debug,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let body = Bytes::from_request(req, state)
            .await
            .map_err(IntoResponse::into_response)?;

        let config = Config::new(10, false);

        let form: T = config
            .deserialize_bytes(&body)
            .map_err(|err| AppError::from(anyhow::Error::new(err)).into_response())?;

        Ok(Form(form))
    }
}
