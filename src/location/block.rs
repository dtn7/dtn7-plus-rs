use super::{Location, NodeTypeFlags};
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
pub enum LocationError {
    #[error("serde cbor error: {0}")]
    Cbor(#[from] serde_cbor::Error),
    #[error("failed to create endpoint: {0}")]
    EndpointIdInvalid(#[from] bp7::eid::EndpointIdError),
    #[error("invalid endpoint supplied")]
    InvalidEndpoint,
    #[error("payload missing")]
    PayloadMissing,
    #[error("invalid location block")]
    InvalidLocationBlock,
}

#[derive(Debug, Clone, PartialEq, TryFromPrimitive, Serialize, Deserialize)]
#[repr(u8)]
pub enum LocationBlockType {
    Position = 1,
    FenceEllipse = 2,
    FenceRect = 3,
    Trace = 4,
}

// HOP_COUNT_BLOCK is a BlockType for a Hop Count block as defined in
// section 4.3.3.
pub const LOCATION_BLOCK: CanonicalBlockType = 223;

#[derive(Debug, Clone, PartialEq)]
pub enum LocationBlockData {
    Position(NodeTypeFlags, Location),
    FenceEllipse(Location, u64, u64),
    FenceRect(Location, Location),
    Trace(NodeTypeFlags, EndpointID, Location),
}

impl Serialize for LocationBlockData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            LocationBlockData::Position(info, coords) => {
                let mut seq = serializer.serialize_seq(Some(3))?;
                seq.serialize_element(&(LocationBlockType::Position as u8))?;
                seq.serialize_element(&info.bits())?;
                seq.serialize_element(&coords)?;
                seq.end()
            }
            LocationBlockData::FenceEllipse(coords, r1, r2) => {
                let mut seq = serializer.serialize_seq(Some(4))?;
                seq.serialize_element(&(LocationBlockType::FenceEllipse as u8))?;
                seq.serialize_element(&coords)?;
                seq.serialize_element(r1)?;
                seq.serialize_element(r2)?;
                seq.end()
            }
            LocationBlockData::FenceRect(topleft, bottomright) => {
                let mut seq = serializer.serialize_seq(Some(3))?;
                seq.serialize_element(&(LocationBlockType::FenceRect as u8))?;
                seq.serialize_element(&topleft)?;
                seq.serialize_element(&bottomright)?;
                seq.end()
            }
            LocationBlockData::Trace(info, node, coords) => {
                let mut seq = serializer.serialize_seq(Some(4))?;
                seq.serialize_element(&(LocationBlockType::Trace as u8))?;
                seq.serialize_element(&info.bits())?;
                seq.serialize_element(&node)?;
                seq.serialize_element(&coords)?;
                seq.end()
            }
        }
    }
}
impl<'de> Deserialize<'de> for LocationBlockData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LocationBlockDataVisitor;

        impl<'de> Visitor<'de> for LocationBlockDataVisitor {
            type Value = LocationBlockData;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("LocationBlockData")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let loc_type: u8 = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let loc = LocationBlockType::try_from(loc_type).map_err(|_err| {
                    de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(loc_type.into()),
                        &self,
                    )
                })?;
                match loc {
                    LocationBlockType::Position => {
                        let info: NodeTypeFlags = NodeTypeFlags::from_bits(
                            seq.next_element()?
                                .ok_or_else(|| de::Error::invalid_length(1, &self))?,
                        )
                        .unwrap_or_default();
                        let coords: Location = seq
                            .next_element()?
                            .ok_or_else(|| de::Error::invalid_length(2, &self))?;
                        Ok(LocationBlockData::Position(info, coords))
                    }
                    LocationBlockType::FenceEllipse => {
                        let coords: Location = seq
                            .next_element()?
                            .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                        let r1: u64 = seq
                            .next_element()?
                            .ok_or_else(|| de::Error::invalid_length(2, &self))?;
                        let r2: u64 = seq
                            .next_element()?
                            .ok_or_else(|| de::Error::invalid_length(3, &self))?;

                        Ok(LocationBlockData::FenceEllipse(coords, r1, r2))
                    }
                    LocationBlockType::FenceRect => {
                        let topleft: Location = seq
                            .next_element()?
                            .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                        let bottomright: Location = seq
                            .next_element()?
                            .ok_or_else(|| de::Error::invalid_length(2, &self))?;
                        Ok(LocationBlockData::FenceRect(topleft, bottomright))
                    }
                    LocationBlockType::Trace => {
                        let info: NodeTypeFlags = NodeTypeFlags::from_bits(
                            seq.next_element()?
                                .ok_or_else(|| de::Error::invalid_length(1, &self))?,
                        )
                        .unwrap_or_default();
                        let node: EndpointID = seq
                            .next_element()?
                            .ok_or_else(|| de::Error::invalid_length(2, &self))?;
                        let coords: Location = seq
                            .next_element()?
                            .ok_or_else(|| de::Error::invalid_length(3, &self))?;
                        Ok(LocationBlockData::Trace(info, node, coords))
                    }
                }
            }
        }

        deserializer.deserialize_any(LocationBlockDataVisitor)
    }
}

