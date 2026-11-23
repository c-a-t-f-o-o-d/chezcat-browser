//! Shared source admission and work constants used by CSS and the browser host.
pub(crate) const STYLESHEET_DOCUMENT_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const SOURCE_WORK_CHUNK: usize = 64;
pub(crate) const SOURCE_WORK_PER_CHUNK: usize = 512;
pub(crate) const FULL_STYLESHEET_SOURCE_WORK: usize =
    STYLESHEET_DOCUMENT_BYTES / SOURCE_WORK_CHUNK * SOURCE_WORK_PER_CHUNK;
