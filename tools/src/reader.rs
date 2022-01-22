use std::{
    fs::File,
    io::{Seek, SeekFrom},
    path::Path,
};

use uplog::Record;

use crate::writer::CBORSequenceWriter;

/// 最低限満たすべき性質
pub trait StorageReader {
    /// メモリに確保する形式。省メモリにするためにWriterを渡すインターフェースにするのが望ましい
    fn read_at(&mut self, index: usize, len: usize) -> Result<Vec<Record>, std::io::Error>;
}

/// 単純なCBORSequenceFile
/// index情報など含まれていないので先頭から読むしかない
pub(crate) struct CBORSequenceReader {
    file: File,
}

impl CBORSequenceReader {
    #[allow(dead_code)]
    pub(crate) fn new<P: AsRef<Path>>(dirpath: P) -> Result<Self, std::io::Error> {
        let file = std::fs::File::open(dirpath.as_ref().join(CBORSequenceWriter::FILENAME))?;
        Ok(Self { file })
    }
}

impl StorageReader for CBORSequenceReader {
    fn read_at(&mut self, index: usize, len: usize) -> Result<Vec<Record>, std::io::Error> {
        // 先頭から読んで特定のindexから特定の長さのデータを読み出して返す
        debug_assert!(len > 0);
        let mut count: usize = 0;
        let mut result = Vec::with_capacity(len);
        self.file.seek(SeekFrom::Start(0))?;
        let iter = serde_cbor::Deserializer::from_reader(&self.file).into_iter::<Record>();
        for (i, v) in iter.enumerate() {
            if i >= index {
                if let Ok(v) = v {
                    result.push(v)
                } else {
                    println!("failed to read");
                }
                count += 1;
                if count >= len {
                    break;
                }
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use tempdir::TempDir;
    use uplog::{devlog, Level, Value};

    use crate::writer::{CBORSequenceWriter, RecordWriter};

    use super::{CBORSequenceReader, StorageReader};
    #[test]
    fn test_cbor_seq_read() -> std::io::Result<()> {
        uplog::session_init();
        let dir = TempDir::new("testdata")?;
        let file_path = dir.path();

        // make testdata
        let mut writer = CBORSequenceWriter::new(&file_path).unwrap();
        for i in 0..10 {
            let r = devlog!(Level::Info, "cat", &format!("nyan {}", i), "number", i);
            writer.push(&r)?;
        }
        drop(writer);

        let mut reader = CBORSequenceReader::new(&file_path)?;

        // check index
        for start in 0..10 {
            let data = reader.read_at(start, 10)?;
            assert_eq!(10 - start, data.len());
            if let Some(Value::U64(ref v)) = data[0].key_values().unwrap().get("number") {
                assert_eq!(start as u64, *v);
            }
        }
        // check len
        for len in 1..10 {
            let data = reader.read_at(0, len)?;
            assert_eq!(len, data.len());
        }

        Ok(())
    }
}
