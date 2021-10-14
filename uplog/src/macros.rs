/// log
#[macro_export(local_inner_macros)]
macro_rules! log {
    ($level:expr, $category:expr, $message:expr, $kv:expr) => {
        $crate::__log_api(
            $level,
            __log_module_path!(),
            $category,
            $message,
            __log_module_path!(),
            __log_file!(),
            __log_line!(),
            $kv,
        )
    };
    ($level:expr, $category:expr, $message:expr) => {
        log!($level, $category, $message, None)
    };
    ($level:expr, $category:expr, $message:expr, $($k:expr, $v:expr),+) => ({
        let kv = kv_zip!($($k, $v),*);
        log!($level, $category, $message, Some(kv))
    });
}

/// build record macro for development
#[macro_export(local_inner_macros)]
macro_rules! devlog {
    ($level:expr, $category:expr, $message:expr, $kv:expr) => {
        $crate::__build_record(
            $level,
            __log_module_path!(),
            $category,
            $message,
            __log_module_path!(),
            __log_file!(),
            __log_line!(),
            $kv,
        )
    };
    ($level:expr, $category:expr, $message:expr) => {
        devlog!($level, $category, $message, None)
    };
    ($level:expr, $category:expr, $message:expr, $($k:expr, $v:expr),+) => ({
        let kv = kv_zip!($($k, $v),*);
        devlog!($level, $category, $message, Some(kv))
    });
}

#[macro_export(local_inner_macros)]
macro_rules! devinit {
    () => {{
        $crate::session_init();
        $crate::start_at()
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __log_module_path {
    () => {
        module_path!()
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __log_file {
    () => {
        file!()
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __log_line {
    () => {
        line!()
    };
}

/// build KV
#[doc(hidden)]
#[macro_export]
macro_rules! kv_zip {
    ($($k:expr, $v:expr),+) => ({
        let mut bt = $crate::KV::new();
        $(
            bt.insert($k.to_string(), $crate::Value::from($v));
        )*
        bt
    });
}
