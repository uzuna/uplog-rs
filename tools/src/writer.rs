use std::{fs::OpenOptions, io::BufWriter, path::Path};

use uplog::Record;

pub(crate) trait RecordWriter {
    fn push(&mut self, record: &Record) -> Result<(), std::io::Error>;
    fn flush(&mut self) {}
}

/// CBORシーケンスライターはデータをただ直接に書き出す
pub(crate) struct CBORSequenceWriter {
    writer: Box<dyn std::io::Write>,
}

impl CBORSequenceWriter {
    #[allow(dead_code)]
    pub(crate) const FILENAME: &'static str = "seqdata";

    #[allow(dead_code)]
    pub(crate) fn new<P: AsRef<Path>>(dirpath: P) -> Result<Self, std::io::Error> {
        let f = OpenOptions::new()
            .create(true)
            .write(true)
            .open(dirpath.as_ref().join(Self::FILENAME))?;
        let writer = Box::new(BufWriter::new(f));
        Ok(Self { writer })
    }
}

impl RecordWriter for CBORSequenceWriter {
    fn push(&mut self, record: &Record) -> Result<(), std::io::Error> {
        use std::io::{Error, ErrorKind};
        serde_cbor::to_writer(&mut self.writer, record)
            .map_err(|e| Error::new(ErrorKind::BrokenPipe, format!("write error {}", e)))
    }
}
