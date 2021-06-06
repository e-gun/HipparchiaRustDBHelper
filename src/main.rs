//    HipparchiaRustDBHelper: search and vector helper app and functions for HipparchiaServer
//    Copyright: E Gunderson 2016-21
//    License: GNU GENERAL PUBLIC LICENSE 3
//        (see LICENSE in the top level directory of the distribution)

use std::{net::TcpListener, thread::spawn};
use std::{thread, time};
use std::collections::HashMap;
use std::convert::TryInto;
use std::env::vars_os;
use std::net::TcpStream;
use std::time::{Duration, Instant, SystemTime};

use clap::{App, Arg, ArgMatches};
use humantime::format_duration;
use json::JsonValue;
use lazy_static::lazy_static;
use postgres::{Client, Error, NoTls};
use redis::Commands;
use regex::Regex;
use tungstenite::{accept_hdr, handshake::server::{Request, Response}, Message, WebSocket};
use uuid::Uuid;

static MYNAME: &str = "Hipparchia Rust Helper";
static SHORTNAME: &str = "HRH";
static VERSION: &str = "0.0.4";
static POLLINGINTERVAL: time::Duration = time::Duration::from_millis(400);
static TESTDB: &str = "lt0448";
static TESTSTART: &str = "1";
static TESTEND: &str = "26";
static TESTKEY: &str = "rusttest";
static LINELENGTH: u32 = 72;
static WORKERSDEFAULT: &str = "5";
static HITSDEFAULT: &str = "200";
static PSQ: &str = r#"{"Host": "localhost", "Port": 5432, "User": "hippa_wr", "Pass": "", "DBName": "hipparchiaDB"}"#;
static RP: &str = r#"{"Addr": "localhost:6379", "Password": "", "DB": 0}"#;

struct DBLine {
    idx: i32,
    uid: String,
    l5: String,
    l4: String,
    l3: String,
    l2: String,
    l1: String,
    l0: String,
    mu: String,
    ac: String,
    st: String,
    hy: String,
    an: String,
}

#[derive(Clone)]
struct DbMorphology {
    obs: String,
    xrf: String,
    pxr: String,
    rpo: String,
    upo: HashMap<String, bool>
}

struct MorphPossibility {
    obs: String,
    num: String,
    ent: String,
    xrf: String,
    ana: String,
}

struct SentenceWithLocus {
    l: String,
    s: String,
}

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
        vector_prep(&thekey, &b, workers, db, sta, end, ll, &pg, &rc);
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
        rs_set_str(&thiskey, &workpile.to_string(), &mut redisconn);

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
                    grabworker(Uuid::new_v4(), &cap.clone(), &k, &pg, &rc);
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
        rs_set_str(&thiskey, w.as_str(), &mut redisconn);

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
                rs_del(&thekey, &mut redisconn);
                break;
            } else {
                let mut thiskey = format!("{}_results", &thekey);
                rs_sadd(&thiskey, &data.dump(), &mut redisconn);
                thiskey = format!("{}_hitcount", &thekey);
                rs_set_int(&thiskey, hits + 1, &mut redisconn);
            }
        }
    }
    Ok(())
}

