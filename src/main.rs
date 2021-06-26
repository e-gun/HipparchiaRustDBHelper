//    HipparchiaRustDBHelper: search and vector helper app and functions for HipparchiaServer
//    Copyright: E Gunderson 21
//    License: GNU GENERAL PUBLIC LICENSE 3
//        (see LICENSE in the top level directory of the distribution)

use clap::{App, Arg, ArgMatches};

use crate::helpers::*;
use crate::thegrabber::*;
use crate::thevectors::*;
use crate::thewebsockets::*;

mod dbfunctions;
mod helpers;
mod svfunctions;
mod thestructs;
mod thewebsockets;
mod thevectors;
mod thegrabber;

static MYNAME: &str = "Hipparchia Rust Helper";
static VERSION: &str = "0.1.4";
static TESTDB: &str = "lt0448";
static TESTSTART: &str = "1";
static TESTEND: &str = "26";
static TESTKEY: &str = "rusttest";
static WORKERSDEFAULT: &str = "5";
static HITSDEFAULT: &str = "200";
static PSQ: &str = r#"{"Host": "localhost", "Port": 5432, "User": "hippa_wr", "Pass": "", "DBName": "hipparchiaDB"}"#;
static RP: &str = r#"{"Addr": "localhost:6379", "Password": "", "DB": 0}"#;

// Hipparchia Rust Helper CLI Debugging Interface (v.0.1.2)
// Hipparchia Rust Helper 0.1.2
//
// USAGE:
//     hipparchia_rust_dbhelper [FLAGS] [OPTIONS]
//
// FLAGS:
//     -h, --help       Prints help information
//         --sv         [vectors] assert that this is a vectorizing run
//     -V, --version    Prints version information
//         --ws         [websockets] assert that you are requesting the websocket server
//
// OPTIONS:
//         --c <c>          [searches] max hit count [default: 200]
//         --k <k>          [searches] redis key to use [default: rusttest]
//         --l <l>          [common] logging level [default: 0]
//         --p <p>          [common] postgres login info (as JSON) [default: {"Host": "localhost", "Port": 5432, "User":
//                          "hippa_wr", "Pass": "", "DBName": "hipparchiaDB"}]
//         --r <r>          [common] redis login info (as JSON) [default: {"Addr": "localhost:6379", "Password": "", "DB":
//                          0}]
//         --svb <svb>      [vectors] the bagging method: choices are alternates, flat, unlemmatized, winnertakesall
//                          [default: winnertakesall]
//         --svbs <svbs>    [vectors] number of sentences per bag [default: 1]
//         --svdb <svdb>    [vectors][for manual debugging] db to grab from [default: lt0448]
//         --sve <sve>      [vectors][for manual debugging] last line to grab [default: 26]
//         --svs <svs>      [vectors][for manual debugging] first line to grab [default: 1]
//         --t <t>          [common] number of workers to dispatch [default: 5]
//         --wsf <wsf>      [websockets] fail threshold before messages stop being sent [default: 4]
//         --wsh <wsh>      [websockets] IP address to open up [default: 127.0.0.1]
//         --wsp <wsp>      [websockets] port on which to open the websocket server [default: 5010]
//         --wss <wss>      [websockets] save the polls instead of deleting them: 0 is no; 1 is yes [default: 0]

