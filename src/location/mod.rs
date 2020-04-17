mod loc;

pub use loc::Location;

use bitflags::bitflags;
use derive_try_from_primitive::TryFromPrimitive;
use serde::{Deserialize, Serialize};

bitflags! {
    // Results in default value with bits: 0
    #[derive(Default)]
    struct NodeTypeFlags: u16 {
        /// Indicates that this node is a mobile device, moving over time, e.g., UAV, smartphone or satellite
        /// If this flag is not set then the node is considered stationary, e.g., immobile infrastructure such as an access point
        const MOBILE = 0b00000001;
        /// Node does not listen for incoming bundles, e.g., wireless sensor node periodically reporting data
        const PURESENDER = 0b00000010;
        /// This node is a gateway connecting different networks
        const GW = 0b00000100;
        /// This node has a working Internet uplink
        const INTERNET = 0b00001000;
        /// Indicates that this node is battery powered
        const BATTERY = 0b00010000;
    }
}

#[derive(Debug, Clone, PartialEq, TryFromPrimitive, Serialize, Deserialize)]
#[repr(u8)]
pub enum LocationBlockType {
    Position = 1,
    FenceEllipse = 2,
    FenceRect = 3,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LocationBlockData {
    Position(Location),
    FenceEllipse(Location, u64, u64),
    FenceRect(Location, Location),
}
