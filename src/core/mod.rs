#[cfg(feature = "network-monitoring")]
pub mod network;
pub mod segments;
pub mod statusline;

pub use statusline::{collect_all_segments, StatusLineGenerator};
