use bp7::canonical::{new_canonical_block, CanonicalBlockType, CanonicalData};
use bp7::{CanonicalBlock, EndpointID};
use derive_try_from_primitive::TryFromPrimitive;
use serde::de::{SeqAccess, Visitor};
use serde::ser::{SerializeSeq, Serializer};
use serde::{de, Deserialize, Deserializer, Serialize};
use std::convert::TryFrom;
use std::fmt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PriorityError {
    #[error("serde cbor error: {0}")]
    Cbor(#[from] serde_cbor::Error),
    #[error("failed to create endpoint: {0}")]
    EndpointIdInvalid(#[from] bp7::eid::EndpointIdError),
    #[error("invalid endpoint supplied")]
    InvalidEndpoint,
    #[error("payload missing")]
    PayloadMissing,
    #[error("invalid priority block")]
    InvalidPriorityBlock,
}

// HOP_COUNT_BLOCK is a BlockType for a Hop Count block as defined in
// section 4.3.3.
pub const PRIORITY_BLOCK: CanonicalBlockType = 224;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriorityBlockData(u16);

pub fn new_priority_block(block_number: u64, data: PriorityBlockData) -> CanonicalBlock {
    new_canonical_block(
        PRIORITY_BLOCK,
        block_number,
        0,
        CanonicalData::Unknown(serde_cbor::to_vec(&data).unwrap_or_default()),
    )
}

pub fn get_priority_data(cblock: &CanonicalBlock) -> Result<PriorityBlockData, PriorityError> {
    if cblock.block_type == PRIORITY_BLOCK {
        if let CanonicalData::Unknown(data) = cblock.data() {
            serde_cbor::from_slice(data).map_err(|_err| PriorityError::InvalidPriorityBlock)
        } else {
            Err(PriorityError::InvalidPriorityBlock)
        }
    } else {
        Err(PriorityError::InvalidPriorityBlock)
    }
}

#[cfg(test)]
mod tests {
    use crate::priority::{
        get_priority_data, new_priority_block, PriorityBlockData,
    };
    use bp7::bundle::Block;
    use bp7::EndpointID;
    use std::convert::TryFrom;

    #[test]
    fn test_priority_roundtrip() {
        let data = PriorityBlockData(23);
        let buf = serde_cbor::to_vec(&data).unwrap();
        let data2 = serde_cbor::from_slice(&buf).unwrap();
        assert_eq!(data, data2);
    }

    #[test]
    fn test_cblock_priority_roundtrip() {
        let data = PriorityBlockData(42);

        let cblock = new_priority_block(1, data.clone());
        let buf = cblock.to_cbor();
        let cblock2 = serde_cbor::from_slice(&buf).unwrap();
        assert_eq!(cblock, cblock2);
        let data2 = get_priority_data(&cblock2).unwrap();
        assert_eq!(data, data2);
    }
}
