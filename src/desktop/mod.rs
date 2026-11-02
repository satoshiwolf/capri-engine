mod hooks;
pub mod ledges;
pub mod monitors;
pub mod scanner;

pub use ledges::{Ledge, open_spans, scan_ledges};
pub use monitors::{MonitorInfo, can_cross_border, enumerate_monitors, virtual_desktop_bounds};
pub use scanner::{LedgeScanner, ledge_surfaces};
