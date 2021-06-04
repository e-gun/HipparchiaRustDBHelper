# HipparchiaRustDBHelper

mostly a way to learn more about rust, I suppose.

this helper is not going to be finished?

`HipparchiaRustDBHelper` can do `websockets` and `grabbing`, but work on the `vectors` called a halt to the procedings: this is **significantly** slower than `HipparchiaGoDBHelper`.

It is not at all clear that craftier algorithms are going to make up the difference here. Go is just plain better if you are dealing with lots of strings?

```
RUST vs GO: Golang slaughters on this one...

1452 •erik@big-sur-box• rust/HipparchiaRustDBHelper/ % ./target/debug/hipparchia_rust_dbhelper --sv --l 3 --p ${L} --svdb lt0474 --svs 4 --sve 100000
Hipparchia Rust Helper CLI Debugging Interface (v.0.0.4)
[HRH] requested the vector_prep() branch of the code
[HRH] Seeking to build winnertakesall bags of words
[HRH] unified text block built [B: 2s 87ms 658us 860ns]
[HRH] preliminary cleanups complete [C: 2s 833ms 810us 80ns]
[HRH] found 44447 sentences [D: 7s 705ms 855us 584ns]
[HRH] found 67447 words [E: 8s 767ms 826us 901ns]
l: 67220; g: 227

1217 •erik@big-sur-box• e-gun/HipparchiaGoDBHelper/ % date && ./HipparchiaGoDBHelper -sv -l 5 -svdb lt0474 -svs 4 -sve 100000 -k "" -p ${L} && date
Fri Jun  4 17:16:33 EDT 2021
[HGH] Hipparchia Golang Helper CLI Debugging Interface (v.1.0.7) [loglevel=5]
[HGH] Bagger Module Launched
[HGH] Seeking to build *winnertakesall* bags of words
[HGH] Connected to redis
[HGH] Connected to hipparchiaDB on PostgreSQL
[HGH] No redis key; gathering lines with a direct CLI PostgreSQL query%!(EXTRA string=hipparchiaDB)
[HGH] unified text block built [B: 0.218878s])
[HGH] preliminary cleanups complete [C: 0.264607s])
[HGH] found 51491 sentences [D: 0.978179s]
[HGH] found 65645 distinct words [E: 1.025116s]
[HGH] Got morphology [F: 2.549166s]
[HGH] Build morphdict [F1: 5.214109s]
[HGH] Pre-Bagging [F2: 5.246322s]
[HGH] Post-Bagging [F3: 5.570288s]
[HGH] Cleared stopwords [F4: 5.664482s]
[HGH] Reached result @ 7.092705s]
bags have been stored at _vectorresults
Fri Jun  4 17:16:40 EDT 2021
```