#!/bin/sh

cargo build
D="target/debug/"
O="hipparchia_rust_dbhelper"
P="HipparchiaRustDBHelper"
T="../HipparchiaRustBinaries/cli_prebuilt_binaries"
mv ${D}${O} ${D}${P}
V=$(./${D}${P} -V | cut -d" " -f 4 | tail -n 1)
U=$(uname)
H="${HOME}/hipparchia_venv/HipparchiaServer/server/externalbinaries/"
cp ${D}${P} ${H}
cp ${D}${P} ${T}/${P}-$U-${V}
rm ${T}/${P}-${U}-${V}.bz2
bzip2 ${T}/${P}-${U}-${V}
cp ${T}/${P}-${U}-${V}.bz2 ${T}/${P}-${U}-latest.bz2

echo "Latest ${U} is ${V}" > ${T}/latest_${U}.txt