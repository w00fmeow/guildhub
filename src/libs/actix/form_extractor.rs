use actix_web::error::ErrorBadRequest;
use actix_web::error::ErrorInternalServerError;
use actix_web::web;
use actix_web::FromRequest;
use futures::Future;
use futures_util::stream::StreamExt;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_qs::Config;
use std::fmt::Debug;
use std::pin::Pin;
use tracing::debug;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Deserialize)]
pub struct Form<T>(pub T);

impl<T> FromRequest for Form<T>
where
    T: DeserializeOwned + Debug,
{
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(
        _req: &actix_web::HttpRequest,
        payload: &mut actix_web::dev::Payload,
    ) -> Self::Future {
        let mut payload = payload.take();

        Box::pin(async move {
            let mut body = web::BytesMut::new();

            while let Some(chunk) = payload.next().await {
                let chunk = chunk.map_err(ErrorInternalServerError)?;
                body.extend_from_slice(&chunk);
            }

            let config = Config::new(10, false);

            let form: T = config.deserialize_bytes(&body).map_err(ErrorBadRequest)?;

            Ok(Form(form))
        })
    }
}

impl<T> std::ops::Deref for Form<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
