pub mod actor;
mod writer;

use std::{
    fmt::Display,
    fs::{File, OpenOptions},
    io,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use log::debug;

/// ログファイルの配置を管理する
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

pub struct SessionInfo {
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    path: PathBuf,
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
