pub(crate) trait ExitStatusSentinel: PartialEq {
    fn sentinel() -> Self;
}

impl ExitStatusSentinel for i32 {
    fn sentinel() -> Self {
        return -1;
    }
}

impl ExitStatusSentinel for i64 {
    fn sentinel() -> Self {
        return -1;
    }
}

pub fn check<T: ExitStatusSentinel>(value: T) -> Result<T, std::io::Error> {
    if value == T::sentinel() {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(value)
    }
}
