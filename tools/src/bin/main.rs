use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use actix::prelude::*;

use actix_cors::Cors;
use actix_http::http::header;
use actix_web::{
    guard,
    web::{self, Data},
    App, Error, HttpRequest, HttpResponse, HttpServer,
};
use actix_web_actors::ws;
use async_graphql::{EmptyMutation, EmptySubscription, Schema};
use env_logger::Env;
use log::{debug, error, info};
use serde_cbor::{to_vec, Deserializer};
use structopt::StructOpt;
use uplog::Record;
use uplog_tools::{
    actor::StorageActor,
    webapi::{self, Query},
    Storage,
};
use uuid::Uuid;

// Handle http request
async fn ws_index(
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<StorageActor>>,
) -> Result<HttpResponse, Error> {
    let ip_addr: String = req
        .connection_info()
        .realip_remote_addr()
        .map(|x| String::from(x))
        .unwrap_or_else(|| String::from("unknown"));
    let actor =
        uplog_tools::actor::WsConn::new(Uuid::new_v4(), ip_addr, srv.get_ref().clone().recipient());
    let mut res = ws::handshake(&req)?;
    // デフォルトでは64KBのペイロードのため拡張する
    let codec = actix_http::ws::Codec::new().max_size(uplog::DEFAULT_BUFFER_SIZE);
    let out_stream = ws::WebsocketContext::with_codec(actor, stream, codec);
    let res = res.streaming(out_stream);
    Ok(res)
}

#[derive(Debug, PartialEq, StructOpt)]
struct Opt {
    #[structopt(long, short)]
    debug: bool,
    #[structopt(subcommand)]
    sub: Subcommands,
}

#[derive(Debug, PartialEq, StructOpt)]
enum Subcommands {
    /// log receive server
    Server(ServerOpt),
    /// dev tool
    Dev(DevOpt),
    /// read data dir and file
    Read(ReadOpt),
}

#[derive(Debug, PartialEq, StructOpt)]
struct ServerOpt {
    /// listen port
    #[structopt(long, short, default_value = "8040")]
    port: u16,
    #[structopt(long, short, default_value = "~/uplog", name = "DATA_DIR")]
    data_dir: String,
}

impl ServerOpt {
    fn get_data_dir(&self) -> Option<PathBuf> {
        if self.data_dir.is_empty() {
            return None;
        }
        if self.data_dir.starts_with("~/") {
            let data_local_dir = dirs::data_local_dir()?;
            Some(data_local_dir.join(&self.data_dir[2..]))
        } else {
            Some(PathBuf::from(&self.data_dir))
        }
    }
}

#[derive(Debug, PartialEq, StructOpt)]
struct DevOpt {
    #[structopt(
        long,
        default_value = "localhost",
        help = "connection host",
        name = "HOST"
    )]
    host: String,
    #[structopt(long, short, default_value = "8040", help = "listen port")]
    port: u16,
    #[structopt(long, short, default_value = "5", help = "data count")]
    count: u16,
    #[structopt(long, short, default_value = "1", name = "MILLISECONDS", parse(try_from_str = parse_milliseconds))]
    duration: Duration,
    #[structopt(short = "l", help = "call from macro interface")]
    use_log_macro: bool,
}

fn parse_milliseconds(src: &str) -> Result<Duration, std::num::ParseIntError> {
    let n = src.parse::<u64>()?;
    Ok(Duration::from_millis(n))
}

#[derive(Debug, PartialEq, StructOpt)]
struct ReadOpt {
    #[structopt(long, short, default_value = "tempdb", name = "DATA_DIR")]
    data_dir: String,
    /// read file
    #[structopt(name = "FILE")]
    file: Option<String>,
}

fn main() {
    let opt = Opt::from_args();

    if opt.debug {
        std::env::set_var("RUST_LOG", "debug");
    }
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    match opt.sub {
        Subcommands::Server(subopt) => {
            server(subopt.into()).unwrap();
        }
        Subcommands::Dev(subopt) => {
            if subopt.use_log_macro {
                client(subopt.into());
            } else {
                client_log_interface(subopt.into())
            }
        }
        Subcommands::Read(subopt) => {
            read(subopt.into());
        }
    };
}

struct ServerOption {
    port: u16,
    data_dir: PathBuf,
}

impl From<ServerOpt> for ServerOption {
    fn from(x: ServerOpt) -> Self {
        Self {
            port: x.port,
            data_dir: x.get_data_dir().expect("not found user local data dir"),
        }
    }
}

