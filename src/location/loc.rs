use core::convert::TryFrom;
use core::fmt;
use derive_try_from_primitive::TryFromPrimitive;
use serde::de::{SeqAccess, Visitor};
use serde::ser::{SerializeSeq, Serializer};
use serde::{de, Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, TryFromPrimitive)]
#[repr(u8)]
enum LocationType {
    LonLat = 1,
    Human = 2,
    WFW = 3,
}

/// Represents an location in various addressing schemes.
///
#[derive(Debug, Clone, PartialEq)]
pub enum Location {
    /// GPS coordinates
    LonLat((f32, f32)),
    /// Human-readable address
    Human(String),
    /// 3 word code geocode: https://3geonames.org/
    WFW(String),
}

impl Serialize for Location {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(2))?;
        match self {
            Location::LonLat(coords) => {
                seq.serialize_element(&(LocationType::LonLat as u8))?;
                seq.serialize_element(&coords)?;
            }
            Location::Human(address) => {
                seq.serialize_element(&(LocationType::Human as u8))?;
                seq.serialize_element(&address)?;
            }
            Location::WFW(address) => {
                seq.serialize_element(&(LocationType::WFW as u8))?;
                seq.serialize_element(&address)?;
            }
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for Location {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LocationVisitor;

        impl<'de> Visitor<'de> for LocationVisitor {
            type Value = Location;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("Location")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let loc_type: u8 = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let loc = LocationType::try_from(loc_type).map_err(|_err| {
                    de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(loc_type.into()),
                        &self,
                    )
                })?;
                match loc {
                    LocationType::LonLat => {
                        let coords: (f32, f32) = seq
                            .next_element()?
                            .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                        Ok(Location::LonLat(coords))
                    }
                    LocationType::Human => {
                        let address: String = seq
                            .next_element()?
                            .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                        Ok(Location::Human(address))
                    }
                    LocationType::WFW => {
                        let address: String = seq
                            .next_element()?
                            .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                        Ok(Location::WFW(address))
                    }
                }
            }
        }

        deserializer.deserialize_any(LocationVisitor)
    }
}

#[cfg(test)]
mod tests {
    use crate::location::Location;
    #[test]
    fn test_loc_lonlat_roundtrip() {
        let loc = Location::LonLat((23.0, 42.0));
        let buf = serde_cbor::to_vec(&loc).unwrap();
        let loc2 = serde_cbor::from_slice(&buf).unwrap();
        assert_eq!(loc, loc2);
    }

    #[test]
    fn test_loc_human_roundtrip() {
        let loc = Location::Human("Bahnhofstr 23, 12345 Nirgendwo".into());
        let buf = serde_cbor::to_vec(&loc).unwrap();
        let loc2 = serde_cbor::from_slice(&buf).unwrap();
        assert_eq!(loc, loc2);
    }

    #[test]
    fn test_loc_wfw_roundtrip() {
        let loc = Location::WFW("SINKUT-MEIJER-BETSUKAI".into());
        let buf = serde_cbor::to_vec(&loc).unwrap();
        let loc2 = serde_cbor::from_slice(&buf).unwrap();
        assert_eq!(loc, loc2);
    }
}
