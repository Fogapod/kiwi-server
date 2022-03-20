use actix_web::{http::header, HttpRequest};

use constant_time_eq::constant_time_eq;

use crate::errors::PinkError;

pub fn authorize(rq: &HttpRequest) -> Result<(), PinkError> {
    let auth_header =
        rq.headers()
            .get(header::AUTHORIZATION)
            .ok_or_else(|| PinkError::Unauthorized {
                message: "missing Authorization header".into(),
            })?;

    let token = auth_header
        .to_str()
        .map_err(|_| PinkError::Unauthorized {
            message: "bad Authorization header".into(),
        })?
        .strip_prefix("Bearer ")
        .ok_or_else(|| PinkError::Unauthorized {
            message: "bad Bearer token format".into(),
        })?;

    // TODO: is it expensive? move to state?
    let master_token = std::env::var("ACCESS_TOKEN").expect("ACCESS_TOKEN not set");

    if !constant_time_eq(token.as_bytes(), master_token.as_bytes()) {
        return Err(PinkError::Unauthorized {
            message: "bad token".into(),
        });
    }

    Ok(())
}
