# HipparchiaRustDBHelper

mostly a way to learn more about rust, I suppose.

this helper is not going to be polished up and/or a high priority

`HipparchiaRustDBHelper` can do `websockets` and `grabbing`, but work on the `vectors` 
exposed some serious issues with using `rust` + `regex`: this is **significantly** slower 
than `HipparchiaGoDBHelper` unless/until you can save it from doing lots of `regex`.

A refactored `HipparchiaBuilder` that pre-parses morphology possibilites spares us the trip to `regex` 
and capture groups. This sped up both `HipparchiaGoDBHelper` and `HipparchiaRustDBHelper` as well as 
the pure `python` version. Nevertheless, it looks like `rust` is not going to be a priority: in general this
project requires tons of `regex`, and if `golang` is going to crush `rust`, then there is no point
in suffering through all of the things one has to do to accommodate it.

### warning

if you do not use `HipparchiaBuilder 1.5.0b+` you will see the following panic because your DB columns are wrong...

```thread 'main' panicked at 'called `Result::unwrap()` on an `Err` value: Error { kind: Db, cause: Some(DbError { severity: "ERROR", parsed_severity: Some(Error), code: SqlState(E42703), message: "column \"related_headwords\" does not exist", detail: None, hint: None, position: Some(Original(42)), where_: None, schema: None, table: None, column: None, datatype: None, constraint: None, file: Some("parse_relation.c"), line: Some(3504), routine: Some("errorMissingColumn") }) }', src/main.rs:655:42```


### speed notes

```
RUST vs GO: Golang slaughters on this one... [pre-Builder 1.5.0b]

1452 •erik@big-sur-box• rust/HipparchiaRustDBHelper/ % ./target/release/hipparchia_rust_dbhelper  --sv --l 3 --p ${L} --svdb lt0474 --svs 4 --sve 100000
  Hipparchia Rust Helper CLI Debugging Interface (v.0.0.4)
  [HRH] requested the vector_prep() branch of the code
  [HRH] Seeking to build winnertakesall bags of words
  [HRH] No redis key; gathering lines with a direct CLI PostgreSQL query)
  [HRH] dblines fetched [A: 351ms 497us 55ns]
  [HRH] unified text block built [B: 392ms 610us 712ns]
  [HRH] preliminary cleanups complete [C: 416ms 774us 487ns]
  [HRH] found 44447 sentences [D: 643ms 367us 938ns]
  [HRH] found 64971 words [E: 721ms 293us 431ns]
  [HRH] found 54464 morphology objects [F: 6s 594ms 658us 949ns]
  [HRH] Built morphmap [G: 29s 157ms 980us 115ns]
  [HRH] Built 44447 bags [H: 30s 999ms 807us 997ns]
  [HRH] Purged stopwords in 35208 bags [I: 31s 59ms 901us 493ns]
  [HRH] Stored 35208 bags [J: 31s 998ms 776us 682ns]


the slowdown is regex in both cases where the gaps are obvious
    
date && ./HipparchiaGoDBHelper -sv -l 5 -svdb lt0474 -svs 4 -sve 100000 -k "" -p ${L} && date
Thu Jun 10 17:28:26 EDT 2021
[HGH] Hipparchia Golang Helper CLI Debugging Interface (v.1.1.2) [loglevel=5]
[HGH] Bagger Module Launched
[HGH] Seeking to build *winnertakesall* bags of words
[HGH] Connected to redis
[HGH] Connected to hipparchiaDB on PostgreSQL
[HGH] No redis key; gathering lines with a direct CLI PostgreSQL query
[HGH] 99997 lines acquired [A: 0.160930s])
[HGH] Unified text block built [B: 0.216524s])
[HGH] Preliminary cleanups complete [C: 0.319855s]
[HGH] Found 45665 sentences [D: 1.020172s]
[HGH] Found 65552 distinct words [E: 1.066192s]
[HGH] Got morphology for 61124 terms [F: 1.765334s]
[HGH] Built morphmap for 61124 terms [G: 2.451704s]
[HGH] Finished bagging 45665 bags [H: 2.752900s]
[HGH] Cleared stopwords [I: 2.858481s]
[HGH] Finished loading [J: 4.034843s]
bags have been stored at _vectorresults
Thu Jun 10 17:28:30 EDT 2021

```

```
RUST vs GO: parity... [post-Builder 1.5.0b]


% ./target/release/hipparchia_rust_dbhelper  --sv --l 3 --p ${L} --svdb lt0474 --svs 4 --sve 140000
Hipparchia Rust Helper CLI Debugging Interface (v.0.0.6)
[HRH] requested the vector_prep() branch of the code
[HRH] Seeking to build winnertakesall bags of words
[HRH] No redis key; gathering lines with a direct CLI PostgreSQL query)
[HRH] dblines fetched [A: 398ms 452us 178ns]
[HRH] unified text block built [B: 455ms 518us 146ns]
[HRH] preliminary cleanups complete [C: 491ms 184us 155ns]
[HRH] found 66511 sentences [D: 723ms 177us 397ns]
[HRH] found 78817 words [E: 837ms 2us 591ns]
[HRH] found 67835 morphology objects [F: 1s 656ms 442us 806ns]
[HRH] Built morphmap [G: 1s 685ms 663us 782ns]
[HRH] Built 66511 bags [H: 2s 175ms 255us 17ns]
[HRH] Purged stopwords in 51747 bags [I: 2s 259ms 803us 672ns]
[HRH] Stored 51747 bags [J: 3s 581ms 419us 158ns]
rusttest_vectorresults


% ./HipparchiaGoDBHelper -sv -l 3 -svdb lt0474 -svs 4 -sve 140000
[HGH] Hipparchia Golang Helper CLI Debugging Interface (v.1.2.0) [loglevel=3]
[HGH] Bagger Module Launched
[HGH] Seeking to build *winnertakesall* bags of words
[HGH] Connected to redis
[HGH] Connected to hipparchiaDB on PostgreSQL
[HGH] No redis key; gathering lines with a direct CLI PostgreSQL query
[HGH] [A: 0.194s][Δ: 0.194s] 139997 lines acquired
[HGH] [B: 0.288s][Δ: 0.094s] Unified text block built
[HGH] [C: 0.441s][Δ: 0.153s] Preliminary cleanups complete
[HGH] [D: 1.397s][Δ: 0.956s] Found 68790 sentences
[HGH] [E: 1.455s][Δ: 0.058s] Found 80125 distinct words
[HGH] [F: 1.822s][Δ: 0.367s] Got morphology for 73819 terms
[HGH] [G: 1.837s][Δ: 0.014s] Built morphmap for 73819 terms
[HGH] [H: 2.153s][Δ: 0.316s] Finished bagging 68790 bags
[HGH] [I: 2.290s][Δ: 0.138s] Cleared stopwords: 66661 bags remain
[HGH] [J: 3.696s][Δ: 1.406s] Finished loading
bags have been stored at _vectorresults%

```