fn server(opt: ServerOption) -> std::io::Result<()> {
    let bind_addr = format!("0.0.0.0:{}", opt.port);
    let storage = uplog_tools::Storage::new(&opt.data_dir)?;
    info!("data store in [{}]", opt.data_dir.to_string_lossy());
    let mut rt = actix_web::rt::System::new("server");
    let schema = Schema::build(Query::new(storage.clone()), EmptyMutation, EmptySubscription).finish();

    rt.block_on(async move {
        // setup storage dir
        let storage_actor = uplog_tools::actor::StorageActor::new(storage);
        let storage_addr = storage_actor.start();

        info!("listen at {}", &bind_addr);
        HttpServer::new(move || {
            let cors = Cors::default()
                .allowed_origin_fn(|_origin, _req_head| true)
                .allowed_methods(vec!["GET", "POST"])
                .allowed_headers(vec![header::AUTHORIZATION, header::ACCEPT])
                .allowed_header(header::CONTENT_TYPE)
                .supports_credentials()
                .max_age(3600);
            App::new()
                .wrap(cors)
                // enable logger
                // .wrap(middleware::Logger::default())
                .data(storage_addr.clone())
                // websocket route
                .service(web::resource("/").route(web::get().to(ws_index)))
                // graphql
                .app_data(Data::new(schema.clone()))
                .service(
                    web::resource("/graphql")
                        .guard(guard::Post())
                        .to(webapi::index),
                )
                .service(
                    web::resource("/graphql")
                        .guard(guard::Get())
                        .to(webapi::index_playground),
                )
                .service(
                    actix_files::Files::new("/view", "./view/")
                        .prefer_utf8(true)
                        .index_file("index.html"),
                )
        })
        .bind(bind_addr)
        .unwrap()
        .run()
        .await
        .unwrap();
    });
    rt.run()
}

struct DevOption {
    host: String,
    port: u16,
    count: u16,
    delay: Duration,
}

impl DevOption {
    fn addr(&self) -> String {
        format!("ws://{}:{}/", self.host, self.port)
    }
}

impl From<DevOpt> for DevOption {
    fn from(x: DevOpt) -> Self {
        Self {
            host: x.host,
            port: x.port,
            count: x.count,
            delay: x.duration,
        }
    }
}

fn client(opt: DevOption) {
    use tungstenite::{connect, Message};
    uplog::devinit!();
    let url = opt.addr();
    let start = Instant::now();
    info!("send to {} length={}", &url, opt.count);
    let (mut client, _) = connect(&url).expect("failed to connect");

    for i in 0..opt.count {
        let record = uplog::devlog!(
            uplog::Level::Info,
            "uplog_server.bin.client",
            "send",
            "loop",
            i
        );
        let buf = to_vec(&record).expect("log format error");
        client
            .write_message(Message::binary(buf.as_slice()))
            .map_err(|e| error!("failed to send at: {}, {} ", i, e))
            .ok();
        debug!("send {}", i);
    }
    info!("finish. dur={:?}", start.elapsed());
}

fn client_log_interface(opt: DevOption) {
    uplog::Builder::default()
        .host(&opt.host)
        .port(opt.port)
        .try_init()
        .unwrap();
    let start = Instant::now();
    info!("send length={}", opt.count);

    for i in 0..opt.count {
        uplog::error!("uplog_server.bin.client", "send", "loop", i);
        debug!("send {}", i);
        std::thread::sleep(opt.delay)
    }
    info!("finish. dur={:?}", start.elapsed());
    uplog::flush();
}

struct ReadOption {
    data_dir: String,
    file: Option<String>,
}

impl From<ReadOpt> for ReadOption {
    fn from(x: ReadOpt) -> Self {
        Self {
            data_dir: x.data_dir,
            file: x.file,
        }
    }
}

fn read(opt: ReadOption) {
    let storage = Storage::new(opt.data_dir).unwrap();
    let mut records = storage.records().unwrap();

    match opt.file {
        Some(path) => {
            // TODO implment into library
            debug!("read file {}", path);
            let iter = records
                .into_iter()
                .filter(|x| x.path().to_str().unwrap().contains(&path));
            for i in iter {
                let f = i.open().unwrap();
                let reader = Deserializer::from_reader(f).into_iter::<Record>();
                for r in reader {
                    match r {
                        Ok(r) => println!("{}", r),
                        Err(e) => {
                            error!("failed to read record, {}", e);
                            return;
                        }
                    }
                }
            }
        }
        None => {
            records.sort_by(|a, b| a.created_at().cmp(b.created_at()));
            for r in records {
                println!("{}", r);
            }
        }
    };
}
