//! Simple ways to interact with dtnd
//!
//! # Example
//!
//! ```
//! use dtn7_plus::client::DtnClient;
//!
//! let client = DtnClient::new();
//!
//! let local_node = client.local_node_id()?;
//! client.register_application_endpoint("incoming")?;
//!
//! # Ok::<(), dtn7_plus::client::ClientError>(())
//! ```
use bp7::{CreationTimestamp, EndpointID};
use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use thiserror::Error;
use tungstenite::{client, WebSocket};

pub use tungstenite::protocol::Message;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("message not utf8: {0}")]
    NonUtf8(#[from] std::string::FromUtf8Error),
    #[error("serde cbor error: {0}")]
    Cbor(#[from] serde_cbor::Error),
    #[error("serde json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("http connection error: {0}")]
    Http(#[from] attohttpc::Error),
    #[error("failed to create endpoint: {0}")]
    EndpointIdInvalid(#[from] bp7::eid::EndpointIdError),
}

/// Client for connecting to a local dtnd instance
///
/// Works with IPv6 and IPv4.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DtnClient {
    localhost: String,
    port: u16,
}

impl DtnClient {
    /// Constructs a new client for `127.0.0.1` on port `3000`.
    pub fn new() -> Self {
        DtnClient {
            localhost: "127.0.0.1".into(),
            port: 3000,
        }
    }
    /// New client with custom host and port
    pub fn with_host_and_port(localhost: String, port: u16) -> Self {
        DtnClient { localhost, port }
    }
    /// Return the local node ID via rest interface
    pub fn local_node_id(&self) -> Result<EndpointID, ClientError> {
        Ok(attohttpc::get(&format!(
            "http://{}:{}/status/nodeid",
            self.localhost, self.port
        ))
        .send()?
        .text()?
        .try_into()?)
    }
    /// Get a new node-wide unique creation timestamp via rest interface
    pub fn creation_timestamp(&self) -> Result<CreationTimestamp, ClientError> {
        let response = attohttpc::get(&format!("http://{}:{}/cts", self.localhost, self.port))
            .send()?
            .text()?;
        Ok(serde_json::from_str(&response)?)
    }
    /// Register a new application endpoint at local node
    pub fn register_application_endpoint(&self, path: &str) -> Result<(), ClientError> {
        let _response = attohttpc::get(&format!(
            "http://{}:{}/register?{}",
            self.localhost, self.port, path
        ))
        .send()?
        .text()?;
        Ok(())
    }
    /// Unregister an application endpoint at local node
    pub fn unregister_application_endpoint(&self, path: &str) -> Result<(), ClientError> {
        let _response = attohttpc::get(&format!(
            "http://{}:{}/unregister?{}",
            self.localhost, self.port, path
        ))
        .send()?
        .text()?;
        Ok(())
    }

    /// Constructs a new websocket connection to the configured dtn7 client
    pub fn ws(&self) -> anyhow::Result<DtnWsConnection<std::net::TcpStream>> {
        let stream = std::net::TcpStream::connect(&format!("{}:{}", self.localhost, self.port))?;
        let ws = self.ws_custom(stream)?;
        Ok(ws)
    }

    /// Constructs a new websocket connection to the configured dtn7 client using a custom Stream
    pub fn ws_custom<Stream>(&self, stream: Stream) -> anyhow::Result<DtnWsConnection<Stream>>
    where
        Stream: std::io::Read + std::io::Write,
    {
        let ws_url = url::Url::parse(&format!("ws://{}:{}/ws", self.localhost, self.port))
            .expect("Error constructing websocket url!");
        let (socket, _) = client(&ws_url, stream).expect("Error constructing websocket!");
        Ok(DtnWsConnection { socket })
    }
}
pub struct DtnWsConnection<Stream>
where
    Stream: std::io::Read + std::io::Write,
{
    socket: WebSocket<Stream>,
}

impl<Stream> DtnWsConnection<Stream>
where
    Stream: std::io::Read + std::io::Write,
{
    /// Send a text message via websocket
    ///
    /// accepted commands:
    /// `/data`
    /// `/bundle`
    /// `/subscribe <service>`
    pub fn write_text(&mut self, txt: &str) -> anyhow::Result<()> {
        self.socket.write_message(Message::text(txt))?;
        Ok(())
    }
    /// Send a binary message via websocket
    ///
    /// Server expects either
    /// - a valid bundle (in bundle mode)
    /// - a WsSendData struct as a cbor buffer (in data mode)
    pub fn write_binary(&mut self, bin: &[u8]) -> anyhow::Result<()> {
        self.socket.write_message(Message::binary(bin))?;
        Ok(())
    }

    /// Read the next message
    ///
    /// Could be text, binary, ping, etc etc
    pub fn read_message(&mut self) -> anyhow::Result<Message> {
        Ok(self.socket.read_message()?)
    }

    /// Expect a text message next, returning an error on any other message type
    pub fn read_text(&mut self) -> anyhow::Result<String> {
        let msg = self.socket.read_message()?;
        if let Message::Text(txt) = msg {
            Ok(txt)
        } else {
            anyhow::bail!("Unexpected message type");
        }
    }
    /// Expect a binary message next, returning an error on any other message type
    pub fn read_binary(&mut self) -> anyhow::Result<Vec<u8>> {
        let msg = self.socket.read_message()?;
        if let Message::Binary(bin) = msg {
            Ok(bin)
        } else {
            anyhow::bail!("Unexpected message type");
        }
    }
}
/// Let server construct a new bundle from the provided data
///
/// To be used via WebSocket connection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WsSendData {
    /// source with a dtn URI scheme, e.g. dtn://node1 or ipn://23.0
    pub src: String,
    /// destination with a dtn URI scheme, e.g. dtn://node1/sms or ipn://23.42/
    pub dst: String,
    /// turn on delivery notifications
    pub delivery_notification: bool,
    /// lifetime for bundle in milliseconds
    pub lifetime: u64,
    /// payload data
    #[serde(with = "crate::serde::base64_or_bytes")]
    pub data: Vec<u8>,
}

/// Received bundle payload with meta data
///
/// To be used via WebSocket connection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WsRecvData {
    pub bid: String,
    pub src: String,
    pub dst: String,
    #[serde(with = "crate::serde::base64_or_bytes")]
    pub data: Vec<u8>,
}
