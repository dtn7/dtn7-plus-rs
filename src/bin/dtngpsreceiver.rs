use bp7::*;
use clap::{crate_authors, crate_version, App, Arg};
use dtn7_plus::client::DtnClient;
use dtn7_plus::location::*;
use std::convert::TryFrom;
use ws::{Builder, CloseCode, Handler, Handshake, Message, Result, Sender};
use anyhow::anyhow;
use bp7::dtntime::DtnTimeHelpers;

struct Connection {
    endpoint: String,
    out: Sender,
    subscribed: bool,
    verbose: bool,
    rest: Option<String>,
}

impl Connection {
    fn handle_incoming_bundle(&self, bndl: &Bundle) -> anyhow::Result<()> {
        let cblock = bndl.extension_block_by_type(LOCATION_BLOCK).ok_or(anyhow!("no extension block"))?;
        let loc_data = get_location_data(cblock)?;
        match loc_data {
            LocationBlockData::Position(flags, pos) => {
                if let Location::LatLon(coords) = pos {
                    let mut log_out = format!("{},{},\"{:?}\",{:?}", bndl.primary.creation_timestamp.dtntime().unix(), bndl.id(),/* bndl.primary.source.node_id().ok_or(anyhow!("no source address"))?,*/ coords, flags);
                    log_out.retain(|c| !c.is_whitespace());
                    if self.verbose {
                        println!("{}", log_out);                        
                    }
                    if let Some(rest) = &self.rest {
                        let _res = attohttpc::get(&format!("{}?data={}", rest, log_out))
                            .send()
                            .expect("error sending position data")
                            .text()?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

impl Handler for Connection {
    fn on_open(&mut self, _: Handshake) -> Result<()> {
        self.out.send(format!("/bundle"))?;
        Ok(())
    }

    fn on_message(&mut self, msg: Message) -> Result<()> {
        match msg {
            Message::Text(txt) => {
                if txt.starts_with("200") {
                    if txt == "200 subscribed" {
                        if self.verbose {
                            eprintln!("successfully subscribed to {}!", self.endpoint);
                        }
                        self.subscribed = true;
                    } else if txt == "200 tx mode: bundle" {
                        if self.verbose {
                            eprintln!("successfully set mode: bundle!");
                        }
                        self.out.send(format!("/subscribe {}", self.endpoint))?;
                    }
                } else {
                    eprintln!("Unexpected response: {}", txt);
                    self.out.close(CloseCode::Error)?;
                }
            }
            Message::Binary(bin) => {
                let bndl: Bundle =
                    Bundle::try_from(bin).expect("Error decoding bundle from server");
                if bndl.is_administrative_record() {
                    eprintln!("Handling of administrative records not yet implemented!");
                } else {
                    /*if self.verbose {
                        eprintln!("Bundle-Id: {}", bndl.id());
                    }*/
                    if self.handle_incoming_bundle(&bndl).is_err() && self.verbose {
                        eprintln!("Not a position bundle: {}", bndl.id());
                    }
                }
            }
        }
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let matches = App::new("dtngpsreceiver")
        .version(crate_version!())
        .author(crate_authors!())
        .about("A simple Bundle Protocol 7 GPS Receiver Utility for Delay Tolerant Networking")
        .arg(
            Arg::with_name("endpoint")
                .short("e")
                .long("endpoint")
                .value_name("ENDPOINT")
                .help("Specify local endpoint, e.g. '/incoming', or a group endpoint 'dtn://helpers/incoming'")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .value_name("PORT")
                .help("Local web port (default = 3000)")
                .required(false)
                .takes_value(true),
        )        
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .help("verbose output")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("ipv6")
                .short("6")
                .long("ipv6")
                .help("Use IPv6")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("rest")
                .short("r")
                .long("rest")
                .help("Rest endpoint to dump incoming location data, e.g., http://127.0.0.1:1880/dtnpos")
                .takes_value(true),
        )
        .get_matches();

    let verbose: bool = matches.is_present("verbose");
    let port = std::env::var("DTN_WEB_PORT").unwrap_or_else(|_| "3000".into());
    let port = matches.value_of("port").unwrap_or(&port); // string is fine no need to parse number
    let localhost = if matches.is_present("ipv6") {
        "[::1]"
    } else {
        "127.0.0.1"
    };
    let local_url = format!("ws://{}:{}/ws", localhost, port);

    let client = DtnClient::with_host_and_port(
        localhost.into(),
        port.parse::<u16>().expect("invalid port number"),
    );

    let endpoint: String = matches.value_of("endpoint").unwrap().into();
    let rest: Option<String> = matches.value_of("rest").map(|r| r.into());

    client.register_application_endpoint(&endpoint)?;
    let mut ws = Builder::new()
        .build(|out| Connection {
            endpoint: endpoint.clone(),
            out,
            subscribed: false,
            verbose,
            rest: rest.clone(),
        })
        .unwrap();
    ws.connect(url::Url::parse(&local_url)?)?;
    ws.run()?;
    Ok(())
}
