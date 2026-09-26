//! Domain model and ports for the fleet terminal application.

pub mod collector;
pub mod inventory;
pub mod model;
pub mod sample;

pub use collector::{CollectError, Collector};
pub use inventory::{Inventory, InventoryError};
pub use model::{HostSnapshot, HostTarget, Throughput, Usage};
pub use sample::{RateTracker, RawHostSample};
