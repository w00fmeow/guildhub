use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::error::InternalError;
use actix_web::http::header;
use actix_web::{web, HttpMessage, HttpResponse};
use futures::future::Ready;
use futures_util::future::{ready, LocalBoxFuture};
use futures_util::FutureExt;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll};
use tracing::{debug, error};

use crate::libs::gitlab_api::gitlab_api::Member;

use super::app::App;

pub struct AuthMiddleware<S> {
    service: Rc<S>,
}

impl<S> Service<ServiceRequest> for AuthMiddleware<S>
where
    S: Service<
            ServiceRequest,
            Response = ServiceResponse<actix_web::body::BoxBody>,
            Error = actix_web::Error,
        > + 'static,
{
    type Response = ServiceResponse<actix_web::body::BoxBody>;
    type Error = actix_web::Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, actix_web::Error>>;

    fn poll_ready(&self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let token = req.cookie("token").map(|c| c.value().to_string());

        if token.is_none() {
            let response = HttpResponse::Found()
                .insert_header((header::LOCATION, "/login"))
                .finish();

            return Box::pin(ready(Err(InternalError::from_response(
                "Missing token",
                response,
            )
            .into())));
        }

        let app_state = req.app_data::<web::Data<Arc<App>>>().unwrap();

        let token_claims = match app_state.auth_service.decode_token(&token.unwrap()) {
            Ok(claims) => claims,
            Err(e) => {
                error!("{e}");

                let response = HttpResponse::Found()
                    .insert_header((header::LOCATION, "/login"))
                    .finish();

                return Box::pin(ready(Err(InternalError::from_response(
                    "Failed to decode token",
                    response,
                )
                .into())));
            }
        };

        let cloned_app_state = app_state.clone();
        let srv = Rc::clone(&self.service);

        async move {
            let member = cloned_app_state
                .gitlab_service
                .get_cached_member(&token_claims.sub)
                .await;

            if member.is_none() {
                let response = HttpResponse::Found()
                    .insert_header((header::LOCATION, "/login"))
                    .finish();

                return Err(InternalError::from_response("User was not found", response).into());
            }

            req.extensions_mut().insert::<Member>(member.unwrap());

            let res = srv.call(req).await?;

            match cloned_app_state
                .auth_service
                .refresh_token_if_needed(token_claims)
            {
                Ok(Some(new_token)) => {
                    debug!("set the new token to cookie: {}", new_token);
                }
                _ => {}
            };

            Ok(res)
        }
        .boxed_local()
    }
}

pub struct RequireAuth;

impl<S> Transform<S, ServiceRequest> for RequireAuth
where
    S: Service<
            ServiceRequest,
            Response = ServiceResponse<actix_web::body::BoxBody>,
            Error = actix_web::Error,
        > + 'static,
{
    type Response = ServiceResponse<actix_web::body::BoxBody>;
    type Error = actix_web::Error;
    type Transform = AuthMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddleware {
            service: Rc::new(service),
        }))
    }
}
