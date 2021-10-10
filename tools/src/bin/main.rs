use actix::prelude::*;
use actix_web::{middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use log::{debug, error, info};
use uplog_tools::actor::StorageActor;
use uuid::Uuid;

// Handle http request
async fn ws_index(
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<StorageActor>>,
) -> Result<HttpResponse, Error> {
    debug!("{:?}", req);
    let res = ws::start(
        uplog_tools::actor::WsConn::new(Uuid::new_v4(), srv.get_ref().clone().recipient()),
        &req,
        stream,
    )?;
    Ok(res)
}

fn main() {
    let m = clap::App::new(clap::crate_name!())
        .version(clap::crate_version!())
        .author(clap::crate_authors!())
        .about(clap::crate_description!())
        .arg(
            clap::Arg::with_name("port")
                .short("p")
                .long("port")
                .help("listen port")
                .value_name("NUMBER")
                .default_value("9001")
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("debug")
                .long("debug")
                .help("debug flag"),
        )
        .subcommand(
            clap::SubCommand::with_name("server")
                .about("start logging server")
                .arg(
                    clap::Arg::with_name("data_dir")
                        .short("d")
                        .long("data_dir")
                        .value_name("DATA_DIR")
                        .default_value("tempdb"),
                ),
        )
        .subcommand(
            clap::SubCommand::with_name("client")
                .about("test client")
                .arg(
                    clap::Arg::with_name("host")
                        .short("h")
                        .long("host")
                        .value_name("HOST")
                        .default_value("localhost"),
                )
                .arg(
                    clap::Arg::with_name("count")
                        .short("c")
                        .long("count")
                        .value_name("COUNT")
                        .default_value("5"),
                ),
        )
        .get_matches();

    let port: u16 = m.value_of("port").unwrap().parse().unwrap();
    if m.is_present("debug") {
        std::env::set_var("RUST_LOG", "debug");
    }

    env_logger::init();

    match m.subcommand() {
        ("server", Some(sub_m)) => {
            let data_dir = sub_m.value_of("data_dir").unwrap().to_string();
            let opt = ServerOption { port, data_dir };
            server(opt).unwrap();
        }
        ("client", Some(sub_m)) => {
            let host = sub_m.value_of("host").unwrap().to_string();
            let count: u16 = sub_m.value_of("count").unwrap().parse().unwrap();
            let opt = ClientOption { host, port, count };
            client(opt);
        }
        (_, _) => {
            println!("{}", m.usage());
        }
    };
}

struct ServerOption {
    port: u16,
    data_dir: String,
}

fn server(opt: ServerOption) -> std::io::Result<()> {
    let bind_addr = format!("0.0.0.0:{}", opt.port);
    let storage = uplog_tools::Storage::new(opt.data_dir)?;
    let mut rt = actix_web::rt::System::new("server");

    rt.block_on(async move {
        // setup storage dir
        let storage_actor = uplog_tools::actor::StorageActor::new(storage);
        let storage_addr = storage_actor.start();

        info!("listen at {}", &bind_addr);
        HttpServer::new(move || {
            App::new()
                // enable logger
                .wrap(middleware::Logger::default())
                // websocket route
                .service(web::resource("/").route(web::get().to(ws_index)))
                .data(storage_addr.clone())
        })
        .bind(bind_addr)
        .unwrap()
        .run()
        .await
        .unwrap();
    });
    rt.run()
}

struct ClientOption {
    host: String,
    port: u16,
    count: u16,
}

impl ClientOption {
    fn addr(&self) -> String {
        format!("ws://{}:{}/", self.host, self.port)
    }
}

fn client(opt: ClientOption) {
    use serde_cbor::to_vec;
    use tungstenite::{connect, Message};
    uplog::init!();
    let url = opt.addr();
    let (mut client, _) = connect(&url).unwrap();

    for i in 0..opt.count {
        let record = uplog::devlog!(uplog::Level::Info, "uplog_server.bin.client", "send");
        let buf = to_vec(&record).unwrap();
        client
            .write_message(Message::binary(buf.as_slice()))
            .map_err(|e| error!("failed to send at: {}, {} ", i, e))
            .ok();
        debug!("send {}", i);
    }
}
