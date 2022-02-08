use std::str::FromStr;

use actix_cors::Cors;
use actix_http::http::header;
use actix_web::web::Data;
use actix_web::{guard, web, App, HttpResponse, HttpServer, Result};
use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql::{EmptyMutation, EmptySubscription, Object, Schema, SimpleObject, Enum, Scalar, ScalarType, scalar};
use async_graphql_actix_web::{Request, Response};
use chrono::{Utc, DateTime};
use serde::{Serialize, Deserialize};

#[derive(SimpleObject)]
struct Dummy {
    name: String,
    id: usize,
}

impl Default for Dummy {
    fn default() -> Self {
        Self {
            name: String::from("it is dummy"),
            id: 21,
        }
    }
}

#[derive(SimpleObject)]
struct PageOfDummys {
    start: usize,
    len: usize,
    data: Vec<Dummy>
}

impl PageOfDummys {
    fn new(start: usize, len: usize, data: Vec<Dummy>) -> Self {
        Self {start, len, data}
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
enum Mode {
    S1,
    S2
}

// 自力で実装しなくてもserdeをかぶせたらいい感じにしてくれる
#[derive(Debug, Serialize, Deserialize)]
struct DateTimeScalar(chrono::DateTime<chrono::Utc>);
scalar!(DateTimeScalar, "DateTime");


#[derive(SimpleObject)]
struct ModeInfo{
    mode: Mode,
    ts: DateTimeScalar,
}


struct Query;

#[Object]
impl Query {
    async fn answer(&self) -> usize {
        42
    }
    async fn double(&self, value: isize) -> isize {
        value * 2
    }
    async fn doublef(&self, value: f32) -> f32 {
        value * 2.0
    }
    async fn mylist(&self) -> &[f32] {
        &[0.1, 2.0, 3.0]
    }
    async fn dummy(&self) -> Dummy {
        Dummy::default()
    }
    async fn dummys(&self, start: usize, len: usize) -> PageOfDummys {
        let data = vec![
            Dummy::default(),
            Dummy {
                name: String::from("expect"),
                id: 90,
            },
        ];
        PageOfDummys::new(start, data.len(), data)
    }

    async fn checkmode(&self) -> ModeInfo {
        ModeInfo{mode: Mode::S1, ts: DateTimeScalar(Utc::now())}
    }

    async fn qts(&self, ts: DateTimeScalar) -> bool {
        println!("{:?}", ts);
        true
    }
}

type ApiSchema = Schema<Query, EmptyMutation, EmptySubscription>;

async fn index(schema: web::Data<ApiSchema>, req: Request) -> Response {
    schema.execute(req.into_inner()).await.into()
}

async fn index_playground() -> Result<HttpResponse> {
    let source = playground_source(GraphQLPlaygroundConfig::new("/").subscription_endpoint("/"));
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(source))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let schema = Schema::build(Query, EmptyMutation, EmptySubscription).finish();

    println!("Playground: http://localhost:8000");

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
            .app_data(Data::new(schema.clone()))
            .service(web::resource("/").guard(guard::Post()).to(index))
            .service(web::resource("/").guard(guard::Get()).to(index_playground))
    })
    .bind("127.0.0.1:8000")?
    .run()
    .await
}