fn main() {
    println!("{} CLI Debugging Interface (v.{})", MYNAME, VERSION);
    // cli stuff
    // .arg(Arg::with_name().long().takes_value().help())
    let cli: ArgMatches = App::new(MYNAME)
        .version(VERSION)
        .arg(Arg::with_name("c")
            .long("c")
            .takes_value(true)
            .help("[searches] max hit count")
            .default_value(HITSDEFAULT))
        .arg(Arg::with_name("k")
            .long("k")
            .takes_value(true)
            .help("[searches] redis key to use")
            .default_value(TESTKEY))
        .arg(Arg::with_name("l")
            .long("l")
            .takes_value(true)
            .help("[common] logging level")
            .default_value("0"))
        .arg(Arg::with_name("p")
            .long("p")
            .takes_value(true)
            .help("[common] postgres login info (as JSON)")
            .default_value(PSQ))
        .arg(Arg::with_name("r")
            .long("r")
            .takes_value(true)
            .help("[common] redis login info (as JSON)")
            .default_value(RP))
        .arg(Arg::with_name("t")
            .long("t")
            .takes_value(true)
            .help("[common] number of workers to dispatch")
            .default_value(WORKERSDEFAULT))
        .arg(Arg::with_name("sv")
            .long("sv")
            .takes_value(false)
            .help("[vectors] assert that this is a vectorizing run"))
        .arg(Arg::with_name("svb")
            .long("svb")
            .takes_value(true)
            .help("[vectors] the bagging method: choices are alternates, flat, unlemmatized, winnertakesall")
            .default_value("winnertakesall"))
        .arg(Arg::with_name("svbs")
            .long("svbs")
            .takes_value(true)
            .help("[vectors] number of sentences per bag")
            .default_value("1"))
        .arg(Arg::with_name("svdb")
            .long("svdb")
            .takes_value(false)
            .help("[vectors][for manual debugging] db to grab from")
            .default_value(TESTDB))
        .arg(Arg::with_name("sve")
            .long("sve")
            .takes_value(false)
            .help("[vectors][for manual debugging] last line to grab")
            .default_value(TESTEND))
        .arg(Arg::with_name("svs")
            .long("svs")
            .takes_value(false)
            .help("[vectors][for manual debugging] first line to grab")
            .default_value(TESTSTART))
        .arg(Arg::with_name("ws")
            .long("ws")
            .takes_value(false)
            .help("[websockets] assert that you are requesting the websocket server"))
        .arg(Arg::with_name("wsf")
            .long("wsf")
            .takes_value(true)
            .help("[websockets] fail threshold before messages stop being sent")
            .default_value("4"))
        .arg(Arg::with_name("wsp")
            .long("wsp")
            .takes_value(true)
            .help("[websockets] port on which to open the websocket server")
            .default_value("5010"))
        .arg(Arg::with_name("wss")
            .long("wss")
            .takes_value(true)
            .help("[websockets] save the polls instead of deleting them: 0 is no; 1 is yes")
            .default_value("0"))
        .arg(Arg::with_name("wsh")
            .long("wsh")
            .takes_value(true)
            .help("[websockets] IP address to open up")
            .default_value("127.0.0.1"))
        .get_matches();

    let ft = cli.value_of("wsf").unwrap();
    let ip = cli.value_of("wsh").unwrap();
    let port = cli.value_of("wsp").unwrap();

    let ll = cli.value_of("l").unwrap();
    let ll: i32 = ll.parse().unwrap();

    let t = cli.value_of("t").unwrap();
    let workers: i32 = t.parse().unwrap();

    let rc = cli.value_of("r").unwrap();
    let pg = cli.value_of("p").unwrap();

    if cli.is_present("ws") {
        let m: String = format!("requested the websocket() branch of the code");
        lfl(m, ll, 1);
        let save = cli.value_of("wss").unwrap();
        let s: i32 = save.parse().unwrap();
        // note that websocket() will never return
        websocket(ft, ll, ip, port, s, rc.to_string());
    }

    let thekey: &str = cli.value_of("k").unwrap();

    if cli.is_present("sv") {
        let m: String = format!("requested the vector_prep() branch of the code");
        lfl(m, ll, 1);
        let b = cli.value_of("svb").unwrap();
        let bs = cli.value_of("svbs").unwrap().parse().unwrap();
        let db = cli.value_of("svdb").unwrap();
        let sta = cli.value_of("svs").unwrap().parse().unwrap();
        let end = cli.value_of("sve").unwrap().parse().unwrap();
        let resultkey: String = vector_prep(&thekey, &b, workers, bs, db, sta, end, ll, &pg, &rc);
        println!("{}", resultkey);
    } else {
        // if neither "ws" or "vs", then you are a "grabber"
        // note that a fn grabber() gets into a lifetime problem w/ thread::spawn()
        let m: String = format!("requested the grabber() branch of the code");
        lfl(m, ll, 1);
        let resultkey: String = grabber(cli.clone(), thekey.to_string(), ll, workers, rc.to_string());
        println!("{}", resultkey);
    }
}
