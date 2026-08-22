#[derive(Default)]
pub(super) struct AuthorityCopyBudget {
    pub(super) entries: usize,
    pub(super) bytes: u64,
    pub(super) full_copy_bytes: u64,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct AuthorityDirectoryEntryIdentity {
    pub(super) name: std::ffi::OsString,
    pub(super) kind: u8,
    pub(super) device: u64,
    pub(super) inode: u64,
    pub(super) size: u64,
    pub(super) mode: u32,
    pub(super) modified_seconds: i64,
    pub(super) modified_nanoseconds: i64,
}