fn vector_prep(k: &str, b: &str, t: i32, db: &str, s: i32, e: i32, ll: i32, psq: &str, rc: &str) {
    // VECTOR PREP builds bags for modeling; to do this you need to...
    //
    // [a] grab db lines that are relevant to the search
    // [b] turn them into a unified text block
    // [c] do some preliminary cleanups
    // [d] break the text into sentences and assemble []SentenceWithLocus (NB: these are "unlemmatized bags of words")
    // [e] figure out all of the words used in the passage
    // [f] find all of the parsing info relative to these words
    // [g] figure out which headwords to associate with the collection of words
    // [h] build the lemmatized bags of words ('unlemmatized' can skip [f] and [g]...)
    // [i] store the bags
    //
    // once you reach this point python can fetch the bags and then run "Word2Vec(bags, parameters, ...)"
    //

    // https://doc.rust-lang.org/std/time/struct.SystemTime.html
    let start = Instant::now();

    let m = format!("Seeking to build {} bags of words", &b);
    lfl(m, ll, 1);

    let mut rc = redisconnect(rc.to_string());
    let mut pg = postgresconnect(psq.to_string());

    // turn of progress logging
    let thiskey = format!("{}_poolofwork", &k);
    rs_set_int(&thiskey, -1, &mut rc);
    let thiskey = format!("{}_hitcount", &k);
    rs_set_int(&thiskey, 0, &mut rc);

    // [a] grab the db lines
    if &k == &"rusttest" {
        let m = format!("No redis key; gathering lines with a direct CLI PostgreSQL query)");
        lfl(m, ll, 1);
        // otherwise we will mimic grabworker() pattern to aggregate the lines
    }

    let dblines: Vec<DBLine> = match &k {
        // either db_directfetch()
        // otherwise we will mimic grabworker() pattern to aggregate the lines
        &"rusttest" => db_directfetch(db, s, e, &mut pg),
        _ => db_redisfectch(),
    };

    let duration = start.elapsed();
    let m = format!("dblines fetched [A: {}]", format_duration(duration).to_string());
    lfl(m, ll, 2);

    // [b] turn them into a unified text block
    // yes, but what is the fastest way...? cf. the huge golang speedup via strings.Builder
    // https://maxuuell.com/blog/how-to-concatenate-strings-in-rust
    // https://stackoverflow.com/questions/30154541/how-do-i-concatenate-strings
    // we have a vector; we want an array so we can try Array.concat()
    // https://stackoverflow.com/questions/29570607/is-there-a-good-way-to-convert-a-vect-to-an-array
    // but is it really possible to generate an array? "arrays cannot have values added or removed at runtime"

    let txtlines: Vec<String> = dblines.iter()
        .map(|x| format!{"⊏line/{}/{}⊐{} ", x.uid, x.idx, x.mu})
        .collect();

    let fulltext: String = txtlines.join(" ");
    // println!("{}", fulltext);

    let duration = start.elapsed();
    let m = format!("unified text block built [B: {}]", format_duration(duration).to_string());
    lfl(m, ll, 2);

    // [c] do some preliminary cleanups
    // parsevectorsentences()

    let strip = vec!["&nbsp;", "- ", "<.*?>"];
    let re_array: Vec<Regex> = strip.iter().map(|x| Regex::new(x).unwrap()).collect();
    let fulltext = sv_stripper(fulltext.as_str(), re_array);
    // println!("stripped\n{}", fulltext);

    let duration = start.elapsed();
    let m = format!("preliminary cleanups complete [C: {}]", format_duration(duration).to_string());
    lfl(m, ll, 2);

    // [d] break the text into sentences and assemble SentencesWithLocus

    // from the .split() documentation:
    // If the pattern is a slice of chars, split on each occurrence of any of the characters:
    // let v: Vec<&str> = "2020-11-03 23:59".split(&['-', ' ', ':', '@'][..]).collect();
    // assert_eq!(v, ["2020", "11", "03", "23", "59"]);

    let terminations: Vec<char> = vec!['.', '?', '!', '·', ';'];
    let splittext: Vec<&str> = fulltext.split(&terminations[..]).collect();

    let sentenceswithlocus: HashMap<String, String> = sv_buildsentences(splittext);

    // for (key, value) in &sentenceswithlocus {
    //     let m = format!("{}: {}", key, value);
    //     lfl(m, 0, 0);
    // }

    let duration = start.elapsed();
    let m = format!("found {} sentences [D: {}]", sentenceswithlocus.len(), format_duration(duration).to_string());
    lfl(m, ll, 2);

    // unlemmatized bags of words customers have in fact reached their target as of now
    if &b == &"unlemmatized" {
        // dropstopwords
        // loadthebags
        // print the result key
        println!("unlemmatized bags of words not yet supported");
        std::process::exit(1);
    }

    // [e] figure out all of the words used in the passage

    let sentences: Vec<&str> = sentenceswithlocus.keys().map(|x| sentenceswithlocus[x].as_str()).collect();
    let allwords: Vec<&str> = sv_findallwords(sentences.clone());

    let duration = start.elapsed();
    let m = format!("found {} words [E: {}]", allwords.len(), format_duration(duration).to_string());
    lfl(m, ll, 2);

    // [f] find all of the parsing info relative to these words

    let  mo: HashMap<String, DbMorphology> = sv_getrequiredmorphobjects(allwords, &mut pg);

    let duration = start.elapsed();
    let m = format!("found {} morphology objects [F: {}]", mo.len(), format_duration(duration).to_string());
    lfl(m, ll, 2);

    // [g] figure out which headwords to associate with the collection of words
    // see convertmophdicttodict()
    // a set of sets
    //	key = word-in-use
    //	value = { maybeA, maybeB, maybeC}
    // {'θεῶν': {'θεόϲ', 'θέα', 'θεάω', 'θεά'}, 'πώ': {'πω'}, 'πολλά': {'πολύϲ'}, 'πατήρ': {'πατήρ'}, ... }

    lazy_static! {
        // this regex matches the python for good/ill...
        static ref POSSPARSE: Regex = Regex::new("(<possibility_([0-9]{1,2})>)(.*?)<xref_value>(.*?)</xref_value><xref_kind>(.*?)</xref_kind>(.*?)</possibility_[0-9]{1,2}>").unwrap();
        }

    let mut morphmap: HashMap<String, HashMap<String, bool>> = HashMap::new();
    for m in mo.keys() {
        // unpack the unique possibilities
        let pp: Vec<MorphPossibility> = mo[m].upo.keys().into_iter()
            .map(|k| sv_getpossiblemorph(k.to_string(), m.clone(), POSSPARSE.clone()))
            .collect();
        // add them to the collection of possibilities or generate a new slot for them in the collection
        for p in pp {
            let mut y: HashMap<String, bool> = HashMap::new();
            y.insert(p.ent.clone(), true);
            if morphmap.contains_key(&p.obs) {
                // X is already present in 'morphdict'; need to add this headword to the set of headwords

                // morphmap[&p.obs].insert(p.ent.clone(), true);
                // above fails to build with:
                //  ^^^^^^^^^^^^^^^^ cannot borrow as mutable
                //  help: trait `IndexMut` is required to modify indexed content, but it is not implemented for `HashMap<std::string::String, HashMap<std::string::String, bool>>`

                // https://stackoverflow.com/questions/30414424/how-can-i-update-a-value-in-a-mutable-hashmap
                // https://doc.rust-lang.org/std/collections/struct.HashMap.html#method.get_mut
                // let mut map = HashMap::new();
                // map.insert(1, "a");
                // if let Some(x) = map.get_mut(&1) {
                //     *x = "b";
                // }
                // assert_eq!(map[&1], "b");

                if let Some(x) = morphmap.get_mut(&p.obs) {
                    *x = y;
                }
            } else {
                // initialize and insert...
                morphmap.insert(p.obs.clone(), y);
            }
        }
    }

    // no need for the "bool" any longer; demap things

    let mut mm: HashMap<String, Vec<&str>> = HashMap::new();
    for m in morphmap.keys() {
        mm.insert(m.to_string(), morphmap[m].keys().map(|k| k.as_str()).collect());
    }

    let duration = start.elapsed();
    let m = format!("Built morphmap [G: {}]", format_duration(duration).to_string());
    lfl(m, ll, 2);

    // [h] build the lemmatized bags of words

    if b == "flat" {
        let bagged: Vec<String> = sv_buildflatbags(sentences.to_owned(), mm);
    } else if b == "alternates" {
        let bagged: Vec<String> = sv_buildcompositebags(sentences.to_owned(), mm);
    } else if b == "winnertakesall" {
        let bagged: Vec<String> = sv_buildwinnertakesallbags(sentences.to_owned(), mm);
    } else {
        // should never hit this but...
        let bagged: Vec<String> = sentences.iter().map(|s| s.to_string()).collect();
    }

    // [i] purge stopwords


    // [j] store...


    std::process::exit(1);
}

