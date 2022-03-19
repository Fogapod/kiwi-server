mod routes;
use std::env;

#[cfg(not(feature = "error_reporting"))]
mod dummy_sentry;
#[cfg(not(feature = "error_reporting"))]
use dummy_sentry as sentry_actix;

use actix_web::{middleware, App, HttpServer};

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

        App::new()
            .wrap(logger)
            .wrap(middleware::Condition::new(
                cfg!(feature = "error_reporting"),
                sentry_actix::Sentry::new(),
            ))
            .configure(routes::config)
    })
    .bind(&host)?
    .run();

    server.await
}
