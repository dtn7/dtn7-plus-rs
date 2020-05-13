use anyhow::Result;
use bp7::bundle::*;
use bp7::*;
use clap::{crate_authors, crate_version, App, Arg};
use dtn7_plus::client::DtnClient;
use dtn7_plus::location::*;
use std::convert::TryInto;
use std::fs;
use std::time::Duration;

fn read_pos_from_file(filename: &str) -> Result<Location> {
    let contents = fs::read_to_string(filename)?;
    let coords: Vec<f32> = contents
        .trim()
        .split(",")
        .map(|c| c.parse().unwrap())
        .collect();
    Ok(Location::LatLon((coords[0], coords[1])))
}
fn main() -> Result<()> {
    let matches = App::new("dtngpslogger")
        .version(crate_version!())
        .author(crate_authors!())
        .about("A simple Bundle Protocol 7 GPS logger")
        .arg(
            Arg::with_name("sender")
                .short("s")
                .long("sender")
                .value_name("SENDER")
                .help("Sets sender name (e.g. 'dtn://node1')")
                .required(false)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("receiver")
                .short("r")
                .long("receiver")
                .value_name("RECEIVER")
                .help("Receiver EID (e.g. 'dtn://nodegroup/pos')")
                .required(true)
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
            Arg::with_name("lifetime")
                .short("l")
                .long("lifetime")
                .value_name("SECONDS")
                .help("Bundle lifetime in seconds (default = 3600)")
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
            Arg::with_name("dryrun")
                .short("D")
                .long("dry-run")
                .help("Don't actually send packet, just dump the encoded one.")
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
            Arg::with_name("INTERVAL")
                .short("i")
                .long("interval")
                .help("Sending interval (1s, 5m, 2h etc)")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("LATLON")
                .short("g")
                .long("gps")
                .help("Coordinates to announce, e.g., '49.1234,008.4567'")
                .takes_value(true)
                .required(true)
                .conflicts_with("WFW")
                .conflicts_with("ADDRESS")
                .conflicts_with("FILE"),
        )
        .arg(
            Arg::with_name("WFW")
                .short("3")
                .long("3words")
                .help("3geonames / whatfreewords, e.g., 'SEMINOLE-CARDELLINI-TOQ'")
                .takes_value(true)
                .required(true)
                .conflicts_with("LATLON")
                .conflicts_with("ADDRESS")
                .conflicts_with("FILE"),
        )
        .arg(
            Arg::with_name("ADDRESS")
                .short("a")
                .long("address")
                .help("Free-form human-readable address")
                .takes_value(true)
                .required(true)
                .conflicts_with("WFW")
                .conflicts_with("LATLON")
                .conflicts_with("FILE"),
        )
        .arg(
            Arg::with_name("FILE")
                .short("f")
                .long("from-file")
                .help("Read GPS coords from file, expected content: 'lat:float,lon:float'")
                .takes_value(true)
                .required(true)
                .conflicts_with("WFW")
                .conflicts_with("ADDRESS")
                .conflicts_with("LATLON"),
        )
        .get_matches();
    let localhost = if matches.is_present("ipv6") {
        "[::1]"
    } else {
        "127.0.0.1"
    };
    let port = std::env::var("DTN_WEB_PORT").unwrap_or_else(|_| "3000".into());
    let port = matches.value_of("port").unwrap_or(&port); // string is fine no need to parse number
    let client = DtnClient::with_host_and_port(
        localhost.into(),
        port.parse::<u16>().expect("invalid port number"),
    );
    let interval = if let Some(i) = matches.value_of("INTERVAL") {
        humantime::parse_duration(i).expect("Could not parse interval parameter!")
    } else {
        std::time::Duration::from_secs(0)
    };

    let dryrun: bool = matches.is_present("dryrun");
    let verbose: bool = matches.is_present("verbose");
    let sender: EndpointID = matches
        .value_of("sender")
        .unwrap_or(
            &client
                .local_node_id()
                .expect("error getting node id from local dtnd")
                .to_string(),
        )
        .try_into()
        .unwrap();
    let receiver: EndpointID = matches.value_of("receiver").unwrap().try_into().unwrap();
    let lifetime: u64 = matches
        .value_of("lifetime")
        .unwrap_or("3600")
        .parse::<u64>()
        .unwrap();

    let from_file = matches.is_present("FILE");
    let mut filename = "";

    let mut loc = if let Some(latlon) = matches.value_of("LATLON") {
        let coords: Vec<f32> = latlon.split(',').map(|c| c.parse().unwrap()).collect();
        Location::LatLon((coords[0], coords[1]))
    } else if let Some(address) = matches.value_of("ADDRESS") {
        Location::Human(address.into())
    } else if let Some(wfw) = matches.value_of("WFW") {
        Location::WFW(wfw.into())
    } else if let Some(fname) = matches.value_of("FILE") {
        filename = fname;
        read_pos_from_file(filename)?
    //Location::WFW(wfw.into())
    } else {
        panic!("This should never happen! Missing position parameter!");
    };
    //let local_url = format!("http://127.0.0.1:3000/send?bundle={}", hexstr);
    //let res = reqwest::get(&local_url).expect("error connecting to local dtnd").text().unwrap();
    loop {
        let cts = client
            .creation_timestamp()
            .expect("error getting creation timestamp from local dtnd");

        let mut bndl = new_std_payload_bundle(sender.clone(), receiver.clone(), vec![]);
        bndl.primary.bundle_control_flags =
            BUNDLE_MUST_NOT_FRAGMENTED | BUNDLE_STATUS_REQUEST_DELIVERY;
        bndl.primary.creation_timestamp = cts;
        bndl.primary.lifetime = std::time::Duration::from_secs(lifetime);

        bndl.canonicals
            .retain(|c| c.block_type != crate::canonical::PAYLOAD_BLOCK);

        let data = LocationBlockData::Position(NodeTypeFlags::MOBILE, loc.clone());
        let cblock = new_location_block(1, data.clone());

        bndl.add_canonical_block(cblock);

        let binbundle = bndl.to_cbor();
        println!("Bundle-Id: {}", bndl.id());
        if verbose || dryrun {
            let hexstr = bp7::helpers::hexify(&binbundle);
            println!("{}", hexstr);
        }
        if !dryrun {
            let res = attohttpc::post(&format!("http://{}:{}/insert", localhost, port))
                .bytes(binbundle)
                .send()
                .expect("error send bundle to dtnd")
                .text()?;
            println!("Result: {}", res);
            let now = std::time::SystemTime::now();
            println!("Time: {}", humantime::format_rfc3339(now));
        }
        if interval == Duration::from_secs(0) {
            break;
        }
        if from_file {
            loc = read_pos_from_file(filename)?;
        }
        std::thread::sleep(interval);
    }
    Ok(())
}