fn websocket(ft: &str, ll: i32, ip: &str, port: &str, rc: String) {
    //  WEBSOCKETS broadcasts search information for web page updates
    //
    //	[a] it launches and starts listening on a port
    //	[b] it waits to receive a websocket message: this is a search key ID (e.g., '2f81c630')
    //	[c] it then looks inside of redis for the relevant polling data associated with that search ID
    //	[d] it parses, packages (as JSON), and then redistributes this information back over the websocket
    //	[e] when the poll disappears from redis, the messages stop broadcasting
    //

    // INCOMPLETE relative to the golang version
    // still missing:
    // deletewhendone()

    let listen = format!("{}:{}", ip, port);
    let failthreshold: u32 = ft.parse().unwrap();

    // https://github.com/snapview/tungstenite-rs/blob/master/examples/server.rs
    env_logger::init();
    let server = TcpListener::bind(listen).unwrap();
    for stream in server.incoming() {
        let r = rc.clone();
        spawn(move || {
            let callback = |_req: &Request, r: Response| {
                Ok(r)
            };

            // [a] it launches and starts listening on a port
            let mut ws: WebSocket<TcpStream> = accept_hdr(stream.unwrap(), callback).unwrap();

            loop {
                // [b] it waits to receive a websocket message: this is a search key ID (e.g., '2f81c630')
                let msg = ws.read_message().unwrap();
                if msg.is_text() {
                    let mut redisconn = redisconnect(r.to_string());
                    let rk = String::from(msg.to_text().unwrap());

                    // at this point you have "ebf24e19" and NOT ebf24e19; fix that
                    let rk2 = String::from(rk.strip_prefix("\"").unwrap());
                    let rediskey = rk2.strip_suffix("\"").unwrap();

                    let f = ws_fields();
                    let mut results: HashMap<String, String> = HashMap::new();
                    let mut missing: u32 = 0u32;
                    let mut iterations: u32 = 0u32;

                    // this is the polling loop
                    loop {
                        thread::sleep(POLLINGINTERVAL);
                        iterations += 1;
                        let m: String = format!("WebSocket server reports that runpollmessageloop() for {} is on iteration {}", &rediskey, &iterations);
                        lfl(m, ll, 3);

                        // [c] it then looks inside of redis for the relevant polling data associated with that search ID
                        for i in &f {
                            let thekey: String = format!("{}_{}", rediskey, i);

                            let mut capkey = i.to_owned().to_string();
                            make_ascii_title_case(&mut capkey);

                            let v = rs_get(&thekey, &mut redisconn);

                            // [d] it parses, packages (as JSON), and then redistributes this information back over the websocket
                            // [d1] insert as {"Launchtime": "1622578053.906691"}
                            results.insert(capkey, v);
                        }

                        // watch activity
                        // for (key, value) in &results {
                        //     let m = format!("{}: {}", key, value);
                        //     lfl(m, 0, 0);
                        // }

                        let a = results.get(&"Active".to_string()).unwrap();
                        if a != "yes" {
                            missing += 1;
                        }

                        // break if inactive
                        if missing >= failthreshold {
                            let m: String = format!("WebSocket broadcasting for {} halting after {} iterations: missing >= failthreshold", &rediskey, &iterations);
                            lfl(m, ll, 1);
                            break
                        }

                        // [d2] package (as JSON)
                        let js = ws_jsonifyresults(&rediskey, results.clone());
                        // [d3] redistribute this information
                        ws.write_message(Message::text(js.dump())).unwrap();
                    }

                    //	[e] when the poll disappears from redis, the messages stop broadcasting
                    // INCOMPLETE relative to the golang version
                    // still missing:
                    // deletewhendone()
                }
            }
        });
    }
}

