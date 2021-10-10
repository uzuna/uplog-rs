use std::{
    sync::Once,
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};

static mut SESSION: Option<SesstionInfo> = None;
static INIT: Once = Once::new();

/// 1 Recordの一意性のための時刻と経過時間を記録する
pub(crate) struct SesstionInfo {
    start_at: DateTime<Utc>,
    instant: Instant,
}

impl SesstionInfo {
    fn new() -> Self {
        Self {
            start_at: Utc::now(),
            instant: Instant::now(),
        }
    }
}

#[doc(hidden)]
pub fn session_init() {
    unsafe {
        INIT.call_once(|| {
            SESSION = Some(SesstionInfo::new());
        });
    }
}

pub(crate) fn elapsed() -> Duration {
    unsafe { SESSION.as_ref().unwrap().instant.elapsed() }
}

pub fn start_at() -> DateTime<Utc> {
    unsafe { SESSION.as_ref().unwrap().start_at }
}

#[cfg(test)]
mod tests {
    use crate::session::{elapsed, session_init, start_at};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_session() {
        session_init();
        let start_duration = elapsed();
        let start_time = start_at();
        thread::sleep(Duration::from_millis(1));
        assert!(elapsed() > start_duration);
        assert_eq!(start_time, start_at())
    }
}
