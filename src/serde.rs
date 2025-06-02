pub mod base64_or_bytes {
    use std::borrow::Borrow;

    use base64::{display::Base64Display, Engine};
    use serde::{Deserializer, Serializer};

    #[allow(clippy::ptr_arg)]
    pub fn serialize<S: Serializer>(v: &Vec<u8>, s: S) -> Result<S::Ok, S::Error> {
        if s.is_human_readable() {
            s.collect_str(
                Base64Display::new(v, &base64::engine::general_purpose::STANDARD).borrow(),
            )
        } else {
            serde_bytes::serialize(v, s)
        }
    }
    /*
    pub fn deserialize<'de: 'a, 'a, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        if d.is_human_readable() {
            let base64 = String::deserialize(d)?;
            base64::decode(base64.as_bytes()).map_err(serde::de::Error::custom)
        } else {
            serde_bytes::deserialize(d)
        }
    }*/
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        if d.is_human_readable() {
            struct Visitor;
            impl<'de> serde::de::Visitor<'de> for Visitor {
                type Value = Vec<u8>;

                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    formatter.write_str("a string")
                }

                fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                where
                    E: serde::de::Error,
                {
                    self.visit_bytes(v.as_ref())
                }

                fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
                where
                    E: serde::de::Error,
                {
                    self.visit_bytes(v.as_ref())
                }

                fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
                where
                    E: serde::de::Error,
                {
                    Engine::decode(&base64::engine::general_purpose::STANDARD, v)
                        .map_err(serde::de::Error::custom)
                }

                fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
                where
                    E: serde::de::Error,
                {
                    self.visit_bytes(v.as_ref())
                }
            }

            d.deserialize_str(Visitor)
        } else {
            serde_bytes::deserialize(d)
        }
    }
}