fn ws_jsonifyresults(rediskey: &str, pd: HashMap<String, String>) -> json::JsonValue {
    // https://docs.rs/json/0.12.4/json/
    // see: "Putting fields on objects"
    let mut data = JsonValue::new_object();
    for (key, val) in &pd {
        data[key] = val.clone().into();
    }
    data["ID"] = rediskey.into();
    data
}

fn ws_fields<'a>() ->  Vec<&'a str> {
    // be careful about the key capitalization issue: go has "Active", etc.
    let fld = "launchtime active statusmessage remaining poolofwork hitcount portnumber notes";
    let v: Vec<&str> = fld.split_whitespace().collect();
    v
}

fn db_sv_get_morphobjects(words: &mut Vec<&str>, lang: &str, pg: &mut postgres::Client) -> Vec<DbMorphology> {
    // look for the upper case matches too: Ϲωκράτηϲ and not just ϲωκρατέω (!)
    // let start = Instant::now();

    let mut wordswithcaps: Vec<String> = words.iter().map(|w| str_cap(w)).collect();
    let mut w = words.iter().map(|w| w.to_string()).collect();
    wordswithcaps.append(&mut w);

    // let tt: &str= "CREATE TEMPORARY TABLE ttw_{} AS SELECT words AS w FROM unnest(ARRAY[{}]) words";
    // let qt: &str= "SELECT observed_form, xrefs, prefixrefs, possible_dictionary_forms FROM {}_morphology WHERE EXISTS (SELECT 1 FROM ttw_{} temptable WHERE temptable.w = {}_morphology.observed_form)";

    let mut rndid = Uuid::new_v4().to_string();
    rndid.retain(|c| c != '-');

    let ttarr = wordswithcaps.join("', '");
    let ttarr = format!("'{}'", ttarr);
    let t = format!("CREATE TEMPORARY TABLE ttw_{} AS SELECT words AS w FROM unnest(ARRAY[{}]) words", &rndid, ttarr);

    // let duration = start.elapsed();
    // let m = format!("TT [α: {}]", format_duration(duration).to_string());
    // lfl(m, 0, 0);

    // println!("{}", &t);
    pg.execute(t.as_str(), &[]).ok().expect("db_arraytogetrequiredmorphobjects() TempTable creation failed");

    lazy_static! {
        static ref POSS : Regex = Regex::new("<possibility_[0-9]{1,2}>.*?</possibility_[0-9]{1,2}>").unwrap();
    }

    let q = format!("SELECT observed_form, xrefs, prefixrefs, possible_dictionary_forms FROM {}_morphology WHERE EXISTS (SELECT 1 FROM ttw_{} temptable WHERE temptable.w = {}_morphology.observed_form)", &lang, rndid, &lang);
    let dbmo = pg.query(q.as_str(), &[]).unwrap().into_iter()
        .map(|row| DbMorphology {
            obs: row.get("observed_form"),
            xrf: row.get("xrefs"),
            pxr: row.get("prefixrefs"),
            rpo: row.get("possible_dictionary_forms"),
            upo: sv_updatesetofpossibilities(row.get("possible_dictionary_forms"), POSS.clone()),
        }).collect::<Vec<DbMorphology>>();

    // let duration = start.elapsed();
    // let m = format!("DBMO [β: {}]", format_duration(duration).to_string());
    // lfl(m, 0, 0);

    dbmo
}

