mod block;
mod loc;

pub use block::{get_location_data, new_location_block, LocationBlockData, LOCATION_BLOCK};
pub use loc::Location;

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

bitflags! {
    // Results in default value with bits: 0
    #[derive(Default, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
    pub struct NodeTypeFlags: u16 {
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
