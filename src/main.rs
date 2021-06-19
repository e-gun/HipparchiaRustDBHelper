//    HipparchiaRustDBHelper: search and vector helper app and functions for HipparchiaServer
//    Copyright: E Gunderson 2016-21
//    License: GNU GENERAL PUBLIC LICENSE 3
//        (see LICENSE in the top level directory of the distribution)

use std::{net::TcpListener, thread::spawn};
use std::{thread, time};
use std::collections::HashMap;
use std::net::TcpStream;
use std::convert::TryFrom;
use std::time::{Instant};

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
static VERSION: &str = "0.0.6";
static POLLINGINTERVAL: time::Duration = time::Duration::from_millis(400);
static TESTDB: &str = "lt0448";
static TESTSTART: &str = "1";
static TESTEND: &str = "26";
static TESTKEY: &str = "rusttest";
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
    upo: Vec<String>,
}

struct MorphPossibility {
    obs: String,
    num: String,
    ent: String,
    xrf: String,
    ana: String,
}

struct WeightedHeadword {
    wd: String,
    ct: i32,
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

fn vector_prep(thekey: &str, b: &str, workers: i32, db: &str, s: i32, e: i32, ll: i32, psq: &str, rca: &str) {
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

    let mut rc = redisconnect(rca.to_string());
    let mut pg = postgresconnect(psq.to_string());

    // turn of progress logging
    let thiskey = format!("{}_poolofwork", &thekey);
    rs_set_int(&thiskey, -1, &mut rc).unwrap();
    let thiskey = format!("{}_hitcount", &thekey);
    rs_set_int(&thiskey, 0, &mut rc).unwrap();

    // [a] grab the db lines
    if &thekey == &"rusttest" {
        let m = format!("No redis key; gathering lines with a direct CLI PostgreSQL query)");
        lfl(m, ll, 1);
        // otherwise we will mimic grabworker() pattern to aggregate the lines
    }

    let dblines: Vec<DBLine> = match &thekey {
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

    let  mo: Vec<DbMorphology> = sv_getrequiredmorphobjects(allwords, &mut pg);

    let duration = start.elapsed();
    let m = format!("found {} morphology objects [F: {}]", mo.len(), format_duration(duration).to_string());
    lfl(m, ll, 2);

    // [g] figure out which headwords to associate with the collection of words
    // see convertmophdicttodict()
    // a set of sets
    //	key = word-in-use
    //	value = { maybeA, maybeB, maybeC}
    // {'θεῶν': {'θεόϲ', 'θέα', 'θεάω', 'θεά'}, 'πώ': {'πω'}, 'πολλά': {'πολύϲ'}, 'πατήρ': {'πατήρ'}, ... }

    let mut morphmap: HashMap<String, Vec<String>> = HashMap::new();
    for m in mo {
        morphmap.insert(m.obs, m.upo);
    }

    let duration = start.elapsed();
    let m = format!("Built morphmap [G: {}]", format_duration(duration).to_string());
    lfl(m, ll, 2);

    // [h] build the lemmatized bags of words

    let mut bagged: Vec<String> = Vec::new();

    if b == "flat" {
        bagged = sv_buildflatbags(sentences.to_owned(), morphmap);
    } else if b == "alternates" {
        bagged =  sv_buildcompositebags(sentences.to_owned(), morphmap);
    } else if b == "winnertakesall" {
        bagged =  sv_buildwinnertakesallbags(sentences.to_owned(), morphmap, &mut pg);
    } else {
        // should never hit this but...
        println!("UNKNOWN BAGGING METHOD ['{}']; you will get 'unlemmatized' bags instead", b);
        bagged =  sentences.iter().map(|s| s.to_string()).collect();
    }

    let duration = start.elapsed();
    let m = format!("Built {} bags [H: {}]", bagged.len(), format_duration(duration).to_string());
    lfl(m, ll, 2);

    // [i] purge stopwords

    let skipheadwords = "unus verum omne sum¹ ab δύο πρότεροϲ ἄνθρωποϲ τίϲ δέω¹ ὅϲτιϲ homo πᾶϲ οὖν εἶπον ἠμί ἄν² tantus μένω μέγαϲ οὐ verus neque eo¹ nam μέν ἡμόϲ aut Sue διό reor ut ἐγώ is πωϲ ἐκάϲ enim ὅτι² παρά ἐν Ἔχιϲ sed ἐμόϲ οὐδόϲ ad de ita πηρόϲ οὗτοϲ an ἐπεί a γάρ αὐτοῦ ἐκεῖνοϲ ἀνά ἑαυτοῦ quam αὐτόϲε et ὑπό quidem Alius¹ οἷοϲ noster γίγνομαι ἄνα προϲάμβ ἄν¹ οὕτωϲ pro² tamen ἐάν atque τε qui² si do multus λόγοϲ idem οὐδέ ἐκ omnes γε causa δεῖ πολύϲ in ἔδω ὅτι¹ μή Ios ἕτεροϲ cum meus ὅλοξ suus omnis ὡϲ sua μετά Ἀλλά ne¹ jam εἰϲ ἤ² ἄναξ ἕ ὅϲοϲ dies ipse ὁ hic οὐδείϲ suo ἔτι ἄνω¹ ὅϲ νῦν ὁμοῖοϲ edo¹ εἰ qui¹ πάλιν ὥϲπερ ne³ ἵνα τιϲ διά φύω per τοιοῦτοϲ for eo² huc locum neo¹ sui non ἤ¹ χάω ex κατά δή ἁμόϲ dico² ὅμοιοϲ αὐτόϲ etiam vaco πρόϲ Ζεύϲ ϲύ quis¹ tuus b εἷϲ Eos οὔτε τῇ καθά ego tu ille pro¹ ἀπό suum εἰμί ἄλλοϲ δέ alius² pars vel ὥϲτε χέω res ἡμέρα quo δέομαι modus ὑπέρ ϲόϲ ito τῷ περί Τήιοϲ ἕκαϲτοϲ autem καί ἐπί nos θεάω γάρον γάροϲ Cos²";
    let skipinflected = "ita a inquit ego die nunc nos quid πάντων ἤ με θεόν δεῖ for igitur ϲύν b uers p ϲου τῷ εἰϲ ergo ἐπ ὥϲτε sua me πρό sic aut nisi rem πάλιν ἡμῶν φηϲί παρά ἔϲτι αὐτῆϲ τότε eos αὐτούϲ λέγει cum τόν quidem ἐϲτιν posse αὐτόϲ post αὐτῶν libro m hanc οὐδέ fr πρῶτον μέν res ἐϲτι αὐτῷ οὐχ non ἐϲτί modo αὐτοῦ sine ad uero fuit τοῦ ἀπό ea ὅτι parte ἔχει οὔτε ὅταν αὐτήν esse sub τοῦτο i omnes break μή ἤδη ϲοι sibi at mihi τήν in de τούτου ab omnia ὃ ἦν γάρ οὐδέν quam per α autem eius item ὡϲ sint length οὗ λόγον eum ἀντί ex uel ἐπειδή re ei quo ἐξ δραχμαί αὐτό ἄρα ἔτουϲ ἀλλ οὐκ τά ὑπέρ τάϲ μάλιϲτα etiam haec nihil οὕτω siue nobis si itaque uac erat uestig εἶπεν ἔϲτιν tantum tam nec unde qua hoc quis iii ὥϲπερ semper εἶναι e ½ is quem τῆϲ ἐγώ καθ his θεοῦ tibi ubi pro ἄν πολλά τῇ πρόϲ l ἔϲται οὕτωϲ τό ἐφ ἡμῖν οἷϲ inter idem illa n se εἰ μόνον ac ἵνα ipse erit μετά μοι δι γε enim ille an sunt esset γίνεται omnibus ne ἐπί τούτοιϲ ὁμοίωϲ παρ causa neque cr ἐάν quos ταῦτα h ante ἐϲτίν ἣν αὐτόν eo ὧν ἐπεί οἷον sed ἀλλά ii ἡ t te ταῖϲ est sit cuius καί quasi ἀεί o τούτων ἐϲ quae τούϲ minus quia tamen iam d διά primum r τιϲ νῦν illud u apud c ἐκ δ quod f quoque tr τί ipsa rei hic οἱ illi et πῶϲ φηϲίν τοίνυν s magis unknown οὖν dum text μᾶλλον λόγοϲ habet τοῖϲ qui αὐτοῖϲ suo πάντα uacat τίϲ pace ἔχειν οὐ κατά contra δύο ἔτι αἱ uet οὗτοϲ deinde id ut ὑπό τι lin ἄλλων τε tu ὁ cf δή potest ἐν eam tum μου nam θεόϲ κατ ὦ cui nomine περί atque δέ quibus ἡμᾶϲ τῶν eorum";

    let bagged: Vec<String> = sv_dropstopwords(skipheadwords, bagged);
    let bagged: Vec<String> = sv_dropstopwords(skipinflected, bagged);

    // no empty bags...
    let mut bags: Vec<String> = Vec::new();
    for b in bagged {
        if b.len() > 0 {
            bags.push(b);
        }
    }

    let duration = start.elapsed();
    let m = format!("Purged stopwords in {} bags [I: {}]", bags.len(), format_duration(duration).to_string());
    lfl(m, ll, 2);

    // [j] store...

    let resultkey = format!("{}_vectorresults", &thekey);
    let bl = bags.len();
    sv_loadthebags(resultkey.clone(), bags, workers, rca);

    let duration = start.elapsed();
    let m = format!("Stored {} bags [J: {}]", bl, format_duration(duration).to_string());
    lfl(m, ll, 2);

    println!("{}", resultkey);
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
    // return the fields we are using
    // be careful about the key capitalization issue: go has "Active", etc.
    let fld = "launchtime active statusmessage remaining poolofwork hitcount portnumber notes";
    let v: Vec<&str> = fld.split_whitespace().collect();
    v
}

fn db_sv_get_morphobjects(words: &mut Vec<&str>, lang: &str, pg: &mut postgres::Client) -> Vec<DbMorphology> {
    // the worker for sv_getrequiredmorphobjects()
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

    let q = format!("SELECT observed_form, xrefs, prefixrefs, related_headwords FROM {}_morphology WHERE EXISTS (SELECT 1 FROM ttw_{} temptable WHERE temptable.w = {}_morphology.observed_form)", &lang, rndid, &lang);
    let dbmo = pg.query(q.as_str(), &[]).unwrap().into_iter()
        .map(|row| DbMorphology {
            obs: row.get("observed_form"),
            xrf: row.get("xrefs"),
            pxr: row.get("prefixrefs"),
            rpo: row.get("related_headwords"),
            upo: row.get::<&str, String>("related_headwords").split_whitespace().map(|s| s.to_string()).collect(),
        }).collect::<Vec<DbMorphology>>();

    // let duration = start.elapsed();
    // let m = format!("DBMO [β: {}]", format_duration(duration).to_string());
    // lfl(m, 0, 0);

    dbmo
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

fn db_fetchheadwordcounts(hw: Vec<String>, pg: &mut postgres::Client) -> HashMap<String, i32> {

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

fn sv_buildflatbags(ss: Vec<&str>, mm: HashMap<String, Vec<String>>) -> Vec<String> {
    // turn a list of sentences into a list of list of headwords; here we put alternate possibilities next to one another:
    // flatbags: ϲυγγενεύϲ ϲυγγενήϲ
    // composite: ϲυγγενεύϲ·ϲυγγενήϲ
    let re = Regex::new(" {2,}").unwrap();

    let swapper = |sent: &str| {
        let words: Vec<&str> = sent.split_whitespace().collect();
        let mut newwords: Vec<String> = Vec::new();
        for w in words {
            if mm.contains_key(w) {
                let mut unpacked: Vec<String> = mm[w].clone();
                // println!("{}: {:?}", w, unpacked);
                newwords.append(&mut unpacked);
            }
        }
        let newsent: String = newwords.join(" ");
        let newsent = re.replace_all(&newsent, " ").into_owned();
        // println!("{}", newsent);
        newsent
    };

    let bagged:Vec<String> = ss.iter()
        .map(|s| swapper(s) )
        .collect();

    // for b in &bagged {
    //     println!("{}", b);
    // }

    bagged
}

fn sv_buildcompositebags(ss: Vec<&str>, mm: HashMap<String, Vec<String>>) -> Vec<String> {
    // turn a list of sentences into a list of list of headwords; here we put yoked alternate possibilities next to one another:
    // flatbags: ϲυγγενεύϲ ϲυγγενήϲ
    // composite: ϲυγγενεύϲ·ϲυγγενήϲ

    let re = Regex::new(" {2,}").unwrap();

    let swapper = |sent: &str| {
        let words: Vec<&str> = sent.split_whitespace().collect();
        let mut newwords: Vec<String> = Vec::new();
        for w in words {
            if mm.contains_key(w) {
                let yoked = mm[w].clone().join("·");
                println!("{}: {}", w, yoked);
                newwords.push(yoked.clone());
            }
        }
        let newsent: String = newwords.join(" ");
        let newsent = re.replace_all(&newsent, " ").into_owned();
        newsent
    };

    let bagged:Vec<String> = ss.iter()
        .map(|s| swapper(s) )
        .collect();

    // for b in &bagged {
    //     println!("{}", b);
    // }

    bagged
}

fn sv_buildwinnertakesallbags(ss: Vec<&str>, mm: HashMap<String, Vec<String>>, pg: &mut postgres::Client) -> Vec<String> {
    // turn a list of sentences into a list of list of headwords; here we figure out which headword is the dominant homonym
    // then we just use that term; "esse" always comes from "sum" and never "edo", etc.

    // [a] figure out all headwords in use

    let mut hwd: HashMap<String, bool> = HashMap::new();
    for m in mm.keys() {
        for p in &mm[m] {
            hwd.insert(p.to_string(), true);
        }
    }

    // [b] assign scores to each of them
    let wds: Vec<String> = mm.keys().map(|k| k.clone()).collect();
    let scoremap: HashMap<String, i32> = db_fetchheadwordcounts(wds, pg);

    // [c] note that there are capital words in here that need lowering
    // [c1] lower the internal values first

    let mut lchwd: HashMap<String, bool> = HashMap::new();
    for k in hwd.keys() {
        lchwd.insert(str_lcs(k), true);
    }

    // [c2] lower the scoremap keys; how worried should we be about the collisions...

    let mut lcscoremap: HashMap<String, i32> = HashMap::new();
    for k in hwd.keys() {
        if scoremap.contains_key(k) {
            lcscoremap.insert(str_lcs(k), scoremap[k]);
        }
        // lcscoremap.insert(format!("{}{}", k.chars().next().unwrap().to_lowercase(), k.chars().skip(1).collect::<String>()).as_str().clone().unwrap(), scoremap[k]);
    }

    // [d] run through the parser map and kill off the losers

    let mut newparsemap: HashMap<String, String> = HashMap::new();
    for w in lchwd.keys() {
            if mm.contains_key(w) {
                let mut poss: Vec<String> = mm[w].iter()
                    .map(|k| k.to_string())
                    .collect();
                poss.sort_by_key(|k| if scoremap.contains_key(k) { lcscoremap[k] } else { 0 });
                // poss.resize(1, "".to_string());
                newparsemap.insert(w.clone(), poss.pop().unwrap());
            } else {
                newparsemap.insert(w.clone(), w.clone());
            }
    }

    // let mut newparsemap: HashMap<String, Vec<&str>> = HashMap::new();
    // for w in lchwd.keys() {
    //     &mm[w].sort_by_key(|k| &lcscoremap[k]);
    //     newparsemap.insert(w.clone(), mm[w].clone());
    // }

    // [e] now just swap out the words: key points to right new values

    let re = Regex::new(" {2,}").unwrap();

    let swapper = |sent: &str| {
        let words: Vec<&str> = sent.split_whitespace().collect();
        let mut newwords: Vec<String> = Vec::new();
        for w in words {
            if newparsemap.contains_key(w) {
                newwords.push(newparsemap[w].clone());
            }
        }
        let newsent: String = newwords.join(" ");
        let newsent = re.replace_all(&newsent, " ").into_owned();
        newsent
    };

    let bagged = ss.iter().map(|s| swapper(s) )
        .collect();

    // for b in &bagged {
    //     println!("{}", b);
    // }

    bagged
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

fn sv_getrequiredmorphobjects(words: Vec<&str>, pg: &mut postgres::Client) -> Vec<DbMorphology> {
    // we need DbMorphology to build our bags; grab it
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

    // println!("l: {}; g: {}", latinwords.len(), greekwords.len());

    let mut morph: Vec<DbMorphology> = db_sv_get_morphobjects(&mut latinwords, "latin", pg);
    let mut grmorph: Vec<DbMorphology> = db_sv_get_morphobjects(&mut greekwords, "greek", pg);

    morph.append(&mut grmorph);
    morph
}

fn sv_dropstopwords(todrop: &str, bags: Vec<String>) -> Vec<String> {
    // purge stopwords from the bags
    let vv: Vec<&str> = todrop.split_whitespace().collect();
    let mut stopmap: HashMap<&str, bool> = HashMap::new();
    for v in vv { stopmap.insert(v, true); }

    let mut cleaned: Vec<String> = Vec::new();
    for b in bags {
        let ww: Vec<&str> = b.split_whitespace().collect();
        let mut ns: Vec<&str> = Vec::new();
        for w in ww {
            if stopmap.contains_key(w) {
                continue;
            } else {
                ns.push(w);
            }
        }
        cleaned.push(ns.join(" "));
    }
    cleaned
}

fn sv_parallelbagloader(id: Uuid, key: String, bags: Vec<String>, c: &mut redis::Connection) {
    // a worker for sv_loadthebags()
    // println!("sv_parallelbagloader worker {} has {} bags", id.to_string(), bags.len());
    for b in bags {
        rs_sadd(key.as_str(), b.as_str(), c);
    }
}

fn sv_loadthebags(key: String, mut bags: Vec<String>, workers: i32, rca: &str) {
    // load the bags of words into redis; parallelize this
    let uworkers = usize::try_from(workers).unwrap();
    let totalwork = bags.len();
    let chunksize = totalwork / uworkers;

    let mut bagmap: HashMap<i32, Vec<String>> = HashMap::new();

    if totalwork <= uworkers {
        bagmap.insert(0, bags.drain(totalwork..).collect());
    } else {
        for i in 0..workers {
            bagmap.insert(i, bags.drain(chunksize * usize::try_from(workers - i).unwrap()..).collect());
        }
    }

    let handles: Vec<thread::JoinHandle<_>> = (0..workers)
        .map(|w| {
            let thisbag = &bagmap[&w].clone();
            let thisbag = thisbag.to_vec();
            let mut rc = redisconnect(rca.to_string());
            let k = key.clone();
            thread::spawn( move || {
                sv_parallelbagloader(Uuid::new_v4(), k, thisbag, &mut rc);
            })
        })
        .collect::<Vec<thread::JoinHandle<_>>>();

    for thread in handles {
        thread.join().unwrap();
    }

    // leave no bag behind...: bags might not be fully drained
    // a max of workers-1 bags could still be here

    let mut rc = redisconnect(rca.to_string());
    sv_parallelbagloader(Uuid::new_v4(), key.clone(), bags, &mut rc);

}

fn sv_parallelmorphology() {
    // https://stackoverflow.com/questions/57649032/returning-a-value-from-a-function-that-spawns-threads
    // TODO...
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

            // note that in [3] you need to take the second half after the comma: "bellus" and not "bellī, bellus"
            let mut ee: Vec<&str> = e.split(",").collect();
            let e = ee.pop().unwrap_or("").trim();

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

// fn sv_updatesetofpossibilities(rpo: String, re: Regex) -> HashMap<String, bool> {
//     // a new collection of possibilities has arrived <p1>xxx</p1><p2>yyy</p2>...
//     // parse this string for a list of possibilities; then add its elements to the set of known possibilities
//     // return the updated set
//     let mut morph: HashMap<String, bool> = HashMap::new();
//     for f in re.find_iter(rpo.as_str()) {
//         morph.insert(String::from(f.as_str()), true);
//     }
//     morph
// }

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

fn str_lcs(s: &str) -> String {
    // if we are not using ascii strings...
    format!("{}{}", s.chars().next().unwrap().to_lowercase(),
            s.chars().skip(1).collect::<String>())
}

fn lfl(message: String, loglevel: i32, threshold: i32) {
    // log if logging
    if loglevel >= threshold {
        println!("[{}] {}", SHORTNAME, message);
    }
}