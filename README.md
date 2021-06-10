# HipparchiaRustDBHelper

mostly a way to learn more about rust, I suppose.

this helper is not going to be finished?

`HipparchiaRustDBHelper` can do `websockets` and `grabbing`, but work on the `vectors` called a halt to the procedings: this is **significantly** slower than `HipparchiaGoDBHelper`.

It is not at all clear that craftier algorithms are going to make up the difference here. Go is just plain better if you are dealing with lots of strings?

```
RUST vs GO: Golang slaughters on this one...

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


the slowdown is effectively postgres + regex:
    "found 54464 morphology objects" should be just a single tt + query + get/load
    "Built morphmap" is sending regexp into a loop 54464 times....
    
    
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