/// docs
#[derive(Debug, Clone)]
pub struct UringOption {
    pub(crate) read: bool,
    pub(crate) write: bool,
    pub(crate) append: bool,
    pub(crate) truncate: bool,
    pub(crate) create: bool,
    pub(crate) create_new: bool,
    pub(crate) mode: libc::mode_t,
    pub(crate) custom_flags: libc::c_int,
}

impl UringOption {
    /// docs
    pub fn new() -> Self {
        Self {
            // generic
            // TODO: default should be false
            read: true,
            write: true,
            append: false,
            truncate: false,
            create: false,
            create_new: false,
            mode: 0o666,
            custom_flags: 0,
        }
    }
}
