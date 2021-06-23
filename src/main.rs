//    HipparchiaRustDBHelper: search and vector helper app and functions for HipparchiaServer
//    Copyright: E Gunderson 21
//    License: GNU GENERAL PUBLIC LICENSE 3
//        (see LICENSE in the top level directory of the distribution)

use std::thread;

use clap::{App, Arg, ArgMatches};
use json::JsonValue;
use postgres::Error;
use uuid::Uuid;

use crate::dbfunctions::*;
use crate::helpers::*;
use crate::thevectors::*;
use crate::thewebsockets::*;

mod dbfunctions;
mod helpers;
mod svfunctions;
mod thestructs;
mod thewebsockets;
mod thevectors;

static MYNAME: &str = "Hipparchia Rust Helper";
static VERSION: &str = "0.1.1";
static TESTDB: &str = "lt0448";
static TESTSTART: &str = "1";
static TESTEND: &str = "26";
static TESTKEY: &str = "rusttest";
static WORKERSDEFAULT: &str = "5";
static HITSDEFAULT: &str = "200";
static PSQ: &str = r#"{"Host": "localhost", "Port": 5432, "User": "hippa_wr", "Pass": "", "DBName": "hipparchiaDB"}"#;
static RP: &str = r#"{"Addr": "localhost:6379", "Password": "", "DB": 0}"#;

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
            .default_value("1"))
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
        // note that websocket() will never return
        websocket(ft, ll, ip, port, rc.to_string());
    }

    let thekey: &str = cli.value_of("k").unwrap();

    if cli.is_present("sv") {
        let m: String = format!("requested the vector_prep() branch of the code");
        lfl(m, ll, 1);
        let b = cli.value_of("svb").unwrap();
        let db = cli.value_of("svdb").unwrap();
        let sta = cli.value_of("svs").unwrap().parse().unwrap();
        let end = cli.value_of("sve").unwrap().parse().unwrap();
        // build more of the cli interface to get rid of TEST... items
        let resultkey: String = vector_prep(&thekey, &b, workers, db, sta, end, ll, &pg, &rc);
        println!("{}", resultkey);
    } else {
        // if neither "ws" or "vs", then you are a "grabber"
        // note that a fn grabber() gets into a lifetime problem w/ thread::spawn()
        let m: String = format!("requested the grabber() branch of the code");
        lfl(m, ll, 1);
        let c: &str = cli.value_of("c").unwrap();

        // the GRABBER is supposed to be pointedly basic
        //
        // [a] it looks to redis for a pile of SQL queries that were pre-rolled
        // [b] it asks postgres to execute these queries
        // [c] it stores the results on redis
        // [d] it also updates the redis progress poll data relative to this search
        //
        let cap: i32 = c.parse().unwrap();

        // recordinitialsizeofworkpile()
        let mut redisconn = redisconnect(rc.to_string());

        let mut thiskey = format!("{}", &thekey);
        let workpile = rs_scard(&thiskey, &mut redisconn);

        thiskey = format!("{}_poolofwork", &thekey);
        rs_set_str(&thiskey, &workpile.to_string(), &mut redisconn).unwrap();

        // dispatch the workers
        // https://averywagar.com/post/multithreading-rust/
        let handles = (0..workers)
            .into_iter()
            .map(|_| {
                let a = cli.clone();
                thread::spawn( move || {
                    let k = a.value_of("k").unwrap();
                    let pg = a.value_of("p").unwrap();
                    let rc = a.value_of("r").unwrap();
                    grabworker(Uuid::new_v4(), &cap.clone(), &k, &pg, &rc).unwrap();
                })
            })
            .collect::<Vec<thread::JoinHandle<_>>>();

        for thread in handles {
            thread.join().unwrap();
        }

        let thiskey = format!("{}_results", &thekey);
        let hits = rs_scard(&thiskey, &mut redisconn);
        let m = format!("{} hits were stored", &hits);
        lfl(m, ll, 1);

        let resultkey = format!("{}_results", &thekey);
        println!("{}", resultkey);
    }
}

fn grabworker(id: Uuid, cap: &i32, thekey: &str, pg: &str, rc: &str) -> Result<(), Error> {
    // this is where all of the work happens
    let mut redisconn = redisconnect(rc.to_string());
    let mut psqlclient = postgresconnect(pg.to_string());

    let mut passes = 0;
    loop {
        passes = passes + 1;

        // [a] pop a query stored as json in redis
        let j = rs_spop(&thekey, &mut redisconn);
        if &j == &"" {
            let m = format!("{} ran out of work on pass #{}", &id, &passes);
            lfl(m, 0, 0);
            break
        }

        // [b] update the polling data
        let workpile = rs_scard(&thekey, &mut redisconn);
        let w = workpile.to_string();

        let thiskey = format!("{}_remaining", &thekey);
        rs_set_str(&thiskey, w.as_str(), &mut redisconn).unwrap();

        // [c] decode the query
        let parsed = json::parse(j.as_str()).unwrap();
        let t = parsed["TempTable"].as_str().unwrap();
        let q = parsed["PsqlQuery"].as_str().unwrap();
        let d = parsed["PsqlData"].as_str().unwrap();

        // [d] build a temp table if needed
        if &t != &"" {
            psqlclient.execute(t, &[]).ok().expect("TempTable creation failed");
        }

        // [e] execute the main query && [f] iterate through the finds
        // https://siciarz.net/24-days-of-rust-postgres/
        // https://docs.rs/postgres/0.19.1/postgres/index.html
        for row in psqlclient.query(q, &[&d])? {
            // [f1] convert the find to JSON
            // note that we can skip using a DBLine struct here
            let flds = db_fields();
            let mut data = JsonValue::new_object();
            let mut index= 0;
            for f in flds {
                if index == 1 {
                    let r: i32 = row.get(index);
                    data[f] = r.into();
                } else {
                    let r: String = row.get(index);
                    data[f] = r.into();
                }
                index = index + 1;
            }

            // [f2] if you have not hit the cap on finds, store the result in 'querykey_results'
            let thiskey = format!("{}_results", &thekey);
            let hits = rs_scard(&thiskey, &mut redisconn);

            if hits >= *cap {
                rs_del(&thekey, &mut redisconn).unwrap();
                break;
            } else {
                let mut thiskey = format!("{}_results", &thekey);
                rs_sadd(&thiskey, &data.dump(), &mut redisconn).unwrap();
                thiskey = format!("{}_hitcount", &thekey);
                rs_set_int(&thiskey, hits + 1, &mut redisconn).unwrap();
            }
        }
    }
    Ok(())
}
