mod auth;
mod errors;
mod routes;

use actix_web::{middleware, web, App, HttpServer, ResponseError};
use errors::PinkError;
use std::env;
#[cfg(not(feature = "error_reporting"))]
mod dummy_sentry;
#[cfg(not(feature = "error_reporting"))]
use dummy_sentry as sentry_actix;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env::set_var("RUST_LOG", "debug");
    env::set_var("RUST_BACKTRACE", "1");

    dotenv::dotenv().ok();

    env_logger::init();

    #[cfg(feature = "error_reporting")]
    let _guard = sentry::init((
        std::env::var("SENTRY_DSN").expect("SENTRY_DSN not set"),
        sentry::ClientOptions {
            release: sentry::release_name!(),
            ..Default::default()
        },
    ));

    let host = format!(
        "{}:{}",
        env::var("HOST").expect("HOST not set"),
        env::var("PORT").expect("PORT not set")
    );
    log::info!("starting PINK server at http://{}", &host);

    let server = HttpServer::new(move || {
        let logger =
            middleware::Logger::new("%{r}a \"%r\" %s %b \"%{Referer}i\" \"%{User-Agent}i\" %T");

        let json_config = web::JsonConfig::default()
            .limit(4096)
            .error_handler(|e, _rq| {
                PinkError::BadRequest {
                    message: e.to_string(),
                }
                .into()
            });

        let path_config = web::PathConfig::default().error_handler(|e, _rq| {
            log::debug!("path: {}", e);

            PinkError::BadRequest {
                message: e.to_string(),
            }
            .into()
        });

        App::new()
            .wrap(logger)
            .wrap(middleware::Condition::new(
                cfg!(feature = "error_reporting"),
                sentry_actix::Sentry::new(),
            ))
            .app_data(json_config)
            .app_data(path_config)
            .default_service(web::to(|| async {
                PinkError::NotFound {}.error_response()
            }))
            .configure(routes::config)
    })
    .bind(&host)?
    // FIXME: this fixes multiple instances of proxy maps for each process
    .workers(1)
    .run();

    server.await
}
