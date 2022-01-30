use actix_web::{
    get,
    web::{self, Query},
    Responder,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uplog::Record;

use crate::{
    reader::{CBORSequenceReader, StorageReader},
    SessionInfo, Storage,
};

#[derive(Debug, Clone)]
pub struct WebState {
    pub data_dir: String,
}

#[derive(Serialize)]
pub struct SessionViewInfo {
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    name: String,
}

impl From<SessionInfo> for SessionViewInfo {
    fn from(x: SessionInfo) -> Self {
        Self {
            created_at: x.created_at,
            updated_at: x.updated_at,
            name: x.path().file_name().unwrap().to_string_lossy().to_string(),
        }
    }
}

#[get("/storages")]
pub async fn storages(state: web::Data<WebState>) -> impl Responder {
    let storage = Storage::new(&state.data_dir).unwrap();
    let mut records = storage.records().unwrap();
    records.sort_by(|a, b| b.created_at().cmp(a.created_at()));
    let sb: Vec<SessionViewInfo> = records.into_iter().map(|x| x.into()).collect();

    web::Json(sb)
}

#[derive(Deserialize)]
pub struct ReadAtQuery {
    start: Option<usize>,
    length: Option<usize>,
}

#[get("/storage/{name}")]
pub async fn storage_read(
    state: web::Data<WebState>,
    name: web::Path<String>,
    info: Query<ReadAtQuery>,
) -> impl Responder {
    let storage = Storage::new(&state.data_dir).unwrap();
    let records = storage.records().unwrap();
    let name = name.into_inner();
    let target: Vec<SessionInfo> = records
        .into_iter()
        .filter(|x| x.path().to_str().unwrap().contains(&name))
        .collect();
    if target.is_empty() {
        return web::Json(Vec::<Record>::new());
    }
    let session = &target[0];
    let mut reader: CBORSequenceReader = session.open().unwrap().into();
    let data = reader
        .read_at(info.start.unwrap_or(0), info.length.unwrap_or(100))
        .unwrap();
    web::Json(data)
}
