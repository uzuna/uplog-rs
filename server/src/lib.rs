mod writer;

use std::{
    io,
    path::{Path, PathBuf},
};

/// ログファイルの配置を管理する
struct Storage {
    /// 保存先ルート
    dir: PathBuf,
}

impl Storage {
    pub fn new<A: AsRef<Path>>(root_dir: A) -> io::Result<Self> {
        std::fs::create_dir(&root_dir)?;
        Ok(Self {
            dir: root_dir.as_ref().to_owned(),
        })
    }

    fn create_session(&self, name: &str) -> io::Result<Session> {
        let dirpath = self.dir.join(name);
        std::fs::create_dir(&dirpath)?;
        Session::new(dirpath)
    }
}

/// ある一連のログの書き込みを管理する
struct Session {
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

#[cfg(test)]
mod tests {
    use std::fs::File;

    use crate::{writer::RecordWriter, *};
    use serde_cbor::Deserializer;
    use tempdir::TempDir;
    use uplog::{devlog, init, Level, Record};

    #[test]
    fn test_storage_session() -> std::io::Result<()> {
        init!();
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
