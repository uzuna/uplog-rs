#[macro_export(local_inner_macros)]
macro_rules! devlog {
    ($level:expr, $category:expr, $message:expr) => {{
        $crate::__build_record(
            $level,
            __log_module_path!(),
            $category,
            $message,
            __log_module_path!(),
            __log_file!(),
            __log_line!(),
        )
    }};
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
