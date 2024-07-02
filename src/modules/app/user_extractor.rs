use actix_web::error::InternalError;
use actix_web::http::header;
use actix_web::FromRequest;
use actix_web::HttpMessage;
use actix_web::HttpResponse;
use futures::future::ready;
use futures::future::Ready;

use crate::libs::gitlab_api::gitlab_api::Member;

pub struct Authenticated(pub Member);

impl FromRequest for Authenticated {
    type Error = actix_web::Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(
        req: &actix_web::HttpRequest,
        _payload: &mut actix_web::dev::Payload,
    ) -> Self::Future {
        let value = req.extensions().get::<Member>().cloned();

        let result = match value {
            Some(user) => Ok(Authenticated(user)),
            None => {
                let response = HttpResponse::Found()
                    .insert_header((header::LOCATION, "/login"))
                    .finish();

                Err(InternalError::from_response("Failed to decode token", response).into())
            }
        };

        ready(result)
    }
}

impl std::ops::Deref for Authenticated {
    type Target = Member;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
