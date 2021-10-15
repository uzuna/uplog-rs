use std::{fmt::Display, time::Duration};

use serde::{Deserialize, Serialize};

#[macro_use]
mod macros;
mod buffer;
mod client;
mod kv;
mod logger;
mod session;

pub use {
    client::{
        try_init, try_init_with_builder, try_init_with_host, Builder, DEFAULT_BUFFER_SIZE,
        WS_DEFAULT_PORT,
    },
    kv::{KVBorrow, Value, ValueBorrow, KV},
    logger::{flush, Log},
    session::session_init,
    session::start_at,
};

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
    pub metadata: Metadata,
    #[serde(with = "duration")]
    pub elapsed: Duration,
    pub category: String,
    pub module_path: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub message: String,
    pub kv: Option<KV>,
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

    #[inline]
    pub fn key_values(&self) -> Option<&KV> {
        self.kv.as_ref()
    }
}

impl Display for Record {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:?}] {:.4} [{}] {} ({}:L{})",
            self.level(),
            self.elapsed.as_secs_f64(),
            self.category,
            self.message,
            self.file().unwrap_or(&"".into()),
            self.line().unwrap_or(0)
        )?;
        if let Some(ref kv) = self.kv {
            write!(f, " {{")?;
            for k in kv.keys() {
                if let Some(v) = kv.get(k) {
                    write!(f, "{} = {}, ", k, v)?;
                } else {
                    write!(f, "{} = ?, ", k)?;
                }
            }
            write!(f, "}}")?;
        }
        Ok(())
    }
}

/// 借用型のメタデータ ログ生成に使う
/// 初期化時に設定する情報
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize)]
pub struct MetadataBorrow<'a> {
    level: Level,
    target: &'a str,
}

impl<'a> MetadataBorrow<'a> {
    #[inline]
    pub fn new(level: Level, target: &'a str) -> Self {
        Self { target, level }
    }

    #[inline]
    pub fn default() -> Self {
        Self {
            level: Level::Info,
            target: "",
        }
    }

    #[inline]
    pub fn level(&self) -> Level {
        self.level
    }
    #[inline]
    pub fn target(&self) -> &'a str {
        self.target
    }
}

impl Default for MetadataBorrow<'_> {
    fn default() -> Self {
        Self {
            level: Level::Info,
            target: "",
        }
    }
}

/// 借用型のログデータ ログ生成に使う
///
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RecordBorrow<'a> {
    metadata: MetadataBorrow<'a>,
    // log detail
    #[serde(with = "duration")]
    elapsed: Duration,
    category: &'a str,
    module_path: Option<&'a str>,
    file: Option<&'a str>,
    line: Option<u32>,
    // log variant
    message: &'a str,
    kv: Option<KVBorrow<'a>>,
}

impl<'a> RecordBorrow<'a> {
    #[inline]
    pub fn metadata(&self) -> &MetadataBorrow<'a> {
        &self.metadata
    }

    #[inline]
    pub fn level(&self) -> Level {
        self.metadata.level()
    }

    #[inline]
    pub fn target(&self) -> &'a str {
        self.metadata.target()
    }

    #[inline]
    pub fn module_path(&self) -> Option<&'a str> {
        self.module_path
    }

    #[inline]
    pub fn file(&self) -> Option<&'a str> {
        self.file
    }

    #[inline]
    pub fn line(&self) -> Option<u32> {
        self.line
    }

    #[inline]
    pub fn key_values(&self) -> Option<&'a KVBorrow> {
        self.kv.as_ref()
    }
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
#[allow(clippy::too_many_arguments)]
pub fn __build_record<'a>(
    level: Level,
    target: &'a str,
    category: &'a str,
    message: &'a str,
    module_path: &'static str,
    file: &'static str,
    line: u32,
    kv: Option<KV>,
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
        kv,
    }
}

#[doc(hidden)]
#[allow(clippy::too_many_arguments)]
pub fn __encode_log<'a>(
    level: Level,
    target: &'a str,
    category: &'a str,
    message: &'a str,
    module_path: &'static str,
    file: &'static str,
    line: u32,
    kv: Option<KVBorrow>,
    buf: &mut [u8],
) {
    let metadata = MetadataBorrow::new(level, target);
    let r = RecordBorrow {
        metadata,
        elapsed: session::elapsed(),
        category,
        message,
        module_path: Some(module_path),
        file: Some(file),
        line: Some(line),
        kv,
    };
    serde_cbor::to_writer(buf, &r).unwrap();
}

#[doc(hidden)]
#[allow(clippy::too_many_arguments)]
pub fn __log_api<'a>(
    level: Level,
    target: &'a str,
    category: &'a str,
    message: &'a str,
    module_path: &'static str,
    file: &'static str,
    line: u32,
    kv: Option<KVBorrow>,
) {
    let metadata = MetadataBorrow::new(level, target);

    logger::logger().log(&RecordBorrow {
        metadata,
        elapsed: session::elapsed(),
        category,
        message,
        module_path: Some(module_path),
        file: Some(file),
        line: Some(line),
        kv,
    });
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
        devinit!();
        let record = devlog!(Level::Info, "test.category", "test_message");
        let encoded = to_vec(&record).unwrap();
        let decoded: Record = from_slice(&encoded).unwrap();
        assert_eq!(record, decoded);
    }

    #[test]
    fn test_record_kv() {
        devinit!();
        let record = devlog!(
            Level::Info,
            "test.category",
            "test_message",
            "u8",
            42_u8,
            "i64",
            i64::MIN,
            "property",
            "alice"
        );
        let encoded = to_vec(&record).unwrap();
        let decoded: Record = from_slice(&encoded).unwrap();
        assert_eq!(record, decoded);

        // check display format
        let actual = format!("{}", &record);
        let expect = r#"{i64 = -9223372036854775808, property = "alice", u8 = 42, }"#;
        assert!(actual.contains("[Info]"));
        assert!(actual.contains("[test.category] test_message (uplog/src/lib.rs"));
        assert!(actual.contains(expect));
    }
}
