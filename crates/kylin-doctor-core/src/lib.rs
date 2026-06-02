pub mod detector;
pub mod detectors;

pub use detector::{Detector, Finding, FixAction, ScanReport, Severity};
pub use detectors::SystemDetector;
