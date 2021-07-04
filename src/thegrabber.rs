//    HipparchiaRustDBHelper: search and vector helper app and functions for HipparchiaServer
//    Copyright: E Gunderson 2021
//    License: GNU GENERAL PUBLIC LICENSE 3
//

use std::thread;

use clap::ArgMatches;
use json::JsonValue;
use postgres::Error;
use uuid::Uuid;

use crate::dbfunctions::*;
use crate::helpers::*;

pub fn grabber(cliclone: ArgMatches<'static>, thekey: String, ll: i32, workers: i32, rc: String) -> String {
    // the GRABBER is supposed to be pointedly basic
    //
    // [a] it looks to redis for a pile of SQL queries that were pre-rolled
    // [b] it asks postgres to execute these queries
    // [c] it stores the results on redis
    // [d] it also updates the redis progress poll data relative to this search
    //
    let c: &str = cliclone.value_of("c").unwrap();
    let cap: i32 = c.parse().unwrap();

    // recordinitialsizeofworkpile()
    let mut redisconn = redisconnect(rc.clone());

    let mut thiskey = format!("{}", &thekey);
    let workpile = rs_scard(&thiskey, &mut redisconn);

    thiskey = format!("{}_poolofwork", &thekey);
    rs_set_str(&thiskey, &workpile.to_string(), &mut redisconn).unwrap();

    // dispatch the workers
    // https://averywagar.com/post/multithreading-rust/
    let handles = (0..workers)
        .into_iter()
        .map(|_| {
            let a = cliclone.clone();
            thread::spawn( move || {
                let k = a.value_of("k").unwrap();
                let pg = a.value_of("p").unwrap();
                let rc = a.value_of("r").unwrap();
                grabworker(Uuid::new_v4(), &cap.clone(), &k, ll, &pg, &rc).unwrap();
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
    resultkey
}

fn grabworker(id: Uuid, cap: &i32, thekey: &str, ll: i32, pg: &str, rc: &str) -> Result<(), Error> {
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
            lfl(m, ll, 3);
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