pub fn new_location_block(block_number: u64, data: LocationBlockData) -> CanonicalBlock {
    new_canonical_block(
        LOCATION_BLOCK,
        block_number,
        0,
        CanonicalData::Unknown(serde_cbor::to_vec(&data).unwrap_or_default()),
    )
}

pub fn get_location_data(cblock: &CanonicalBlock) -> Result<LocationBlockData, LocationError> {
    if cblock.block_type == LOCATION_BLOCK {
        if let CanonicalData::Unknown(data) = cblock.data() {
            let loc_data =
                serde_cbor::from_slice(data).map_err(|_err| LocationError::InvalidLocationBlock);
            loc_data
        } else {
            Err(LocationError::InvalidLocationBlock)
        }
    } else {
        Err(LocationError::InvalidLocationBlock)
    }
}

#[cfg(test)]
mod tests {
    use crate::location::{
        get_location_data, new_location_block, Location, LocationBlockData, NodeTypeFlags,
    };
    use bp7::bundle::Block;
    use bp7::EndpointID;
    use std::convert::TryFrom;

    #[test]
    fn test_locblock_data_position_roundtrip() {
        let loc = Location::LatLon((23.0, 42.0));
        let data = LocationBlockData::Position(NodeTypeFlags::MOBILE, loc);
        let buf = serde_cbor::to_vec(&data).unwrap();
        let data2 = serde_cbor::from_slice(&buf).unwrap();
        assert_eq!(data, data2);
    }

    #[test]
    fn test_locblock_data_fence_ellipse_roundtrip() {
        let loc = Location::LatLon((23.0, 42.0));
        let data = LocationBlockData::FenceEllipse(loc, 10, 5);
        let buf = serde_cbor::to_vec(&data).unwrap();
        let data2 = serde_cbor::from_slice(&buf).unwrap();
        assert_eq!(data, data2);
    }

    #[test]
    fn test_locblock_data_fence_rect_roundtrip() {
        let loc = Location::LatLon((23.0, 42.0));
        let loc2 = Location::LatLon((42.0, 66.0));
        let data = LocationBlockData::FenceRect(loc, loc2);
        let buf = serde_cbor::to_vec(&data).unwrap();
        let data2 = serde_cbor::from_slice(&buf).unwrap();
        assert_eq!(data, data2);
    }
    #[test]
    fn test_locblock_data_trace_roundtrip() {
        let loc = Location::LatLon((23.0, 42.0));
        let data = LocationBlockData::Trace(
            NodeTypeFlags::MOBILE,
            EndpointID::try_from("dtn://node1").unwrap(),
            loc,
        );
        let buf = serde_cbor::to_vec(&data).unwrap();
        let data2 = serde_cbor::from_slice(&buf).unwrap();
        assert_eq!(data, data2);
    }

    #[test]
    fn test_cblock_location_roundtrip() {
        let loc = Location::LatLon((23.0, 42.0));
        let data = LocationBlockData::Position(NodeTypeFlags::MOBILE, loc);

        let cblock = new_location_block(1, data.clone());
        let buf = cblock.to_cbor();
        let cblock2 = serde_cbor::from_slice(&buf).unwrap();
        assert_eq!(cblock, cblock2);
        let data2 = get_location_data(&cblock2).unwrap();
        assert_eq!(data, data2);
    }
}
