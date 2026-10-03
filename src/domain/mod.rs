//! # Core Domain Layer
//!
//! Pure, framework-independent business models, heuristic algorithms, and abstract ports.
//!
//! In accordance with Clean Architecture and the Dependency Inversion Principle (DIP),
//! this layer has zero dependencies on external I/O, process execution, or GUI frameworks.
//!
//! Submodules:
//! - [`entities`]: Core domain entities representing DVDs, titles, metadata, and rip jobs.
//! - [`heuristics`]: Pure business logic (PAL speedup, cluster ranking, TV episode detection).
//! - [`ports`]: Inverted trait contracts implemented by the infrastructure layer.

pub mod entities;
pub mod heuristics;
pub mod ports;

pub use entities::*;
pub use heuristics::*;
pub use ports::*;
