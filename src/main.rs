//    HipparchiaRustDBHelper: search and vector helper app and functions for HipparchiaServer
//    Copyright: E Gunderson 21
//    License: GNU GENERAL PUBLIC LICENSE 3
//        (see LICENSE in the top level directory of the distribution)

mod thestructs;
mod dbfunctions;
mod svfunctions;

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
use redis::{Commands, Connection};
use regex::Regex;
use tungstenite::{accept_hdr, handshake::server::{Request, Response}, Message, WebSocket};
use uuid::Uuid;

use crate::thestructs::*;
use crate::dbfunctions::*;
use crate::svfunctions::*;

static MYNAME: &str = "Hipparchia Rust Helper";
static SHORTNAME: &str = "HRH";
static VERSION: &str = "0.1.1";
static POLLINGINTERVAL: time::Duration = time::Duration::from_millis(400);
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

fn vector_prep(thekey: &str, b: &str, workers: i32, db: &str, s: i32, e: i32, ll: i32, psq: &str, rca: &str) -> String {
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
        _ => db_redisfectch(&thekey, psq, rca, ),
    };

    let duration = start.elapsed();
    let m = format!("{} dblines fetched [A: {}]", dblines.len(), format_duration(duration).to_string());
    lfl(m, ll, 2);

    // [b] turn them into a unified text block
    // yes, but what is the fastest way...? cf. the huge golang speedup via strings.Builder
    // https://maxuuell.com/blog/how-to-concatenate-strings-in-rust
    // https://stackoverflow.com/questions/30154541/how-do-i-concatenate-strings
    // we have a vector; we want an array so we can try Array.concat()
    // https://stackoverflow.com/questions/29570607/is-there-a-good-way-to-convert-a-vect-to-an-array
    // but is it really possible to generate an array? "arrays cannot have values added or removed at runtime"

    let txtlines: Vec<String> = dblines.iter()
        .map(|x| format!{"⊏line/{}/{}⊐{}", x.uid, x.idx, x.mu})
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

    let re = Regex::new("v").unwrap();
    let fulltext = re.replace_all(&*fulltext, "u");
    let re = Regex::new("j").unwrap();
    let fulltext = re.replace_all(&*fulltext, "i");
    let re = Regex::new("[σς]").unwrap();
    let fulltext = re.replace_all(&*fulltext, "ϲ");
    let fulltext = sv_swapper(&fulltext);
    // println!("{}", fulltext);

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

    // alternate splitter: no real speed difference

    // let re = Regex::new("[?!;·]").unwrap();
    // let fulltext = re.replace_all(&*fulltext, ".");
    // let splittext: Vec<&str> = fulltext.split(".").collect();

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

    let  mo: Vec<DbMorphology> = sv_getrequiredmorphobjects(allwords.clone(), &mut pg);

    // note that you will have more in [f] than in [g]; but the golang version is a map and
    // so there len(f) = len(g); nevertheless len(e) is supposed to match len(g) in both cases
    // since we refuse to drop any words

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

    // retain unparsed terms
    for w in allwords {
        if morphmap.contains_key(w) {
            ()
        } else {
            morphmap.insert(w.to_string(), vec![w.to_string()]);
        }
    }

    // note that capitalization issues mean that your morphmap can be longer than the total number of words

    let duration = start.elapsed();
    let m = format!("Built morphmap for {} items [G: {}]", morphmap.len(), format_duration(duration).to_string());
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

    // for b in &bagged {
    //     if b.len() > 0 {
    //         println!("[bag] {}", &b);
    //     }
    // }

    let duration = start.elapsed();
    let m = format!("Built {} bags [H: {}]", bagged.len(), format_duration(duration).to_string());
    lfl(m, ll, 2);

    // [i] purge stopwords

    let skipheadwords = "unus verum omne sum¹ ab δύο πρότεροϲ ἄνθρωποϲ τίϲ δέω¹ ὅϲτιϲ homo πᾶϲ οὖν εἶπον ἠμί ἄν² tantus μένω μέγαϲ οὐ verus neque eo¹ nam μέν ἡμόϲ aut Sue διό reor ut ἐγώ is πωϲ ἐκάϲ enim ὅτι² παρά ἐν Ἔχιϲ sed ἐμόϲ οὐδόϲ ad de ita πηρόϲ οὗτοϲ an ἐπεί a γάρ αὐτοῦ ἐκεῖνοϲ ἀνά ἑαυτοῦ quam αὐτόϲε et ὑπό quidem Alius¹ οἷοϲ noster γίγνομαι ἄνα προϲάμβ ἄν¹ οὕτωϲ pro² tamen ἐάν atque τε qui² si multus idem οὐδέ ἐκ omnes γε causa δεῖ πολύϲ in ἔδω ὅτι¹ μή Ios ἕτεροϲ cum meus ὅλοξ suus omnis ὡϲ sua μετά Ἀλλά ne¹ jam εἰϲ ἤ² ἄναξ ἕ ὅϲοϲ dies ipse ὁ hic οὐδείϲ suo ἔτι ἄνω¹ ὅϲ νῦν ὁμοῖοϲ edo¹ εἰ qui¹ πάλιν ὥϲπερ ne³ ἵνα τιϲ διά φύω per τοιοῦτοϲ for eo² huc locum neo¹ sui non ἤ¹ χάω ex κατά δή ἁμόϲ dico² ὅμοιοϲ αὐτόϲ etiam vaco πρόϲ Ζεύϲ ϲύ quis¹ tuus b εἷϲ Eos οὔτε τῇ καθά ego tu ille pro¹ ἀπό suum εἰμί ἄλλοϲ δέ alius² pars vel ὥϲτε χέω res ἡμέρα quo δέομαι modus ὑπέρ ϲόϲ ito τῷ περί Τήιοϲ ἕκαϲτοϲ autem καί ἐπί nos θεάω γάρον γάροϲ Cos²";
    let skipinflected = "ita a inquit ego die nunc nos quid πάντων ἤ με θεόν δεῖ for igitur ϲύν b uers p ϲου τῷ εἰϲ ergo ἐπ ὥϲτε sua me πρό sic aut nisi rem πάλιν ἡμῶν φηϲί παρά ἔϲτι αὐτῆϲ τότε eos αὐτούϲ λέγει cum τόν quidem ἐϲτιν posse αὐτόϲ post αὐτῶν libro m hanc οὐδέ fr πρῶτον μέν res ἐϲτι αὐτῷ οὐχ non ἐϲτί modo αὐτοῦ sine ad uero fuit τοῦ ἀπό ea ὅτι parte ἔχει οὔτε ὅταν αὐτήν esse sub τοῦτο i omnes break μή ἤδη ϲοι sibi at mihi τήν in de τούτου ab omnia ὃ ἦν γάρ οὐδέν quam per α autem eius item ὡϲ sint length οὗ λόγον eum ἀντί ex uel ἐπειδή re ei quo ἐξ δραχμαί αὐτό ἄρα ἔτουϲ ἀλλ οὐκ τά ὑπέρ τάϲ μάλιϲτα etiam haec nihil οὕτω siue nobis si itaque uac erat uestig εἶπεν ἔϲτιν tantum tam nec unde qua hoc quis iii ὥϲπερ semper εἶναι e ½ is quem τῆϲ ἐγώ καθ his θεοῦ tibi ubi pro ἄν πολλά τῇ πρόϲ l ἔϲται οὕτωϲ τό ἐφ ἡμῖν οἷϲ inter idem illa n se εἰ μόνον ac ἵνα ipse erit μετά μοι δι γε enim ille an sunt esset γίνεται omnibus ne ἐπί τούτοιϲ ὁμοίωϲ παρ causa neque cr ἐάν quos ταῦτα h ante ἐϲτίν ἣν αὐτόν eo ὧν ἐπεί οἷον sed ἀλλά ii ἡ t te ταῖϲ est sit cuius καί quasi ἀεί o τούτων ἐϲ quae τούϲ minus quia tamen iam d διά primum r τιϲ νῦν illud u apud c ἐκ δ quod f quoque tr τί ipsa rei hic οἱ illi et πῶϲ φηϲίν τοίνυν s magis unknown οὖν dum text μᾶλλον λόγοϲ habet τοῖϲ qui αὐτοῖϲ suo πάντα uacat τίϲ pace ἔχειν οὐ κατά contra δύο ἔτι αἱ uet οὗτοϲ deinde id ut ὑπό τι lin ἄλλων τε tu ὁ cf δή potest ἐν eam tum μου nam θεόϲ κατ ὦ cui nomine περί atque δέ quibus ἡμᾶϲ τῶν eorum";

    let bagged: Vec<String> = sv_dropstopwords(skipheadwords, bagged);
    let bagged: Vec<String> = sv_dropstopwords(skipinflected, bagged);

    // no empty bags...
    let mut bags: Vec<String> = Vec::new();
    for b in bagged {
        if b.len() > 0 {
            // println!("[bag] {}", &b);
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

    resultkey
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

// https://stackoverflow.com/questions/38406793/why-is-capitalizing-the-first-letter-of-a-string-so-convoluted-in-rust/53571882#53571882
fn make_ascii_title_case(s: &mut str) {
    if let Some(r) = s.get_mut(0..1) {
        r.make_ascii_uppercase();
    }
}

fn lfl(message: String, loglevel: i32, threshold: i32) {
    // log if logging
    if loglevel >= threshold {
        println!("[{}] {}", SHORTNAME, message);
    }
}