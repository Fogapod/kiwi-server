mod ip;
mod proxy;

use actix_web::web;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/ip").configure(ip::config));
    cfg.service(web::scope("/proxy").configure(proxy::config));
}
