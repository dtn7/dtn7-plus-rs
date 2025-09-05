use anyhow::Result;
use bp7::bundle::*;
use bp7::flags::{BundleControlFlags, BundleValidation};
use bp7::*;
use clap::{crate_authors, crate_version, Arg, ArgAction, ArgGroup, Command};
use dtn7_plus::client::DtnClient;
use dtn7_plus::location::*;
use std::convert::TryInto;
use std::fs;
use std::time::Duration;

fn read_pos_from_file(filename: &str, file_is_xy: bool) -> Result<Location> {
    let contents = fs::read_to_string(filename)?;
    let coords: Vec<f32> = contents
        .trim()
        .split(',')
        .map(|c| c.parse().unwrap())
        .collect();
    if file_is_xy {
        Ok(Location::XY((coords[0], coords[1])))
    } else {
        Ok(Location::LatLon((coords[0], coords[1])))
    }
}
fn main() -> Result<()> {
    let matches = Command::new("dtngpslogger")
        .version(crate_version!())
        .author(crate_authors!())
        .about("A simple Bundle Protocol 7 GPS logger")
        .arg(
            Arg::new("sender")
                .short('s')
                .long("sender")
                .value_name("SENDER")
                .help("Sets sender name (e.g. 'dtn://node1')")
                .required(false)
        )
        .arg(
            Arg::new("receiver")
                .short('r')
                .long("receiver")
                .value_name("RECEIVER")
                .help("Receiver EID (e.g. 'dtn://nodegroup/pos')")
                .required(true)
        )
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .value_name("PORT")
                .help("Local web port (default = $DTN_WEB_PORT or 3000)")
                .value_parser(clap::value_parser!(u16))
        )
        .arg(
            Arg::new("lifetime")
                .short('l')
                .long("lifetime")
                .value_name("SECONDS")
                .help("Bundle lifetime in seconds (default = 3600)")
                .value_parser(clap::value_parser!(u64))
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("verbose output")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("dryrun")
                .short('D')
                .long("dry-run")
                .help("Don't actually send packet, just dump the encoded one.")
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
            Arg::new("flag-mobile")
                .short('m')
                .long("flag-mobile")
                .help("Set mobile flag")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("INTERVAL")
                .short('i')
                .long("interval")
                .help("Sending interval (1s, 5m, 2h etc)")
        )
        .arg(
            Arg::new("LATLON")
                .short('g')
                .long("gps")
                .help("Coordinates to announce, e.g., '49.1234,008.4567'")
        )
        .arg(
            Arg::new("WFW")
                .short('3')
                .long("3words")
                .help("3geonames / whatfreewords, e.g., 'SEMINOLE-CARDELLINI-TOQ'")
        )
        .arg(
            Arg::new("ADDRESS")
                .short('a')
                .long("address")
                .help("Free-form human-readable address")
        )
        .arg(
            Arg::new("FILE")
                .short('f')
                .long("from-file")
                .help("Read coordinates from file\nexpected content: 'float,float' as lat,lon or x,y\nBy default coordinates are parsed as lat,lon")
        )
        .arg(
            Arg::new("FILE_XY")
                .short('x')
                .long("file-is-xy")
                .help("Interpret file as x,y coordinates")
                .action(ArgAction::SetTrue)
                .requires("FILE")
        )
        // Require exactly one of the position inputs:
        .group(
            ArgGroup::new("POSITION")
                .args(["LATLON", "WFW", "ADDRESS", "FILE"])
                .required(true)
                .multiple(false)
        )
        .get_matches();

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

    let interval: Duration = matches
        .get_one::<String>("INTERVAL")
        .map(|s| humantime::parse_duration(s).expect("Could not parse interval parameter!"))
        .unwrap_or_else(|| Duration::from_secs(0));

    let dryrun: bool = matches.get_flag("dryrun");
    let verbose: bool = matches.get_flag("verbose");
    let flag_mobile: bool = matches.get_flag("flag-mobile");
    let file_is_xy: bool = matches.get_flag("FILE_XY");

    let default_sender = client
        .local_node_id()
        .expect("error getting node id from local dtnd")
        .to_string();

    let sender: EndpointID = matches
        .get_one::<String>("sender")
        .map(String::as_str)
        .unwrap_or(&default_sender)
        .try_into()
        .unwrap();

    let receiver: EndpointID = matches
        .get_one::<String>("receiver")
        .expect("receiver is required")
        .as_str()
        .try_into()
        .unwrap();

    let lifetime: u64 = matches.get_one::<u64>("lifetime").copied().unwrap_or(3600);

    let from_file = matches.get_flag("FILE");
    let mut filename: &str = "";

    let mut loc = if let Some(latlon) = matches.get_one::<String>("LATLON") {
        let coords: Vec<f32> = latlon.split(',').map(|c| c.parse().unwrap()).collect();
        Location::LatLon((coords[0], coords[1]))
    } else if let Some(address) = matches.get_one::<String>("ADDRESS") {
        Location::Human(address.into())
    } else if let Some(wfw) = matches.get_one::<String>("WFW") {
        Location::WFW(wfw.into())
    } else if let Some(fname) = matches.get_one::<String>("FILE") {
        filename = fname;
        read_pos_from_file(filename, file_is_xy)?
    //Location::WFW(wfw.into())
    } else {
        unreachable!("ArgGroup POSITION ensures one of the inputs is present");
    };
    //let local_url = format!("http://127.0.0.1:3000/send?bundle={}", hexstr);
    //let res = reqwest::get(&local_url).expect("error connecting to local dtnd").text().unwrap();

    loop {
        let cts = client
            .creation_timestamp()
            .expect("error getting creation timestamp from local dtnd");

        let mut bndl = new_std_payload_bundle(sender.clone(), receiver.clone(), vec![]);
        bndl.primary
            .bundle_control_flags
            .set(BundleControlFlags::BUNDLE_MUST_NOT_FRAGMENTED);
        bndl.primary.creation_timestamp = cts;
        bndl.primary.lifetime = Duration::from_secs(lifetime);

        bndl.canonicals
            .retain(|c| c.block_type != crate::canonical::PAYLOAD_BLOCK);

        let node_flags = if flag_mobile {
            NodeTypeFlags::MOBILE
        } else {
            NodeTypeFlags::empty()
        };
        let data = LocationBlockData::Position(node_flags, loc.clone());
        let cblock = new_location_block(1, data.clone());

        bndl.add_canonical_block(cblock);

        let binbundle = bndl.to_cbor();
        println!("Bundle-Id: {}", bndl.id());
        if verbose || dryrun {
            let hexstr = bp7::helpers::hexify(&binbundle);
            println!("{}", hexstr);
        }
        if !dryrun {
            let res = attohttpc::post(format!("http://{}:{}/insert", localhost, port))
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
            loc = read_pos_from_file(filename, file_is_xy)?;
        }
        std::thread::sleep(interval);
    }

    Ok(())
}