fn sv_updatesetofpossibilities(rpo: String, re: Regex) -> HashMap<String, bool> {
    // a new collection of possibilities has arrived <p1>xxx</p1><p2>yyy</p2>...
    // parse this string for a list of possibilities; then add its elements to the set of known possibilities
    // return the updated set
    let mut morph: HashMap<String, bool> = HashMap::new();
    for f in re.find_iter(rpo.as_str()) {
        morph.insert(String::from(f.as_str()), true);
    }
    morph
}

fn db_fields<'a>() ->  Vec<&'a str> {
    // used to prep the json encoding for a dbworkline
    let fld = "WkUID TbIndex Lvl5Value Lvl4Value Lvl3Value Lvl2Value Lvl1Value Lvl0Value MarkedUp Accented Stripped Hypenated Annotations";
    let v: Vec<&str> = fld.split_whitespace().collect();
    v
}

fn db_directfetch(t: &str, s: i32, e: i32, pg: &mut postgres::Client) -> Vec<DBLine> {
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

fn db_redisfectch() -> Vec<DBLine> {
    // hollow placeholder for now
    let l = DBLine { idx: 1, uid: "".to_string(), l5: "".to_string(), l4: "".to_string(),
        l3: "".to_string(), l2: "".to_string(), l1: "".to_string(), l0: "".to_string(),
        mu: "(this is a hollow placeholder)".to_string(), ac: "".to_string(), st: "".to_string(), hy: "".to_string(),
        an: "".to_string() };

    let v = vec![l];
    v
}

fn rs_del(k: &str, c: &mut redis::Connection) -> redis::RedisResult<()> {
    // DEL
    let _ : () = c.del(k)?;
    Ok(())
}

fn rs_scard(k: &str, c: &mut redis::Connection) -> i32 {
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

fn rs_spop(k: &str, c: &mut redis::Connection) -> String {
    // SPOP
    let p: String = match redis::cmd("SPOP")
        .arg(k)
        .query(c) {
        Ok(s) => s,
        Err(_e) => "".to_string(),
    };
    p
}

fn rs_get(k: &str, c: &mut redis::Connection) -> String {
    // GET
    let p: String = match redis::cmd("GET")
        .arg(k)
        .query(c) {
        Ok(s) => s,
        Err(_e) => "".to_string(),
    };

    // if &p == &"" {
    //     let m = format!("GET failed for '{}'; returning an empty string", k);
    //     lfl(m, 0, 0)
    // }

    p
}

fn rs_sadd(k: &str, v: &str, c: &mut redis::Connection) -> redis::RedisResult<()> {
    // SADD
    let _ : () = c.sadd(k, v).unwrap_or(());
    Ok(())
}

fn rs_set_str(k: &str, v: &str, c: &mut redis::Connection) -> redis::RedisResult<()> {
    // SET
    let _ : () = c.set(k, v).unwrap_or(());
    Ok(())
}

fn rs_set_int(k: &str, v: i32, c: &mut redis::Connection) -> redis::RedisResult<()> {
    // SET
    let _ : () = c.set(k, v).unwrap_or(());
    Ok(())
}

fn sv_stripper(text: &str, topurge: Vec<Regex>) -> String {
    // https://github.com/rust-lang/regex/blob/master/examples/shootout-regex-dna-replace.rs
    // avoid compliling regex in a loop: it is a killer...
    let mut newtext = String::from(text);
    for r in topurge {
        newtext = r.replace_all(&newtext, "").into_owned();
    }
    newtext
}

fn sv_buildsentences(splittext: Vec<&str>) -> HashMap<String, String> {
    // if HashMap<&str, &str> compile error: returns a value referencing data owned by the current function
    // see: https://stackoverflow.com/questions/32682876/is-there-any-way-to-return-a-reference-to-a-variable-created-in-a-function
    // "Instead of trying to return a reference, return an owned object. String instead of &str, Vec<T> instead of &[T], T instead of &T, etc."]

    lazy_static! {
        static ref TAGGER : Regex = Regex::new(" ⊏.*?⊐").unwrap();
        static ref NOTCHAR : Regex = Regex::new("[^ a-zα-ωϲϹἀἁἂἃἄἅἆἇᾀᾁᾂᾃᾄᾅᾆᾇᾲᾳᾴᾶᾷᾰᾱὰάἐἑἒἓἔἕὲέἰἱἲἳἴἵἶἷὶίῐῑῒΐῖῗὀὁὂὃὄὅόὸὐὑὒὓὔὕὖὗϋῠῡῢΰῦῧύὺᾐᾑᾒᾓᾔᾕᾖᾗῂῃῄῆῇἤἢἥἣὴήἠἡἦἧὠὡὢὣὤὥὦὧᾠᾡᾢᾣᾤᾥᾦᾧῲῳῴῶῷώὼ]").unwrap();
        static ref LOCC : Regex = Regex::new("⊏(.*?)⊐").unwrap();
        }

    let mut sentenceswithlocus: HashMap<String, String> = HashMap::new();

    for s in splittext {
        let lcs = s.to_string().to_lowercase();
        let thesentence = TAGGER.replace_all(&lcs, "").into_owned();
        let thesentence = NOTCHAR.replace_all(&thesentence, "").into_owned();
        let firsthit: String = match LOCC.captures(s.clone()) {
            None => "".to_string(),
            Some(x) => x[1].to_string(),
        };
        sentenceswithlocus.insert(firsthit, thesentence);
    }
    sentenceswithlocus
}

fn sv_buildflatbags(s: Vec<&str>, mm: HashMap<String, Vec<&str>>) -> Vec<String> {
    // turn a list of sentences into a list of list of headwords; here we put alternate possibilities next to one another:
    // flatbags: ϲυγγενεύϲ ϲυγγενήϲ
    // composite: ϲυγγενεύϲ·ϲυγγενήϲ

    let hollow: Vec<String> = Vec::new();
    hollow
}

fn sv_buildcompositebags(s: Vec<&str>, mm: HashMap<String, Vec<&str>>) -> Vec<String> {
    // turn a list of sentences into a list of list of headwords; here we put yoked alternate possibilities next to one another:
    // flatbags: ϲυγγενεύϲ ϲυγγενήϲ
    // composite: ϲυγγενεύϲ·ϲυγγενήϲ
    let hollow: Vec<String> = Vec::new();
    hollow
}

fn sv_buildwinnertakesallbags(s: Vec<&str>, mm: HashMap<String, Vec<&str>>) -> Vec<String> {
    // turn a list of sentences into a list of list of headwords; here we figure out which headword is the dominant homonym
    // then we just use that term; "esse" always comes from "sum" and never "edo", etc.

    // [a] figure out all headwords in use

    // [b] assign scores to each of them

    // [c] note that there are capital words in here that need lowering
    // [c1] lower the internal values first

    // [c2] lower the keys; how worried should we be about the collisions...

    // [d] run through the parser map and kill off the losers

    // [e] now you can just sv_buildflatbags() with the new pruned parser map

    let hollow: Vec<String> = Vec::new();
    hollow
}

fn sv_findallwords(sentences: Vec<&str>) -> Vec<&str> {
    let mut allwords: HashMap<&str, bool> = HashMap::new();
    for s in sentences {
        let words: Vec<&str> = s.split_whitespace().collect();
        for w in words {
            allwords.insert(w, true);
        }
    };
    let thewords = allwords.keys().map(|x| *x ).collect();
    thewords
}

fn sv_getrequiredmorphobjects(words: Vec<&str>, pg: &mut postgres::Client) -> HashMap<String, DbMorphology> {
    let latintest = Regex::new("[a-z]+").unwrap();
    // let greektest = Regex::new("[α-ωϲἀἁἂἃἄἅἆἇᾀᾁᾂᾃᾄᾅᾆᾇᾲᾳᾴᾶᾷᾰᾱὰάἐἑἒἓἔἕὲέἰἱἲἳἴἵἶἷὶίῐῑῒΐῖῗὀὁὂὃὄὅόὸὐὑὒὓὔὕὖὗϋῠῡῢΰῦῧύὺᾐᾑᾒᾓᾔᾕᾖᾗῂῃῄῆῇἤἢἥἣὴήἠἡἦἧὠὡὢὣὤὥὦὧᾠᾡᾢᾣᾤᾥᾦᾧῲῳῴῶῷώὼ]+").unwrap();
    let mut latinwords: Vec<&str> = Vec::new();
    let mut greekwords: Vec<&str> = Vec::new();
    for w in words {
        // note that we are hereby implying that there are only two languages possible...
        if latintest.is_match(w) {
            latinwords.push(w);
        } else {
            greekwords.push(w);
        }
    }
    println!("l: {}; g: {}", latinwords.len(), greekwords.len());

    let mut morph: Vec<DbMorphology> = db_sv_get_morphobjects(&mut latinwords, "latin", pg);
    let mut grmorph: Vec<DbMorphology> = db_sv_get_morphobjects(&mut greekwords, "greek", pg);

    morph.append(&mut grmorph);

    let mut mo: HashMap<String, DbMorphology> = HashMap::new();
    for m in morph {
        mo.insert(m.obs.clone(), m.clone());
    }
    mo
}

fn sv_getpossiblemorph(ob: String, po: String, re: Regex) -> MorphPossibility {
    //     let pf = "(<possibility_([0-9]{1,2})>)(.*?)<xref_value>(.*?)</xref_value><xref_kind>(.*?)</xref_kind>(.*?)</possibility_[0-9]{1,2}>";
    //     let re = Regex::new(pf).unwrap();
    //
    //     let p = "<possibility_2>bellī, bellus<xref_value>8636495</xref_value><xref_kind>9</xref_kind><transl>A. pretty; B. every thing beautiful; A. Gallant; B. good</transl><analysis>masc nom/voc pl</analysis></possibility_2>";
    //
    //     let c = re.captures(p).unwrap();
    //
    //     for i in 0..7 {
    //         let t = c.get(i).map_or("", |m| m.as_str());
    //         println!("{}: {}", i, t);
    //     }
    // 0: <possibility_2>bellī, bellus<xref_value>8636495</xref_value><xref_kind>9</xref_kind><transl>A. pretty; B. every thing beautiful; A. Gallant; B. good</transl><analysis>masc nom/voc pl</analysis></possibility_2>
    // 1: <possibility_2>
    // 2: 2
    // 3: bellī, bellus
    // 4: 8636495
    // 5: 9
    // 6: <transl>A. pretty; B. every thing beautiful; A. Gallant; B. good</transl><analysis>masc nom/voc pl</analysis>

    let c = re.captures(po.as_str());
    match c {
        Some(v) => {
            let n = v.get(2).map_or("", |m| m.as_str());
            let e = v.get(3).map_or("", |m| m.as_str());
            let x = v.get(4).map_or("", |m| m.as_str());
            let a = v.get(6).map_or("", |m| m.as_str());
            let mp: MorphPossibility = MorphPossibility {
                obs: ob,
                num: n.to_string(),
                ent: e.to_string(),
                xrf: x.to_string(),
                ana: a.to_string(),
            };
            return mp
        }
        None => {
            let mp: MorphPossibility = MorphPossibility {
                obs: ob,
                num: "".to_string(),
                ent: "".to_string(),
                xrf: "".to_string(),
                ana: "".to_string(),
            };
            return mp
        }
    }
}

fn postgresconnect(j: String) -> postgres::Client {
    // https://docs.rs/postgres/0.19.1/postgres/
    // https://rust-lang-nursery.github.io/rust-cookbook/database/postgres.html
    let parsed = json::parse(&j).unwrap();
    let host = parsed["Host"].as_str().unwrap();
    let user = parsed["User"].as_str().unwrap();
    let db = parsed["DBName"].as_str().unwrap();
    let port = parsed["Port"].as_i32().unwrap();
    let pw = parsed["Pass"].as_str().unwrap();
    let validate = format!("host={} user={} dbname={} port={} password={}", &host, &user, &db, &port, &pw);
    let mut client = Client::connect(&validate, NoTls).expect("failed to connect to postgreSQL");
    client
}

fn redisconnect(j: String) -> redis::Connection {
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

// https://stackoverflow.com/questions/38406793/why-is-capitalizing-the-first-letter-of-a-string-so-convoluted-in-rust/53571882#53571882
fn make_ascii_title_case(s: &mut str) {
    if let Some(r) = s.get_mut(0..1) {
        r.make_ascii_uppercase();
    }
}

fn str_cap(s: &str) -> String {
    // if we are not using ascii strings...
    format!("{}{}", s.chars().next().unwrap().to_uppercase(),
            s.chars().skip(1).collect::<String>())
}

fn lfl(message: String, loglevel: i32, threshold: i32) {
    // log if logging
    if loglevel >= threshold {
        println!("[{}] {}", SHORTNAME, message);
    }
}