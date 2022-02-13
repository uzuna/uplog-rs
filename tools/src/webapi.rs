use crate::{
    reader::{CBORSequenceReader, StorageReader},
    LogRecord, SessionInfo, Storage,
};
use actix_web::HttpRequest;
use actix_web::{web, HttpResponse, Result};
use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql::{
    scalar, EmptyMutation, EmptySubscription, InputObject, Object, Schema, SimpleObject,
};
use async_graphql_actix_web::{Request, Response};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct DateTimeScalar(DateTime<Utc>);
scalar!(DateTimeScalar, "DateTime");

/// GraphQL Schema
pub type ApiSchema = Schema<Query, EmptyMutation, EmptySubscription>;

/// GraphQL Endpoint
pub async fn index(schema: web::Data<ApiSchema>, req: Request) -> Response {
    schema.execute(req.into_inner()).await.into()
}

/// GraphQL PlayGround
pub async fn index_playground(req: HttpRequest) -> Result<HttpResponse> {
    let source = playground_source(
        GraphQLPlaygroundConfig::new(req.path()).subscription_endpoint(req.path()),
    );
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(source))
}

#[derive(SimpleObject)]
struct SessionViewInfo {
    created_at: DateTimeScalar,
    updated_at: DateTimeScalar,
    name: String,
}

impl From<SessionInfo> for SessionViewInfo {
    fn from(x: SessionInfo) -> Self {
        Self {
            created_at: DateTimeScalar(x.created_at),
            updated_at: DateTimeScalar(x.updated_at),
            name: x.path().file_name().unwrap().to_string_lossy().to_string(),
        }
    }
}

#[derive(Debug)]
pub struct Query {
    storage: Storage,
}

impl Query {
    pub fn new(storage: Storage) -> Self {
        Self{storage}
    }
}

#[Object]
impl Query {
    async fn storages(&self) -> Result<Vec<SessionViewInfo>, std::io::Error> {
        let mut records: Vec<SessionInfo> = self.storage.records()?;
        records.sort_by(|a, b| b.created_at().cmp(a.created_at()));

        let record = records.into_iter().map(SessionViewInfo::from).collect();
        Ok(record)
    }

    async fn storage_read_at(&self, vars: ReadAtVars) -> Result<Vec<LogRecord>, std::io::Error> {
        let records = self.storage.records().unwrap();
        let target: Vec<SessionInfo> = records
            .into_iter()
            .filter(|x| x.path().to_str().unwrap().contains(&vars.name))
            .collect();
        if target.is_empty() {
            return Ok(Vec::new());
        }
        let session = &target[0];
        let mut reader: CBORSequenceReader = session.open().unwrap().into();
        reader.read_at(vars.start.unwrap_or(0), vars.length.unwrap_or(100))
    }
}

#[derive(InputObject)]
struct ReadAtVars {
    name: String,
    start: Option<usize>,
    length: Option<usize>,
}
