//! Domain model and ports for the fleet terminal application.

pub mod collector;
pub mod inventory;
pub mod model;
pub mod sample;

pub use collector::{CollectError, Collector};
pub use inventory::{Inventory, InventoryError};
pub use model::{
    ContainerSnapshot, HostSnapshot, HostTarget, ProcessSnapshot, StorageEntry, StorageSnapshot,
    Throughput, Usage,
};
pub use sample::{RateTracker, RawHostSample, RawProcessSample};
