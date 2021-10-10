use std::time::Duration;

use serde::{Deserialize, Serialize};

#[macro_use]
mod macros;
mod session;

pub use session::session_init;

/// 指定可能なログレベル
#[repr(usize)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize)]
pub enum Level {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

impl From<log::Level> for Level {
    fn from(x: log::Level) -> Self {
        match x {
            log::Level::Trace => Self::Trace,
            log::Level::Debug => Self::Debug,
            log::Level::Info => Self::Info,
            log::Level::Warn => Self::Warn,
            log::Level::Error => Self::Error,
        }
    }
}

/// logクレートと対応
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Metadata {
    level: Level,
    target: String,
}

impl Metadata {
    pub fn new(level: Level, target: String) -> Self {
        Self { level, target }
    }

    #[inline]
    pub fn level(&self) -> Level {
        self.level
    }
    #[inline]
    pub fn target(&self) -> &str {
        &self.target
    }
}

/// logクレートと対応 ログ記録単位
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Record {
    metadata: Metadata,
    #[serde(with = "duration")]
    elapsed: Duration,
    category: String,
    module_path: Option<String>,
    file: Option<String>,
    line: Option<u32>,
    // log variant
    message: String,
    // TODO adding
    // kv: Option<KV>,
}

impl Record {
    #[inline]
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    #[inline]
    pub fn level(&self) -> Level {
        self.metadata.level()
    }

    #[inline]
    pub fn target(&self) -> &str {
        self.metadata.target()
    }

    #[inline]
    pub fn module_path(&self) -> Option<&String> {
        self.module_path.as_ref()
    }

    #[inline]
    pub fn file(&self) -> Option<&String> {
        self.file.as_ref()
    }

    #[inline]
    pub fn line(&self) -> Option<u32> {
        self.line
    }

    // #[inline]
    // pub fn key_values(&self) -> Option<&KV> {
    //     self.kv.as_ref()
    // }
}

// durationは(デ)シリアライザが実装されていないのでmoduleで指定する
mod duration {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let de = Duration::deserialize(deserializer)?;
        Ok(de)
    }
}

#[doc(hidden)]
pub fn __build_record<'a>(
    level: Level,
    target: &'a str,
    category: &'a str,
    message: &'a str,
    module_path: &'static str,
    file: &'static str,
    line: u32,
) -> Record {
    let metadata = Metadata::new(level, target.into());
    Record {
        metadata,
        elapsed: session::elapsed(),
        category: category.into(),
        message: message.into(),
        module_path: Some(module_path.into()),
        file: Some(file.into()),
        line: Some(line),
    }
}

#[cfg(test)]
mod tests {
    use serde_cbor::{from_slice, to_vec};

    use crate::*;

    #[test]
    fn test_metadata() {
        let target = "xxx";

        let metadata = Metadata::new(Level::Info, target.to_string());
        let encoded = to_vec(&metadata).unwrap();
        let decoded: Metadata = from_slice(&encoded).unwrap();
        assert_eq!(metadata, decoded);
    }

    #[test]
    fn test_record() {
        init!();
        let record = devlog!(Level::Info, "test.category", "test_message");
        let encoded = to_vec(&record).unwrap();
        let decoded: Record = from_slice(&encoded).unwrap();
        assert_eq!(record, decoded);
    }
}
