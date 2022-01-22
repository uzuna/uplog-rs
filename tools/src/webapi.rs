

use actix_web::{web, Responder};
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::{SessionInfo, Storage};

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
pub async fn storages(state: web::Data<WebState>) -> impl Responder {
    let storage = Storage::new(&state.data_dir).unwrap();
    let records = storage.records().unwrap();
    let sb: Vec<SessionViewInfo> = records.into_iter().map(|x| x.into()).collect();

    web::Json(sb)
}
