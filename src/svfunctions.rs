//    HipparchiaRustDBHelper: search and vector helper app and functions for HipparchiaServer
//    Copyright: E Gunderson 2021
//    License: GNU GENERAL PUBLIC LICENSE 3
//        (see LICENSE in the top level directory of the distribution)

use json::JsonValue;
use redis::RedisResult;
use regex::Regex;
use std::collections::HashMap;

use crate::thestructs::*;
use crate::dbfunctions::*;

pub fn sv_stripper(text: &str, topurge: Vec<Regex>) -> String {
    // https://github.com/rust-lang/regex/blob/master/examples/shootout-regex-dna-replace.rs
    // avoid compliling regex in a loop: it is a killer...
    let mut newtext = String::from(text);
    for r in topurge {
        newtext = r.replace_all(&newtext, "").into_owned();
    }
    newtext
}

pub fn sv_swapper(text: &str) -> String {
    let mut swapper: HashMap<&str, &str> = HashMap::new();
    swapper.insert("A.", "Aulus");
    swapper.insert("App.", "Appius");
    swapper.insert("C.", "Caius");
    swapper.insert("G.", "Gaius");
    swapper.insert("Cn.", "Cnaius");
    swapper.insert("D.", "Decimus");
    swapper.insert("L.", "Lucius");
    swapper.insert("M.", "Marcus");
    swapper.insert("M.’", "Manius");
    swapper.insert("N.", "Numerius");
    swapper.insert("P.", "Publius");
    swapper.insert("Q.", "Quintus");
    swapper.insert("S.", "Spurius");
    swapper.insert("Sp.", "Spurius",);
    swapper.insert("Ser.", "Servius");
    swapper.insert("Sex.", "Sextus");
    swapper.insert("T.", "Titus");
    swapper.insert("Ti", "Tiberius");
    swapper.insert("V.", "Vibius");
    swapper.insert("a.", "ante");
    swapper.insert("d.", "dies");
    swapper.insert("Id.", "Idibus");
    swapper.insert("Kal.", "Kalendas");
    swapper.insert("Non.", "Nonas");
    swapper.insert("prid.", "pridie");
    swapper.insert("Ian.", "Ianuarias");
    swapper.insert("Feb.", "Februarias");
    swapper.insert("Mart.", "Martias");
    swapper.insert("Apr.", "Aprilis");
    swapper.insert("Mai.", "Maias");
    swapper.insert("Iun.", "Iunias");
    swapper.insert("Quint.", "Quintilis");
    swapper.insert("Sext.", "Sextilis");
    swapper.insert("Sept.", "Septembris");
    swapper.insert("Oct.", "Octobris");
    swapper.insert("Nov.", "Novembris");
    swapper.insert("Dec.", "Decembris");

    let words: Vec<&str> = text.split_ascii_whitespace().collect();
    let newwords: Vec<&str> = words.iter()
        .map(|w| if swapper.contains_key(w) { swapper[w]} else { w })
        .collect();
    let newtext = newwords.join(" ");
    newtext
}

pub fn sv_acuteforgrave(text: String) -> String {

    // https://stackoverflow.com/questions/34606043/how-do-i-replace-specific-characters-idiomatically-in-rust
    // let s:String = "Hello, world!".chars()
    //     .map(|x| match x {
    //         '!' => '?',
    //         'A'..='Z' => 'X',
    //         'a'..='z' => 'x',
    //         _ => x
    //     }).collect();
    // println!("{}", s);// Xxxxx, xxxxx?

    let remapped: String = text.chars()
        .map(|c| match c {
            'ὰ' => 'ά',
            'ὲ' => 'έ',
            'ὶ' => 'ί',
            'ὸ' => 'ό',
            'ὺ' => 'ύ',
            'ὴ' => 'ή',
            'ὼ' => 'ώ',
            'ἂ' => 'ἄ',
            'ἒ' => 'ἔ',
            'ἲ' => 'ἴ',
            'ὂ' => 'ὄ',
            'ὒ' => 'ὔ',
            'ἢ' => 'ἤ',
            'ὢ' => 'ὤ',
            'ᾃ' => 'ᾅ',
            'ᾓ' => 'ᾕ',
            'ᾣ' => 'ᾥ',
            'ᾂ' => 'ᾄ',
            'ᾒ' => 'ᾔ',
            'ᾢ' => 'ᾤ',
            _ => c
        }).collect();
    remapped
}

