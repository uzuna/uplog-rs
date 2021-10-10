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
macro_rules! init {
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

#[doc(hidden)]
#[macro_export]
macro_rules! kv_zip {
    // ({}) で囲まれているので最後に返す値が戻り値になる
    ($($k:expr, $v:expr),+) => ({
        // 1行出力の場合はkv交互に出すことが出来ない
        // (println!("item: {} {} {:?}, {:?}", $category, $message, ($($k),+), ($($v),+)))
        let mut bt = $crate::kv::KV::new();
        // $()で入れ子関係を合わせる必要がある
        $(
            bt.insert($k.to_string(), $crate::kv::Value::from($v));
        )*
        bt
    });
}
