//    HipparchiaRustDBHelper: search and vector helper app and functions for HipparchiaServer
//    Copyright: E Gunderson 21
//    License: GNU GENERAL PUBLIC LICENSE 3
//

use std::collections::HashMap;
use std::time::Instant;

use humantime::format_duration;
use regex::Regex;

use crate::dbfunctions::*;
use crate::helpers::*;
use crate::svfunctions::*;
use crate::thestructs::*;

static SKIPHEADWORDS: &str = "unus verum omne sum¹ ab δύο πρότεροϲ ἄνθρωποϲ τίϲ δέω¹ ὅϲτιϲ homo πᾶϲ οὖν εἶπον ἠμί ἄν² tantus μένω μέγαϲ οὐ verus neque eo¹ nam μέν ἡμόϲ aut Sue διό reor ut ἐγώ is πωϲ ἐκάϲ enim ὅτι² παρά ἐν Ἔχιϲ sed ἐμόϲ οὐδόϲ ad de ita πηρόϲ οὗτοϲ an ἐπεί a γάρ αὐτοῦ ἐκεῖνοϲ ἀνά ἑαυτοῦ quam αὐτόϲε et ὑπό quidem Alius¹ οἷοϲ noster γίγνομαι ἄνα προϲάμβ ἄν¹ οὕτωϲ pro² tamen ἐάν atque τε qui² si multus idem οὐδέ ἐκ omnes γε causa δεῖ πολύϲ in ἔδω ὅτι¹ μή Ios ἕτεροϲ cum meus ὅλοξ suus omnis ὡϲ sua μετά Ἀλλά ne¹ jam εἰϲ ἤ² ἄναξ ἕ ὅϲοϲ dies ipse ὁ hic οὐδείϲ suo ἔτι ἄνω¹ ὅϲ νῦν ὁμοῖοϲ edo¹ εἰ qui¹ πάλιν ὥϲπερ ne³ ἵνα τιϲ διά φύω per τοιοῦτοϲ for eo² huc locum neo¹ sui non ἤ¹ χάω ex κατά δή ἁμόϲ dico² ὅμοιοϲ αὐτόϲ etiam vaco πρόϲ Ζεύϲ ϲύ quis¹ tuus b εἷϲ Eos οὔτε τῇ καθά ego tu ille pro¹ ἀπό suum εἰμί ἄλλοϲ δέ alius² pars vel ὥϲτε χέω res ἡμέρα quo δέομαι modus ὑπέρ ϲόϲ ito τῷ περί Τήιοϲ ἕκαϲτοϲ autem καί ἐπί nos θεάω γάρον γάροϲ Cos²";
static SKIPINFLECTED: &str = "ita a inquit ego die nunc nos quid πάντων ἤ με θεόν δεῖ for igitur ϲύν b uers p ϲου τῷ εἰϲ ergo ἐπ ὥϲτε sua me πρό sic aut nisi rem πάλιν ἡμῶν φηϲί παρά ἔϲτι αὐτῆϲ τότε eos αὐτούϲ λέγει cum τόν quidem ἐϲτιν posse αὐτόϲ post αὐτῶν libro m hanc οὐδέ fr πρῶτον μέν res ἐϲτι αὐτῷ οὐχ non ἐϲτί modo αὐτοῦ sine ad uero fuit τοῦ ἀπό ea ὅτι parte ἔχει οὔτε ὅταν αὐτήν esse sub τοῦτο i omnes break μή ἤδη ϲοι sibi at mihi τήν in de τούτου ab omnia ὃ ἦν γάρ οὐδέν quam per α autem eius item ὡϲ sint length οὗ λόγον eum ἀντί ex uel ἐπειδή re ei quo ἐξ δραχμαί αὐτό ἄρα ἔτουϲ ἀλλ οὐκ τά ὑπέρ τάϲ μάλιϲτα etiam haec nihil οὕτω siue nobis si itaque uac erat uestig εἶπεν ἔϲτιν tantum tam nec unde qua hoc quis iii ὥϲπερ semper εἶναι e ½ is quem τῆϲ ἐγώ καθ his θεοῦ tibi ubi pro ἄν πολλά τῇ πρόϲ l ἔϲται οὕτωϲ τό ἐφ ἡμῖν οἷϲ inter idem illa n se εἰ μόνον ac ἵνα ipse erit μετά μοι δι γε enim ille an sunt esset γίνεται omnibus ne ἐπί τούτοιϲ ὁμοίωϲ παρ causa neque cr ἐάν quos ταῦτα h ante ἐϲτίν ἣν αὐτόν eo ὧν ἐπεί οἷον sed ἀλλά ii ἡ t te ταῖϲ est sit cuius καί quasi ἀεί o τούτων ἐϲ quae τούϲ minus quia tamen iam d διά primum r τιϲ νῦν illud u apud c ἐκ δ quod f quoque tr τί ipsa rei hic οἱ illi et πῶϲ φηϲίν τοίνυν s magis unknown οὖν dum text μᾶλλον λόγοϲ habet τοῖϲ qui αὐτοῖϲ suo πάντα uacat τίϲ pace ἔχειν οὐ κατά contra δύο ἔτι αἱ uet οὗτοϲ deinde id ut ὑπό τι lin ἄλλων τε tu ὁ cf δή potest ἐν eam tum μου nam θεόϲ κατ ὦ cui nomine περί atque δέ quibus ἡμᾶϲ τῶν eorum";
static TERMINATIONS: &str = ".?!;·";

pub fn vector_prep(thekey: &str, b: &str, workers: i32, bagsize: i32, db: &str, s: i32, e: i32, ll: i32, psq: &str, rca: &str) -> String {
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

    let terminations: Vec<char> = TERMINATIONS.chars().collect();
    let splittext: Vec<&str> = fulltext.split(&terminations[..]).collect();

    // alternate splitter: no real speed difference

    // let re = Regex::new("[?!;·]").unwrap();
    // let fulltext = re.replace_all(&*fulltext, ".");
    // let splittext: Vec<&str> = fulltext.split(".").collect();

    let sentenceswithlocus: HashMap<String, String> = sv_buildsentences(splittext, bagsize);

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

    let bagged: Vec<String>  = match b {
        "flat" => sv_buildflatbags(sentences.to_owned(), morphmap),
        "alternates" => sv_buildcompositebags(sentences.to_owned(), morphmap),
        "winnertakesall" => sv_buildwinnertakesallbags(sentences.to_owned(), morphmap, &mut pg),
        // should never hit this but...
        _ => sentences.iter().map(|s| s.to_string()).collect(),
    };

    // for b in &bagged {
    //     if b.len() > 0 {
    //         println!("[bag] {}", &b);
    //     }
    // }

    let duration = start.elapsed();
    let m = format!("Built {} bags [H: {}]", bagged.len(), format_duration(duration).to_string());
    lfl(m, ll, 2);

    // [i] purge stopwords

    let bagged: Vec<String> = sv_dropstopwords(SKIPHEADWORDS, bagged);
    let bagged: Vec<String> = sv_dropstopwords(SKIPINFLECTED, bagged);

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
