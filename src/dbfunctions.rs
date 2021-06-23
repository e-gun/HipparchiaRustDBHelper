//    HipparchiaRustDBHelper: search and vector helper app and functions for HipparchiaServer
//    Copyright: E Gunderson 21
//    License: GNU GENERAL PUBLIC LICENSE 3
//        (see LICENSE in the top level directory of the distribution)

use postgres::{Client, NoTls};
use redis::Commands;
use uuid::Uuid;

use std::collections::HashMap;
use crate::thestructs::*;

pub fn postgresconnect(j: String) -> postgres::Client {
    // https://docs.rs/postgres/0.19.1/postgres/
    // https://rust-lang-nursery.github.io/rust-cookbook/database/postgres.html
    let parsed = json::parse(&j).unwrap();
    let host = parsed["Host"].as_str().unwrap();
    let user = parsed["User"].as_str().unwrap();
    let db = parsed["DBName"].as_str().unwrap();
    let port = parsed["Port"].as_i32().unwrap();
    let pw = parsed["Pass"].as_str().unwrap();
    let validate = format!("host={} user={} dbname={} port={} password={}", &host, &user, &db, &port, &pw);
    let client = Client::connect(&validate, NoTls).expect("failed to connect to postgreSQL");
    client
}

pub fn redisconnect(j: String) -> redis::Connection {
    // https://medium.com/swlh/tutorial-getting-started-with-rust-and-redis-69041dd38279
    let parsed = json::parse(&j).unwrap();
    let redis_host_name = parsed["Addr"].as_str().unwrap();
    let redis_password = parsed["Password"].as_str().unwrap();
    // let redis_db = parsed["DB"].as_str().unwrap();
    let uri_scheme = "redis";
    let redis_conn_url = format!("{}://:{}@{}", uri_scheme, redis_password, redis_host_name);
    redis::Client::open(redis_conn_url)
        .expect("Invalid connection URL")
        .get_connection()
        .expect("failed to connect to Redis")
}

pub fn db_sv_get_morphobjects(words: &mut Vec<&str>, lang: &str, pg: &mut postgres::Client) -> Vec<DbMorphology> {
    // the worker for sv_getrequiredmorphobjects()
    // look for the upper case matches too: Ϲωκράτηϲ and not just ϲωκρατέω (!)
    // let start = Instant::now();

    let mut wordswithcaps: Vec<String> = words.iter().map(|w| str_cap(w)).collect();
    let mut w = words.iter().map(|w| w.to_string()).collect();
    wordswithcaps.append(&mut w);

    let mut rndid = Uuid::new_v4().to_string();
    rndid.retain(|c| c != '-');

    let ttarr = wordswithcaps.join("', '");
    let ttarr = format!("'{}'", ttarr);
    let t = format!("CREATE TEMPORARY TABLE ttw_{} AS SELECT words AS w FROM unnest(ARRAY[{}]) words", &rndid, ttarr);

    pg.execute(t.as_str(), &[]).ok().expect("db_arraytogetrequiredmorphobjects() TempTable creation failed");

    let q = format!("SELECT observed_form, xrefs, prefixrefs, related_headwords FROM {}_morphology WHERE EXISTS (SELECT 1 FROM ttw_{} temptable WHERE temptable.w = {}_morphology.observed_form)", &lang, rndid, &lang);
    let dbmo = pg.query(q.as_str(), &[]).unwrap().into_iter()
        .map(|row| DbMorphology {
            obs: row.get("observed_form"),
            xrf: row.get("xrefs"),
            pxr: row.get("prefixrefs"),
            rpo: row.get("related_headwords"),
            upo: row.get::<&str, String>("related_headwords").split_whitespace().map(|s| s.to_string()).collect(),
        }).collect::<Vec<DbMorphology>>();

    dbmo
}

pub fn db_fields<'a>() ->  Vec<&'a str> {
    // used to prep the json encoding for a dbworkline
    let fld = "WkUID TbIndex Lvl5Value Lvl4Value Lvl3Value Lvl2Value Lvl1Value Lvl0Value MarkedUp Accented Stripped Hypenated Annotations";
    let v: Vec<&str> = fld.split_whitespace().collect();
    v
}

pub fn db_directfetch(t: &str, s: i32, e: i32, pg: &mut postgres::Client) -> Vec<DBLine> {
    // let q = "SELECT * FROM lt0448 WHERE index BETWEEN 1 and 25";
    let q = format!("SELECT * FROM {} WHERE index BETWEEN {} and {}", t, s, e);
    let lines: Vec<DBLine> = pg.query(q.as_str(), &[]).unwrap().into_iter()
        .map(|row| DBLine {
            idx: row.get("index"),
            uid: row.get("wkuniversalid"),
            l5: row.get("level_05_value"),
            l4: row.get("level_04_value"),
            l3: row.get("level_03_value"),
            l2: row.get("level_02_value"),
            l1: row.get("level_01_value"),
            l0: row.get("level_00_value"),
            mu: row.get("marked_up_line"),
            ac: row.get("accented_line"),
            st: row.get("stripped_line"),
            hy: row.get("hyphenated_words"),
            an: row.get("annotations"),
        }).collect::<Vec<DBLine>>();
    lines
}

