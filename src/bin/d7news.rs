use std::{
    convert::{TryFrom, TryInto},
    io::{Read, Write},
};

use anyhow::Result;
use clap::{crate_authors, crate_version, ArgAction, Parser};
use dtn7_plus::news::{new_news, reply_news, NewsBundle};

#[derive(Parser)]
#[clap(version = crate_version!(), author = crate_authors!())]
struct Opts {
    /// A level of verbosity, and can be used multiple times
    #[clap(short, long, action = ArgAction::Count)]
    verbose: u8,
    #[clap(subcommand)]
    subcmds: SubCommand,
}

#[derive(Parser)]
enum SubCommand {
    //#[clap(version = "1.3", author = "Someone E. <someone_else@other.com>")]
    Post(PostCmd),
    Reply(ReplyCmd),
    Read(ReadCmd),
}

/// Create a new post
#[derive(Parser)]
struct PostCmd {
    /// Sender DTN node name
    #[clap(short, long)]
    src_node_name: String,

    /// Destination newsgroup
    #[clap(short, long)]
    dst_group: String,

    /// Topic
    #[clap(short, long)]
    topic: String,

    /// Message body or '-' to read from stdin
    #[clap(short, long)]
    message: String,

    /// Output bundle as hex
    #[clap(short = 'H', long)]
    hex: bool,
}

fn cmd_post(opts: PostCmd, _log_level: u8) -> Result<()> {
    let msg = if opts.message == "-" {
        let mut raw_bytes: Vec<u8> = Vec::new();
        std::io::stdin()
            .read_to_end(&mut raw_bytes)
            .expect("Error reading from stdin.");
        String::from_utf8(raw_bytes)?
    } else {
        opts.message
    };
    let post = new_news(
        &opts.src_node_name,
        &opts.dst_group,
        &opts.topic,
        None,
        None,
        &msg,
        Vec::new(),
        true,
    )?
    .to_cbor();

    if opts.hex {
        println!("{}", bp7::helpers::hexify(&post));
    } else {
        std::io::stdout().write_all(&post).unwrap();
    }

    Ok(())
}

/// Create a new post
#[derive(Parser)]
struct ReplyCmd {
    /// Sender DTN node name
    #[clap(short, long)]
    src_node_name: String,

    /// Message body or '-' to read from stdin
    #[clap(short, long)]
    message: String,

    /// Original bundle as hex string
    #[clap(short, long)]
    input_newsbundle: String,

    /// Original bundle as hex
    #[clap(short = 'H', long)]
    hex: bool,
}

fn cmd_reply(opts: ReplyCmd, _log_level: u8) -> Result<()> {
    let msg = if opts.message == "-" {
        let mut raw_bytes: Vec<u8> = Vec::new();
        std::io::stdin()
            .read_to_end(&mut raw_bytes)
            .expect("Error reading from stdin.");
        String::from_utf8(raw_bytes)?
    } else {
        opts.message
    };
    let raw_bytes = bp7::helpers::unhexify(&opts.input_newsbundle)?;
    let news_bundle: NewsBundle = raw_bytes.try_into()?;
    let post = reply_news(&news_bundle, &opts.src_node_name, &msg, true)?.to_cbor();

    if opts.hex {
        println!("{}", bp7::helpers::hexify(&post));
    } else {
        std::io::stdout().write_all(&post).unwrap();
    }

    Ok(())
}

/// Decode news bundle in various forms
#[derive(Parser)]
struct ReadCmd {
    /// Read bundle provided as hex string
    #[clap(short = 'H', long)]
    hex: Option<String>,
    /// Read bundles from a file or '-' for stdin
    #[clap(short, long)]
    path: Option<String>,
}
fn cmd_read(opts: ReadCmd, _log_level: u8) -> Result<()> {
    let bytes = if let Some(hex_str) = opts.hex {
        bp7::helpers::unhexify(&hex_str)?
    } else {
        let mut raw_bytes: Vec<u8> = Vec::new();
        std::io::stdin()
            .read_to_end(&mut raw_bytes)
            .expect("Error reading from stdin.");
        raw_bytes
    };
    let news = NewsBundle::try_from(bytes)?;
    println!("{}", news);
    Ok(())
}

fn main() -> Result<()> {
    let opts: Opts = Opts::parse();

    let log_level = opts.verbose;

    match opts.subcmds {
        SubCommand::Post(post) => {
            cmd_post(post, log_level)?;
        }
        SubCommand::Read(read) => {
            cmd_read(read, log_level)?;
        }
        SubCommand::Reply(reply) => {
            cmd_reply(reply, log_level)?;
        }
    }

    Ok(())
}
