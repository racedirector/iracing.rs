/// IBT replay file provider.
pub mod ibt;

/// Live shared-memory provider (Windows only).
#[cfg(windows)]
#[cfg_attr(docsrs, doc(cfg(windows)))]
pub mod live;