pub fn sv_buildsentences(splittext: Vec<&str>, bagsize: i32) -> HashMap<String, String> {
    // if HashMap<&str, &str> compile error: returns a value referencing data owned by the current function
    // see: https://stackoverflow.com/questions/32682876/is-there-any-way-to-return-a-reference-to-a-variable-created-in-a-function
    // "Instead of trying to return a reference, return an owned object. String instead of &str, Vec<T> instead of &[T], T instead of &T, etc."]

    let tagger: Regex = Regex::new("⊏.*?⊐").unwrap();
    let notachar: Regex = Regex::new("[^ a-zα-ωϲϹἀἁἂἃἄἅἆἇᾀᾁᾂᾃᾄᾅᾆᾇᾲᾳᾴᾶᾷᾰᾱὰάἐἑἒἓἔἕὲέἰἱἲἳἴἵἶἷὶίῐῑῒΐῖῗὀὁὂὃὄὅόὸὐὑὒὓὔὕὖὗϋῠῡῢΰῦῧύὺᾐᾑᾒᾓᾔᾕᾖᾗῂῃῄῆῇἤἢἥἣὴήἠἡἦἧὠὡὢὣὤὥὦὧᾠᾡᾢᾣᾤᾥᾦᾧῲῳῴῶῷώὼ]").unwrap();
    let locc: Regex = Regex::new("⊏(.*?)⊐").unwrap();

    let mut sentenceswithlocus: HashMap<String, String> = HashMap::new();

    let mut splittext: Vec<&str> = splittext.into_iter().rev().collect();

    while  splittext.len() > 0 {
        let mut parcel = String::new();
        for _ in 0..bagsize {
            parcel.push_str(splittext.pop().unwrap_or(""));
        }
        let lcs = parcel.to_lowercase();
        let firsthit: String = match locc.captures(&lcs.as_str()) {
            None => "".to_string(),
            Some(x) => x[1].to_string(),
        };
        let thesentence = tagger.replace_all(&lcs, "").into_owned();
        let thesentence = notachar.replace_all(&thesentence, "").into_owned();

        // println!("{}: {}", firsthit, thesentence);
        sentenceswithlocus.insert(firsthit, thesentence);

    }
    sentenceswithlocus
}

pub fn sv_buildflatbags(sentenceswithlocus: HashMap<String, String>, mm: HashMap<String, Vec<String>>) -> HashMap<String, String> {
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

    let mut bagged: HashMap<String, String> = HashMap::new();
    for s in sentenceswithlocus.keys() {
        bagged.insert(s.clone(), swapper(&sentenceswithlocus[s]));
    }

    // for b in &bagged {
    //     println!("{}", b);
    // }

    bagged
}

pub fn sv_buildcompositebags(sentenceswithlocus: HashMap<String, String>, mm: HashMap<String, Vec<String>>) -> HashMap<String, String> {
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

    let mut bagged: HashMap<String, String> = HashMap::new();
    for s in sentenceswithlocus.keys() {
        bagged.insert(s.clone(), swapper(&sentenceswithlocus[s]));
    }

    // for b in &bagged {
    //     println!("{}", b);
    // }

    bagged
}

