/**
 * @file domain/mod.rs
 * @brief Pure core domain layer defining business entities, heuristic algorithms, and ports (DIP traits).
 */

pub mod entities;
pub mod heuristics;
pub mod ports;

pub use entities::*;
pub use heuristics::*;
pub use ports::*;
