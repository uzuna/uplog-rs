use actix::prelude::*;
use actix_web::{middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use log::debug;
use uplog_server::actor::StorageActor;
use uuid::Uuid;

// Handle http request
async fn ws_index(
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<StorageActor>>,
) -> Result<HttpResponse, Error> {
    debug!("{:?}", req);
    let res = ws::start(
        uplog_server::actor::WsConn::new(Uuid::new_v4(), srv.get_ref().clone().recipient()),
        &req,
        stream,
    )?;
    Ok(res)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    // setup storage dir
    let dir = "tempdb";
    let storage = uplog_server::Storage::new(dir)?;
    let storage_actor = uplog_server::actor::StorageActor::new(storage);
    let storage_addr = storage_actor.start();

    HttpServer::new(move || {
        App::new()
            // enable logger
            .wrap(middleware::Logger::default())
            // websocket route
            .service(web::resource("/").route(web::get().to(ws_index)))
            .data(storage_addr.clone())
    })
    .bind("0.0.0.0:9001")?
    .run()
    .await
}