pub fn sv_buildwinnertakesallbags(sentenceswithlocus: HashMap<String, String>, parsemap: HashMap<String, Vec<String>>, pg: &mut postgres::Client) -> HashMap<String, String> {
    // turn a list of sentences into a list of list of headwords; here we figure out which headword is the dominant homonym
    // then we just use that term; "esse" always comes from "sum" and never "edo", etc.

    // [a] figure out all headwords in use

    let mut allheadwords: HashMap<String, bool> = HashMap::new();
    for m in parsemap.keys() {
        for p in &parsemap[m] {
            allheadwords.insert(p.to_string(), true);
        }
    }

    // [b] generate scoremap and assign scores to each of the headwords
    let wds: Vec<String> = allheadwords.keys().map(|k| k.clone()).collect();
    let scoremap: HashMap<String, i32> = db_fetchheadwordcounts(wds, pg);

    // for s in scoremap.keys() {
    //     println!("{} {}", &s, &scoremap[s]);
    // }

    // [c] note that there are capital words in the parsemap that need lowering
    // lower the keys and the values at the same time

    let mut lcparsemap: HashMap<String, Vec<String>> = HashMap::new();
    for (key, value) in &parsemap {
        lcparsemap.insert(str_lcs(key), value.clone().iter().map(|v| str_lcs(v)).collect());
    }

    let mut lcscoremap: HashMap<String, i32> = HashMap::new();
    for (key, value) in &scoremap {
        lcscoremap.insert(str_lcs(key), *value);
    }

    // reset our names
    let parsemap = lcparsemap;
    let scoremap = lcscoremap;

    // [d] run through the parser map and kill off the losers

    // this part is broken: the newparsemap contains only headword:headword pairs;
    //  dissero² dissero²
    //  gratiosus gratiosus

    let mut newparsemap: HashMap<String, String> = HashMap::new();
    for (headword, possibilities) in &parsemap {
        // let mut poss: HashMap<i32, String> = HashMap::new();
        let highscore = 0;
        for p in possibilities {
            if scoremap.contains_key(&*headword.clone()) {
                let thisscore: i32 = scoremap[&*headword.clone()];
                if thisscore >= highscore {
                    newparsemap.insert(headword.clone(), p.to_string());
                }
            } else {
                newparsemap.insert(headword.clone(), p.to_string());
            }
        }
    }

    // for s in newparsemap.keys() {
    //     println!("{} {}", &s, &newparsemap[s]);
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

    let mut bagged: HashMap<String, String> = HashMap::new();
    for s in sentenceswithlocus.keys() {
        bagged.insert(s.clone(), swapper(&sentenceswithlocus[s]));
    }

    // for b in &bagged {
    //     println!("{}", b);
    // }

    bagged
}

pub fn sv_findallwords(sentences: Vec<&str>) -> Vec<&str> {
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

pub fn sv_getrequiredmorphobjects(words: Vec<&str>, pg: &mut postgres::Client) -> Vec<DbMorphology> {
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

    let mut morph: Vec<DbMorphology> = db_sv_get_morphobjects(&mut latinwords, "latin", pg);
    let mut grmorph: Vec<DbMorphology> = db_sv_get_morphobjects(&mut greekwords, "greek", pg);

    morph.append(&mut grmorph);
    morph
}

pub fn sv_dropstopwords(todrop: &str, bags: HashMap<String, String>) -> HashMap<String, String> {
    // purge stopwords from the bags
    let vv: Vec<&str> = todrop.split_whitespace().collect();
    let mut stopmap: HashMap<&str, bool> = HashMap::new();
    for v in vv { stopmap.insert(v, true); }

    let mut cleaned: HashMap<String, String> = HashMap::new();
    for b in bags.keys() {
        let ww: Vec<&str> = bags[b].split_whitespace().collect();
        let mut ns: Vec<&str> = Vec::new();
        for w in ww {
            if stopmap.contains_key(w) {
                continue;
            } else {
                ns.push(w);
            }
        }
        cleaned.insert(b.clone(), ns.join(" "));
    }
    cleaned
}

pub fn sv_loadthebags(key: String, bags: HashMap<String, String>, rca: &str) {
    // load the bags of words into redis
    // on the python end: hits = {j['Loc']: j['Bag'] for j in js}
    let mut c = redisconnect(rca.to_string());

    let mut pipe = redis::pipe();
    for b in bags.keys() {
        let mut data = JsonValue::new_object();
        data["Loc"] = b.clone().into();
        data["Bag"] = bags[b].clone().into();
        // print!["{}", data.dump()];
        pipe.cmd("SADD").arg(key.as_str()).arg(data.dump()).ignore();
    }
    let _: RedisResult<()> = pipe.query(&mut c);
}

pub fn _sv_parallelmorphology() {
    // https://stackoverflow.com/questions/57649032/returning-a-value-from-a-function-that-spawns-threads
    // TODO...
}