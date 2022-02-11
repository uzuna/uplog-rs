pub mod actor;
mod reader;
pub mod webapi;
mod writer;

use std::{
    fmt::Display,
    fs::{File, OpenOptions},
    io,
    path::{Path, PathBuf},
};

use async_graphql::{scalar, Enum, Object};
use chrono::{DateTime, Utc};
use log::debug;
use serde::{Deserialize, Serialize};
use uplog::{Level, Record, KV};

#[derive(Debug, Serialize)]
pub struct LogRecord {
    id: usize,
    record: Record,
}

impl LogRecord {
    pub fn new(id: usize, record: Record) -> Self {
        Self { id, record }
    }
}

#[Object]
impl LogRecord {
    async fn id(&self) -> usize {
        self.id
    }
    async fn record<'a>(&'a self) -> RecordObject<'a> {
        RecordObject(&self.record)
    }
}

struct RecordObject<'record>(&'record Record);

#[Object]
impl<'record> RecordObject<'record> {
    async fn level(&self) -> LogLevel {
        self.0.metadata.level().into()
    }
    async fn elapsed(&self) -> DurationScalar {
        DurationScalar(self.0.elapsed.as_secs_f64())
    }
    async fn category(&self) -> &str {
        &self.0.category
    }
    async fn message(&self) -> &str {
        &self.0.message
    }
    async fn module_path(&self) -> Option<&str> {
        if let Some(ref x) = self.0.module_path {
            Some(x)
        } else {
            None
        }
    }
    async fn file(&self) -> Option<&str> {
        if let Some(ref x) = self.0.file {
            Some(x)
        } else {
            None
        }
    }
    async fn line(&self) -> Option<&u32> {
        if let Some(ref x) = self.0.line {
            Some(x)
        } else {
            None
        }
    }
    async fn kv(&self) -> Option<KeyValue<'record>> {
        if let Some(ref kv) = self.0.kv {
            return Some(KeyValue(kv));
        }
        None
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<Level> for LogLevel {
    fn from(x: Level) -> Self {
        match x {
            Level::Trace => Self::Trace,
            Level::Debug => Self::Debug,
            Level::Info => Self::Info,
            Level::Warn => Self::Warn,
            Level::Error => Self::Error,
        }
    }
}

struct KeyValue<'record>(&'record KV);

#[Object]
impl<'record> KeyValue<'record> {
    async fn json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.0)
    }
}

// 自力で実装しなくてもserdeをかぶせたらいい感じにしてくれる
#[derive(Debug, Serialize, Deserialize)]
struct DurationScalar(f64);
scalar!(DurationScalar, "Duration");

/// ログファイルの配置を管理する
#[derive(Debug)]
pub struct Storage {
    /// 保存先ルート
    dir: PathBuf,
}

impl Storage {
    pub fn new<A: AsRef<Path>>(root_dir: A) -> io::Result<Self> {
        std::fs::create_dir_all(&root_dir)?;
        Ok(Self {
            dir: root_dir.as_ref().to_owned(),
        })
    }

    pub fn create_session(&self, name: &str) -> io::Result<Session> {
        let dirpath = self.dir.join(name);
        std::fs::create_dir_all(&dirpath).expect("failed to create storage dir");
        Session::new(dirpath)
    }

    pub fn records(&self) -> io::Result<Vec<SessionInfo>> {
        let rd = std::fs::read_dir(&self.dir)?;
        let vec = rd.fold(vec![], |mut a, v| {
            if let Ok(d) = v {
                let metadata = std::fs::metadata(d.path()).unwrap();
                let i = SessionInfo {
                    created_at: metadata.created().unwrap().into(),
                    updated_at: metadata.modified().unwrap().into(),
                    path: d.path(),
                };
                a.push(i);
            };
            a
        });
        Ok(vec)
    }
}

/// ある一連のログの書き込みを管理する
pub struct Session {
    writer: Box<dyn writer::RecordWriter>,
}

impl Session {
    fn new<A: AsRef<Path>>(dirpath: A) -> io::Result<Self> {
        let writer = writer::CBORSequenceWriter::new(dirpath.as_ref())?;
        Ok(Self {
            writer: Box::new(writer),
        })
    }
}

impl writer::RecordWriter for Session {
    fn push(&mut self, record: &uplog::Record) -> Result<(), std::io::Error> {
        self.writer.push(record)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.writer.flush()
    }
}

#[derive(Debug)]
pub struct SessionInfo {
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) path: PathBuf,
}

impl Display for SessionInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}, created: {}, updated: {}",
            self.path.file_name().unwrap().to_str().unwrap(),
            self.created_at,
            self.updated_at
        )
    }
}

impl SessionInfo {
    #[allow(dead_code)]
    const FILENAME: &'static str = "seqdata";
    pub fn open(&self) -> io::Result<File> {
        debug!("SessionInfo open: {}", self.filepath().to_str().unwrap());
        OpenOptions::new().read(true).open(self.filepath())
    }

    pub fn path(&self) -> &Path {
        self.path.as_ref()
    }

    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    fn filepath(&self) -> PathBuf {
        self.path.join(Self::FILENAME)
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use crate::{writer::RecordWriter, *};
    use serde_cbor::Deserializer;
    use tempdir::TempDir;
    use uplog::{devinit, devlog, Level, Record};

    #[test]
    fn test_storage_session() -> std::io::Result<()> {
        devinit!();
        let path = TempDir::new("storage").expect("create temp dir of storage");
        let dirpath = path.path().join("storage");
        let storage = Storage::new(&dirpath)?;
        let name = "00";

        // testdata
        let r = devlog!(Level::Info, "cat", "msg");

        // write record
        {
            let mut session = storage.create_session(name)?;
            session.push(&r)?;
            session.push(&r)?;
        }

        let iter = {
            let f = File::open(dirpath.join(name).join("seqdata"))?;
            Deserializer::from_reader(f).into_iter::<Record>()
        };
        let mut counter = 0;
        for v in iter {
            assert_eq!(&r, &v.unwrap());
            counter += 1;
        }
        assert_eq!(counter, 2);
        Ok(())
    }
}
