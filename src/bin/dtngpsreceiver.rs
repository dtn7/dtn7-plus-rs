use anyhow::{Result, anyhow, bail};
use bp7::dtntime::DtnTimeHelpers;
use bp7::*;
use clap::{Arg, ArgAction, Command, crate_authors, crate_version};
use dtn7_plus::client::DtnClient;
use dtn7_plus::location::*;
use std::convert::TryFrom;
use tungstenite::Message;

fn handle_incoming_bundle(
    bndl: &Bundle,
    rest: Option<String>,
    verbose: bool,
) -> anyhow::Result<()> {
    let cblock = bndl
        .extension_block_by_type(LOCATION_BLOCK)
        .ok_or_else(|| anyhow!("no extension block"))?;
    let loc_data = get_location_data(cblock)?;
    if let LocationBlockData::Position(flags, pos) = loc_data {
        if let Location::LatLon(coords) = pos {
            let mut log_out = format!(
                "{},{},\"{:?}\",{:?}",
                bndl.primary.creation_timestamp.dtntime().unix(),
                bndl.id(),
                // bndl.primary.source.node_id().ok_or(anyhow!("no source address"))?,
                coords,
                flags
            );
            log_out.retain(|c| !c.is_whitespace());
            if verbose {
                println!("{}", log_out);
            }
            if let Some(rest) = rest.clone() {
                let _res = attohttpc::get(format!("{}?gps={}", rest, log_out))
                    .send()
                    .expect("error sending position data")
                    .text()?;
            }
        }
        if let Location::XY(coords) = pos {
            let mut log_out = format!(
                "{},{},\"{:?}\",{:?}",
                bndl.primary.creation_timestamp.dtntime().unix(),
                bndl.id(),
                // bndl.primary.source.node_id().ok_or(anyhow!("no source address"))?,
                coords,
                flags
            );
            log_out.retain(|c| !c.is_whitespace());
            if verbose {
                println!("{}", log_out);
            }
            if let Some(rest) = rest {
                let _res = attohttpc::get(format!("{}?xy={}", rest, log_out))
                    .send()
                    .expect("error sending position data")
                    .text()?;
            }
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let matches = Command::new("dtngpsreceiver")
        .version(crate_version!())
        .author(crate_authors!())
        .about("A simple Bundle Protocol 7 GPS Receiver Utility for Delay Tolerant Networking")
        .arg(
            Arg::new("endpoint")
                .short('e')
                .long("endpoint")
                .value_name("ENDPOINT")
                .help("Specify local endpoint, e.g. '/incoming', or a group endpoint 'dtn://helpers/incoming'")
                .required(true)
        )
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .value_name("PORT")
                .help("Local web port (default = $DTN_WEB_PORT or 3000)")
                .value_parser(clap::value_parser!(u16))
                .required(false)
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("verbose output")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("ipv6")
                .short('6')
                .long("ipv6")
                .help("Use IPv6")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("rest")
                .short('r')
                .long("rest")
                .help("REST endpoint to dump incoming location data, e.g., http://127.0.0.1:1880/dtnpos")
        )
        .get_matches();

    let verbose: bool = matches.get_flag("verbose");
    let localhost = if matches.get_flag("ipv6") {
        "[::1]"
    } else {
        "127.0.0.1"
    };

    // prefer CLI, fall back to env, then 3000
    let port: u16 = matches.get_one::<u16>("port").copied().unwrap_or_else(|| {
        std::env::var("DTN_WEB_PORT")
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(3000)
    });

    let client = DtnClient::with_host_and_port(localhost.into(), port);

    let endpoint: String = matches
        .get_one::<String>("endpoint")
        .expect("endpoint is required")
        .to_owned();
    let rest: Option<String> = matches.get_one::<String>("rest").cloned();

    client.register_application_endpoint(&endpoint)?;
    let mut wscon = client.ws()?;

    wscon.write_text("/bundle")?;
    let msg = wscon.read_text()?;
    if msg.starts_with("200 tx mode: bundle") {
        println!("[*] {}", msg);
    } else {
        bail!("[!] Failed to set mode to `bundle`");
    }

    wscon.write_text(&format!("/subscribe {}", endpoint))?;
    let msg = wscon.read_text()?;
    if msg.starts_with("200 subscribed") {
        println!("[*] {}", msg);
    } else {
        bail!("[!] Failed to subscribe to service");
    }

    loop {
        let msg = wscon.read_message()?;
        match msg {
            Message::Text(txt) => {
                eprintln!("[!] Unexpected response: {}", txt);
                break;
            }
            Message::Binary(bin) => {
                let bndl: Bundle =
                    Bundle::try_from(bin.to_vec()).expect("Error decoding bundle from server");
                if bndl.is_administrative_record() {
                    eprintln!("[!] Handling of administrative records not yet implemented!");
                } else if handle_incoming_bundle(&bndl, rest.clone(), verbose).is_err() && verbose {
                    eprintln!("[!] Not a position bundle: {}", bndl.id());
                }
            }
            Message::Ping(_) => {
                if verbose {
                    eprintln!("[<] Ping")
                }
            }
            Message::Pong(_) => {
                if verbose {
                    eprintln!("[<] Ping")
                }
            }
            Message::Close(_) => {
                if verbose {
                    eprintln!("[<] Close")
                }
                break;
            }
            Message::Frame(_) => {
                if verbose {
                    eprintln!("[!] Received raw frame, not supported!")
                }
            }
        }
    }

    Ok(())
}