pub fn db_redisfectch(thekey: &str, pg: &str, rc: &str) -> Vec<DBLine> {
    let mut redisconn = redisconnect(rc.to_string());
    let mut psqlclient = postgresconnect(pg.to_string());

    let mut foundlines: Vec<DBLine> = Vec::new();
    loop {
        // [a] pop a query stored as json in redis
        let j = rs_spop(&thekey, &mut redisconn);
        if &j == &"" { break }
        // [b] update the polling data
        let workpile = rs_scard(&thekey, &mut redisconn);
        let w = workpile.to_string();
        let thiskey = format!("{}_remaining", &thekey);
        rs_set_str(&thiskey, w.as_str(), &mut redisconn).unwrap();

        // [c] decode the query
        let parsed = json::parse(j.as_str()).unwrap();
        let t = parsed["TempTable"].as_str().unwrap();
        let q = parsed["PsqlQuery"].as_str().unwrap();
        let _d = parsed["PsqlData"].as_str().unwrap();  // never any data, right...?

        // [d] build a temp table if needed
        if &t != &"" {
            psqlclient.execute(t, &[]).ok().expect("TempTable creation failed");
        }

        // [e] execute the main query && aggregate the finds
        // https://siciarz.net/24-days-of-rust-postgres/
        // https://docs.rs/postgres/0.19.1/postgres/index.html
        let thelines: Vec<DBLine> = psqlclient.query(q, &[]).unwrap().into_iter()
            .map(|row| DBLine {
                idx: row.get("index"),
                uid: row.get("wkuniversalid"),
                l5: row.get("level_05_value"),
                l4: row.get("level_04_value"),
                l3: row.get("level_03_value"),
                l2: row.get("level_02_value"),
                l1: row.get("level_01_value"),
                l0: row.get("level_00_value"),
                mu: row.get("marked_up_line"),
                ac: row.get("accented_line"),
                st: row.get("stripped_line"),
                hy: row.get("hyphenated_words"),
                an: row.get("annotations"),
            }).collect::<Vec<DBLine>>();
        foundlines.extend(thelines);
    }
    println!("db_redisfectch found {} lines", &foundlines.len());
    foundlines
}

pub fn db_fetchheadwordcounts(hw: Vec<String>, pg: &mut postgres::Client) -> HashMap<String, i32> {

    let mut rndid = Uuid::new_v4().to_string();
    rndid.retain(|c| c != '-');

    let ttarr = format!("'{}'", hw.join("', '"));
    let t = format!("CREATE TEMPORARY TABLE ttw_{} AS SELECT words AS w FROM unnest(ARRAY[{}]) words", &rndid, ttarr);
    pg.execute(t.as_str(), &[]).ok().expect("db_fetchheadwordcounts() TempTable creation failed");

    let q = format!("SELECT entry_name, total_count FROM dictionary_headword_wordcounts WHERE EXISTS (SELECT 1 FROM ttw_{} temptable WHERE temptable.w = dictionary_headword_wordcounts.entry_name)", rndid);

    let whwvec: Vec<WeightedHeadword> = pg.query(q.as_str(), &[]).unwrap().into_iter()
        .map(|row| WeightedHeadword {
            wd: row.get("entry_name"),
            ct: row.get("total_count"),
        })
        .collect::<Vec<WeightedHeadword>>();

    let mut wtwhhm: HashMap<String, i32> = HashMap::new();

    for w in whwvec {
        wtwhhm.insert(w.wd, w.ct);
    }

    // don't kill off unfound terms
    for k in hw {
        if wtwhhm.contains_key(k.as_str()) {
            continue;
        } else {
            wtwhhm.insert(k.clone(), 0);
        }
    };
    wtwhhm
}

pub fn rs_del(k: &str, c: &mut redis::Connection) -> redis::RedisResult<()> {
    // DEL
    let _ : () = c.del(k)?;
    Ok(())
}

pub fn rs_scard(k: &str, c: &mut redis::Connection) -> i32 {
    // SCARD

    // https://stackoverflow.com/questions/51141672/how-do-i-ignore-an-error-returned-from-a-rust-function-and-proceed-regardless
    // https://doc.rust-lang.org/book/ch06-02-match.html

    let r: Option<i32> = c.scard(k).ok();
    let count = match r {
        None => 0,
        Some(n) => n,
    };
    count
}

pub fn rs_spop(k: &str, c: &mut redis::Connection) -> String {
    // SPOP
    let p: String = match redis::cmd("SPOP")
        .arg(k)
        .query(c) {
        Ok(s) => s,
        Err(_e) => "".to_string(),
    };
    p
}

pub fn rs_get(k: &str, c: &mut redis::Connection) -> String {
    // GET
    let p: String = match redis::cmd("GET")
        .arg(k)
        .query(c) {
        Ok(s) => s,
        Err(_e) => "".to_string(),
    };
    p
}

pub fn rs_sadd(k: &str, v: &str, c: &mut redis::Connection) -> redis::RedisResult<()> {
    // SADD
    let _ : () = c.sadd(k, v).unwrap_or(());
    Ok(())
}

pub fn rs_set_str(k: &str, v: &str, c: &mut redis::Connection) -> redis::RedisResult<()> {
    // SET
    let _ : () = c.set(k, v).unwrap_or(());
    Ok(())
}

pub fn rs_set_int(k: &str, v: i32, c: &mut redis::Connection) -> redis::RedisResult<()> {
    // SET
    let _ : () = c.set(k, v).unwrap_or(());
    Ok(())
}

pub fn str_cap(s: &str) -> String {
    // if we are not using ascii strings...
    format!("{}{}", s.chars().next().unwrap().to_uppercase(),
            s.chars().skip(1).collect::<String>())
}

pub fn str_lcs(s: &str) -> String {
    // if we are not using ascii strings...
    format!("{}{}", s.chars().next().unwrap().to_lowercase(),
            s.chars().skip(1).collect::<String>())
}

