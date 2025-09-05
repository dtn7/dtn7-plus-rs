/// This protocol is inspired by the net news format ([RFC](https://datatracker.ietf.org/doc/html/rfc5536))
use bp7::flags::BlockControlFlags;
use bp7::*;
use core::fmt;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;
#[derive(Error, Debug)]
pub enum NewsError {
    #[error("bundle decoding error: {0}")]
    BundleDecoding(#[from] bp7::error::Error),
    #[error("message not utf8: {0}")]
    NonUtf8(#[from] std::string::FromUtf8Error),
    #[error("serde cbor error: {0}")]
    Cbor(#[from] serde_cbor::Error),
    #[error("failed to decompress message: {0}")]
    SmazDecompress(#[from] smaz::DecompressError),
    #[error("failed to create endpoint: {0}")]
    EndpointIdInvalid(#[from] bp7::eid::EndpointIdError),
    #[error("News is missing message text")]
    NoMessage,
    #[error("News is missing a topic")]
    NoTopic,
    #[error("invalid endpoint supplied")]
    InvalidEndpoint,
    #[error("payload missing")]
    PayloadMissing,
    #[error("invalid news bundle")]
    InvalidNewsBundle,
}

fn smaz_compress(indata: &[u8]) -> Vec<u8> {
    smaz::compress(indata)
}

fn smaz_decompress(indata: &[u8]) -> Result<Vec<u8>, NewsError> {
    Ok(smaz::decompress(indata)?)
}

#[derive(Debug, PartialEq, Clone)]
pub struct NewsBundle(Bundle);

impl TryFrom<Bundle> for NewsBundle {
    type Error = NewsError;

    fn try_from(value: Bundle) -> Result<Self, Self::Error> {
        let news_bundle = NewsBundle(value);
        if news_bundle.is_valid().is_err() {
            Err(NewsError::InvalidNewsBundle)
        } else {
            Ok(news_bundle)
        }
    }
}
impl TryFrom<Vec<u8>> for NewsBundle {
    type Error = NewsError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let bundle = Bundle::try_from(value.to_vec())?;
        let news_bundle = NewsBundle(bundle);
        if news_bundle.is_valid().is_err() {
            Err(NewsError::InvalidNewsBundle)
        } else {
            Ok(news_bundle)
        }
    }
}

impl fmt::Display for NewsBundle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "ID: {}", self.id())?;
        writeln!(f, "From: {}", self.src().unwrap_or_default())?;
        writeln!(f, "To: {}", self.dst().unwrap_or_default())?;
        writeln!(f, "Creation TS: {}", self.creation_timestamp())?;
        writeln!(f, "Thread ID: {}", self.tid())?;
        writeln!(f, "References: {:?}", self.references())?;
        writeln!(f, "Tags: {:?}", self.tags())?;
        writeln!(f, "Topic: {}", self.topic())?;
        writeln!(f, "\n{}", self.msg())
    }
}
enum EIDType {
    Src,
    Dst,
}
impl NewsBundle {
    fn is_eid_valid(&self, eid: &EndpointID, service: EIDType) -> Result<(), NewsError> {
        match eid {
            EndpointID::Ipn(_, ipn) => match service {
                EIDType::Src => {
                    if ipn.service_number() == 767 {
                        Ok(())
                    } else {
                        Err(NewsError::InvalidEndpoint)
                    }
                }
                EIDType::Dst => {
                    if ipn.service_number() == 119 {
                        Ok(())
                    } else {
                        Err(NewsError::InvalidEndpoint)
                    }
                }
            },
            EndpointID::Dtn(_, ssp) => match service {
                EIDType::Src => {
                    if ssp.service_name() == Some("sms") {
                        Ok(())
                    } else {
                        Err(NewsError::InvalidEndpoint)
                    }
                }
                EIDType::Dst => {
                    if ssp.service_name() == Some("~news") {
                        Ok(())
                    } else {
                        Err(NewsError::InvalidEndpoint)
                    }
                }
            },
            _ => Err(NewsError::InvalidEndpoint),
        }
    }
    fn is_valid(&self) -> Result<(), NewsError> {
        self.is_eid_valid(&self.0.primary.source, EIDType::Src)?;
        self.is_eid_valid(&self.0.primary.destination, EIDType::Dst)?;

        // Validate general payload
        let payload = self.0.payload().ok_or(NewsError::PayloadMissing)?;
        let news: News = serde_cbor::from_slice(payload)?;

        // Validate payload message and compression
        if news.comp {
            String::from_utf8(smaz_decompress(&news.msg)?)?;
        } else {
            String::from_utf8(news.msg)?;
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
    pub fn news(&self) -> News {
        let payload = self.0.payload().expect("missing payload in bundle");

        serde_cbor::from_slice(payload).expect("error decoding news payload")
    }
    pub fn compression(&self) -> bool {
        self.news().compression()
    }
    pub fn encryption(&self) -> bool {
        self.news().encryption()
    }
    pub fn signature(&self) -> Option<Vec<u8>> {
        self.news().signature()
    }
    pub fn msg(&self) -> String {
        self.news().msg()
    }
    pub fn topic(&self) -> String {
        self.news().topic()
    }
    pub fn tid(&self) -> Uuid {
        self.news().thread_id()
    }
    pub fn references(&self) -> Option<String> {
        self.news().references()
    }
    pub fn tags(&self) -> Vec<String> {
        self.news().tags().to_vec()
    }
    pub fn bundle(&self) -> &Bundle {
        &self.0
    }

    pub fn to_cbor(&mut self) -> Vec<u8> {
        self.0.to_cbor()
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct News {
    comp: bool,
    enc: bool,
    #[serde(with = "serde_bytes")]
    topic: Vec<u8>,
    tid: Uuid,
    references: Option<String>,
    tags: Vec<String>,
    #[serde(with = "serde_bytes")]
    msg: Vec<u8>,
    sig: Option<Vec<u8>>,
}

impl News {
    pub fn compression(&self) -> bool {
        self.comp
    }
    pub fn encryption(&self) -> bool {
        self.enc
    }
    pub fn references(&self) -> Option<String> {
        self.references.clone()
    }
    pub fn signature(&self) -> Option<Vec<u8>> {
        self.sig.clone()
    }
    pub fn msg(&self) -> String {
        if self.compression() {
            String::from_utf8_lossy(&smaz_decompress(&self.msg).expect("decompressing msg failed"))
                .to_string()
        } else {
            String::from_utf8_lossy(&self.msg).to_string()
        }
    }
    pub fn topic(&self) -> String {
        if self.compression() {
            String::from_utf8_lossy(
                &smaz_decompress(&self.topic).expect("decompressing topic failed"),
            )
            .to_string()
        } else {
            String::from_utf8_lossy(&self.topic).to_string()
        }
    }
    pub fn thread_id(&self) -> Uuid {
        self.tid
    }
    pub fn tags(&self) -> &[String] {
        self.tags.as_slice()
    }
}

pub struct NewsBuilder {
    comp: bool,
    enc: bool,
    topic: Option<String>,
    thread_id: Option<Uuid>,
    references: Option<String>,
    tags: Vec<String>,
    msg: Option<String>,
    sig: Option<Vec<u8>>,
}

impl NewsBuilder {
    pub fn new() -> Self {
        NewsBuilder {
            comp: true,
            enc: false,
            topic: None,
            thread_id: None,
            references: None,
            tags: vec![],
            msg: None,
            sig: None,
        }
    }
    pub fn reply_to(mut self, news: &NewsBundle) -> Self {
        self.references = Some(news.id());
        self.thread_id = Some(news.tid());
        self.tags = news.tags();
        self.topic = Some(news.topic());
        self
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
    pub fn topic(mut self, topic: &str) -> Self {
        self.topic = Some(topic.into());
        self
    }
    pub fn thread_id(mut self, tid: Uuid) -> Self {
        self.thread_id = Some(tid);
        self
    }
    pub fn references(mut self, bid: &str) -> Self {
        self.references = Some(bid.into());
        self
    }
    pub fn tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.into());
        self
    }
    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }
    pub fn signature(mut self, sig: Vec<u8>) -> Self {
        self.sig = Some(sig);
        self
    }
    pub fn build(self) -> Result<News, NewsError> {
        if let Some(msg) = self.msg {
            Ok(News {
                comp: self.comp,
                enc: self.enc,
                topic: if self.comp {
                    smaz_compress(self.topic.ok_or(NewsError::NoTopic)?.as_bytes())
                } else {
                    self.topic.ok_or(NewsError::NoTopic)?.as_bytes().to_vec()
                },
                tid: if let Some(tid) = self.thread_id {
                    tid
                } else {
                    Uuid::new_v4()
                },
                references: self.references,
                tags: self.tags,
                msg: if self.comp {
                    smaz_compress(msg.as_bytes())
                } else {
                    msg.as_bytes().to_vec()
                },
                sig: self.sig,
            })
        } else {
            Err(NewsError::NoMessage)
        }
    }
}

impl Default for NewsBuilder {
    fn default() -> Self {
        Self::new()
    }
}
/// Create a new news bundle for DTN addressing scheme
#[allow(clippy::too_many_arguments)]
pub fn new_news(
    src_node_name: &str,
    dst_newsgroup: &str,
    topic: &str,
    thread_id: Option<Uuid>,
    references: Option<String>,
    msg: &str,
    tags: Vec<String>,
    compression: bool,
) -> Result<NewsBundle, NewsError> {
    let src_eid = EndpointID::with_dtn(&format!("//{}/sms", src_node_name))?;
    let dst_eid = EndpointID::with_dtn(&format!("//{}/~news", dst_newsgroup))?;

    let pblock = primary::PrimaryBlockBuilder::default()
        .destination(dst_eid)
        .source(src_eid)
        .report_to(EndpointID::none())
        .creation_timestamp(CreationTimestamp::now())
        .lifetime(Duration::from_secs(60 * 60))
        .build()
        .unwrap();

    let payload = NewsBuilder::new()
        .compression(compression)
        .message(msg)
        .topic(topic)
        .thread_id(thread_id.unwrap_or_else(Uuid::new_v4))
        .tags(tags);
    let payload = if let Some(referece) = references {
        payload.references(&referece).build()?
    } else {
        payload.build()?
    };
    let cblocks = vec![canonical::new_payload_block(
        BlockControlFlags::empty(),
        serde_cbor::to_vec(&payload)
            .expect("Fatal failure, could not convert news payload to CBOR"),
    )];

    Ok(NewsBundle::try_from(bundle::Bundle::new(pblock, cblocks))
        .expect("error creating news bundle"))
}

/// Create a new news bundle for DTN addressing scheme
pub fn reply_news(
    parent_post: &NewsBundle,
    src_node_name: &str,
    msg: &str,
    compression: bool,
) -> Result<NewsBundle, NewsError> {
    let src_eid = EndpointID::with_dtn(&format!("//{}/sms", src_node_name))?;

    let pblock = primary::PrimaryBlockBuilder::default()
        .destination(parent_post.bundle().primary.destination.clone())
        .source(src_eid)
        .report_to(EndpointID::none())
        .creation_timestamp(CreationTimestamp::now())
        .lifetime(Duration::from_secs(60 * 60))
        .build()
        .unwrap();

    let payload = NewsBuilder::new()
        .compression(compression)
        .message(msg)
        .reply_to(parent_post)
        .build()?;

    let cblocks = vec![canonical::new_payload_block(
        BlockControlFlags::empty(),
        serde_cbor::to_vec(&payload)
            .expect("Fatal failure, could not convert news payload to CBOR"),
    )];

    Ok(NewsBundle::try_from(bundle::Bundle::new(pblock, cblocks))
        .expect("error creating news bundle"))
}

#[cfg(test)]
mod tests {
    use crate::news::{new_news, NewsBundle};
    use std::convert::TryFrom;

    use super::reply_news;
    #[test]
    fn test_news_new_uncompressed() {
        let mut news = new_news(
            "node1",
            "de.hessen.darmstadt",
            "Lorem ipsum dolor sit amet",
            None,
            None,
            "The quick brown fox jumps over the lazy dog",
            Vec::new(),
            false,
        )
        .unwrap();
        let bin_bundle = news.to_cbor();
        dbg!(bin_bundle.len());
        dbg!(bp7::hexify(&bin_bundle));
    }

    #[test]
    fn test_news_new_compressed() {
        let mut news = new_news(
            "node1",
            "de.hessen.darmstadt",
            "Lorem ipsum dolor sit amet",
            None,
            None,
            "The quick brown fox jumps over the lazy dog",
            Vec::new(),
            true,
        )
        .unwrap();
        let bin_bundle = news.to_cbor();
        dbg!(bin_bundle.len());
        dbg!(bp7::hexify(&bin_bundle));

        assert_eq!(
            dbg!(news.msg()),
            "The quick brown fox jumps over the lazy dog"
        );
        assert_eq!(dbg!(news.src().unwrap()), "node1"); // leading zeros are stripped
        assert_eq!(dbg!(news.dst().unwrap()), "de.hessen.darmstadt");
        dbg!(news.creation_timestamp());
    }

    /*#[test]
    fn test_bundle_compressed() {
        let mut news = new_news(
            01239468786,
            01239468999,
            "The quick brown fox jumps over the lazy dog",
            true,
        )
        .unwrap();
        let bin_bundle = news.to_cbor();
        let compressed_bundle = loragent::compression::snap_compress(&bin_bundle);

        dbg!(bin_bundle.len());
        dbg!(compressed_bundle.len());

        dbg!(bp7::hexify(&compressed_bundle));
    }*/

    #[test]
    fn test_invalid_bundles() {
        let news = new_news(
            "node1",
            "de.hessen.darmstadt",
            "Lorem ipsum dolor sit amet",
            None,
            None,
            "The quick brown fox jumps over the lazy dog",
            Vec::new(),
            false,
        )
        .unwrap();
        let mut raw_bundle = news.bundle().clone();
        //let parsed_bundle = newsBundle::try_from(raw_bundle);
        assert!(NewsBundle::try_from(raw_bundle.clone()).is_ok());

        raw_bundle.primary.destination = bp7::EndpointID::none();
        assert!(NewsBundle::try_from(raw_bundle.clone()).is_err());

        raw_bundle.primary.source = bp7::EndpointID::none();
        assert!(NewsBundle::try_from(raw_bundle.clone()).is_err());

        /*raw_bundle.primary.destination = bp7::EndpointID::with_dtn("1234567/news").unwrap();
        assert!(newsBundle::try_from(raw_bundle.clone()).is_err());

        raw_bundle.primary.source = bp7::EndpointID::with_dtn("node1/news").unwrap();
        assert!(newsBundle::try_from(raw_bundle.clone()).is_err());*/

        raw_bundle.primary.source = bp7::EndpointID::with_ipn(123, 777).unwrap();
        assert!(NewsBundle::try_from(raw_bundle.clone()).is_err());

        raw_bundle.primary.destination = bp7::EndpointID::with_ipn(123, 777).unwrap();
        assert!(NewsBundle::try_from(raw_bundle).is_err());
    }

    #[test]
    fn test_news_reply() {
        let news1 = new_news(
            "node1",
            "de.hessen.darmstadt",
            "Lorem ipsum dolor sit amet",
            None,
            None,
            "The quick brown fox jumps over the lazy dog",
            Vec::new(),
            false,
        )
        .unwrap();

        let news2 = reply_news(&news1, "node2", "just a reply", true).unwrap();
        assert_eq!(news1.topic(), news2.topic());
        assert_eq!(news1.tid(), news2.tid());
        assert_eq!(news1.tags(), news2.tags());
        assert_eq!(Some(news1.id()), news2.references());
        assert_ne!(news1.msg(), news2.msg());
    }
}
