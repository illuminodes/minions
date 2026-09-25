use std::fmt::Display;

/// Browser console output that exists only in debug builds.
pub struct DevConsole;

#[cfg(debug_assertions)]
impl DevConsole {
    pub fn log(message: impl Display) {
        web_sys::console::log_1(&message.to_string().into());
    }

    pub fn error(message: impl Display) {
        web_sys::console::error_1(&message.to_string().into());
    }
}

#[cfg(not(debug_assertions))]
impl DevConsole {
    pub fn log(_message: impl Display) {}

    pub fn error(_message: impl Display) {}
}
