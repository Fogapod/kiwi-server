mod routes;

#[cfg(not(feature = "error_reporting"))]
mod dummy_sentry;
#[cfg(not(feature = "error_reporting"))]
use dummy_sentry as sentry_actix;

use actix_web::{middleware, App, HttpServer};

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "debug");
    std::env::set_var("RUST_BACKTRACE", "1");

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
    .bind("0.0.0.0:8000")?
    .run();

    server.await
}
