//! The drawn runtime: what every platform Skene draws whole runs beneath its realization.
//!
//! Each node's style is presented on the platform's frame clock, its transitions and
//! keyframe animations laid over it ([`BoxMotion`], [`RunMotion`]).

mod motion;

pub use self::motion::{BoxMotion, Presented, RunMotion, Sample};
