use bp7::*;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SmsError {
    #[error("message not utf8: {0}")]
    NonUtf8(#[from] std::string::FromUtf8Error),
    #[error("serde cbor error: {0}")]
    Cbor(#[from] serde_cbor::Error),
    #[error("failed to decompress message: {0}")]
    SmazDecompress(#[from] smaz::DecompressError),
    #[error("failed to create endpoint: {0}")]
    EndpointIdInvalid(#[from] bp7::eid::EndpointIdError),
    #[error("SMS is missing message text")]
    NoMessage,
    #[error("invalid endpoint supplied")]
    InvalidEndpoint,
    #[error("payload missing")]
    PayloadMissing,
    #[error("invalid sms bundle")]
    InvalidSmsBundle,
}

fn smaz_compress(indata: &[u8]) -> Vec<u8> {
    smaz::compress(indata)
}

fn smaz_decompress(indata: &[u8]) -> Result<Vec<u8>, SmsError> {
    Ok(smaz::decompress(indata)?)
}

#[derive(Debug, PartialEq, Clone)]
pub struct SMSBundle(Bundle);

impl TryFrom<Bundle> for SMSBundle {
    type Error = SmsError;

    fn try_from(value: Bundle) -> Result<Self, Self::Error> {
        let sms_bundle = SMSBundle(value);
        if sms_bundle.is_valid().is_err() {
            Err(SmsError::InvalidSmsBundle)
        } else {
            Ok(sms_bundle)
        }
    }
}

impl SMSBundle {
    fn is_eid_valid(&self, eid: &EndpointID) -> Result<(), SmsError> {
        match eid {
            EndpointID::Ipn(_, ipn) => {
                if ipn.service_number() == 767 {
                    Ok(())
                } else {
                    Err(SmsError::InvalidEndpoint)
                }
            }
            EndpointID::Dtn(_, ssp) => {
                if ssp.ends_with("/sms") {
                    Ok(())
                } else {
                    Err(SmsError::InvalidEndpoint)
                }
            }
            _ => Err(SmsError::InvalidEndpoint),
        }
    }
    fn is_valid(&self) -> Result<(), SmsError> {
        self.is_eid_valid(&self.0.primary.source)?;
        self.is_eid_valid(&self.0.primary.destination)?;

        // Validate general payload
        let payload = self.0.payload().ok_or(SmsError::PayloadMissing)?;
        let sms: SMS = serde_cbor::from_slice(&payload)?;

        // Validate payload message and compression
        if sms.comp {
            String::from_utf8(smaz_decompress(&sms.msg)?)?;
        } else {
            String::from_utf8(sms.msg)?;
        }
        Ok(())
    }
    pub fn id(&self) -> String {
        self.0.id()
    }
    pub fn is_pure(&self, scheme: &str) -> bool {
        self.0.primary.source.scheme() == scheme && self.0.primary.destination.scheme() == scheme
    }
    pub fn src_ipn(&self) -> u64 {
        match &self.0.primary.source {
            EndpointID::Ipn(_, addr) => addr.node_number(),
            _ => 0,
        }
    }
    pub fn dst_ipn(&self) -> u64 {
        match &self.0.primary.destination {
            EndpointID::Ipn(_, addr) => addr.node_number(),
            _ => 0,
        }
    }
    pub fn src(&self) -> Option<String> {
        self.0.primary.source.node()
    }
    pub fn dst(&self) -> Option<String> {
        self.0.primary.destination.node()
    }
    pub fn creation_timestamp(&self) -> &CreationTimestamp {
        &self.0.primary.creation_timestamp
    }
    pub fn sms(&self) -> SMS {
        let payload = self.0.payload().expect("missing payload in bundle");

        serde_cbor::from_slice(&payload).expect("error decoding sms payload")
    }
    pub fn compression(&self) -> bool {
        self.sms().compression()
    }
    pub fn encryption(&self) -> bool {
        self.sms().encryption()
    }
    pub fn signature(&self) -> Option<Vec<u8>> {
        self.sms().signature()
    }
    pub fn msg(&self) -> String {
        self.sms().msg()
    }
    pub fn bundle(&self) -> &Bundle {
        &self.0
    }

    pub fn to_cbor(&mut self) -> Vec<u8> {
        self.0.to_cbor()
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct SMS {
    comp: bool,
    enc: bool,
    msg: Vec<u8>,
    sig: Option<Vec<u8>>,
}

impl SMS {
    pub fn compression(&self) -> bool {
        self.comp
    }
    pub fn encryption(&self) -> bool {
        self.enc
    }
    pub fn signature(&self) -> Option<Vec<u8>> {
        if let Some(sig) = self.sig.clone() {
            Some(sig)
        } else {
            None
        }
    }
    pub fn msg(&self) -> String {
        if self.compression() {
            String::from_utf8_lossy(&smaz_decompress(&self.msg).expect("decompressing msg failed"))
                .to_string()
        } else {
            String::from_utf8_lossy(&self.msg).to_string()
        }
    }
}

pub struct SmsBuilder {
    comp: bool,
    enc: bool,
    msg: Option<String>,
    sig: Option<Vec<u8>>,
}

impl SmsBuilder {
    pub fn new() -> Self {
        SmsBuilder {
            comp: true,
            enc: false,
            msg: None,
            sig: None,
        }
    }
    pub fn compression(mut self, comp: bool) -> Self {
        self.comp = comp;
        self
    }
    pub fn encryption(mut self, enc: bool) -> Self {
        self.enc = enc;
        self
    }
    pub fn message(mut self, msg: &str) -> Self {
        self.msg = Some(msg.into());
        self
    }
    pub fn signature(mut self, sig: Vec<u8>) -> Self {
        self.sig = Some(sig);
        self
    }
    pub fn build(self) -> Result<SMS, SmsError> {
        if let Some(msg) = self.msg {
            let msg_bytes = if self.comp {
                smaz_compress(msg.as_bytes())
            } else {
                msg.as_bytes().to_vec()
            };
            Ok(SMS {
                comp: self.comp,
                enc: self.enc,
                msg: msg_bytes,
                sig: self.sig,
            })
        } else {
            Err(SmsError::NoMessage)
        }
    }
}
/// Create a new sms bundle for IPN addressing scheme
pub fn new_sms(src: u64, dst: u64, msg: &str, compression: bool) -> Result<SMSBundle, SmsError> {
    let src_eid = EndpointID::with_ipn(src, 767)?;
    let dst_eid = EndpointID::with_ipn(dst, 767)?;

    let pblock = primary::PrimaryBlockBuilder::default()
        .destination(dst_eid)
        .source(src_eid)
        .report_to(EndpointID::none())
        .creation_timestamp(CreationTimestamp::now())
        .lifetime(Duration::from_secs(60 * 60))
        .build()
        .unwrap();

    let payload = SmsBuilder::new()
        .compression(compression)
        .message(msg)
        .build()?;
    let cblocks = vec![canonical::new_payload_block(
        0,
        serde_cbor::to_vec(&payload).expect("Fatal failure, could not convert sms payload to CBOR"),
    )];

    Ok(SMSBundle::try_from(bundle::Bundle::new(pblock, cblocks))
        .expect("error creating sms bundle"))
}
