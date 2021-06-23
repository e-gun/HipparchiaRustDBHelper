//    HipparchiaRustDBHelper: search and vector helper app and functions for HipparchiaServer
//    Copyright: E Gunderson 21
//    License: GNU GENERAL PUBLIC LICENSE 3
//        (see LICENSE in the top level directory of the distribution)

pub struct DBLine {
    pub idx: i32,
    pub uid: String,
    pub l5: String,
    pub  l4: String,
    pub l3: String,
    pub l2: String,
    pub l1: String,
    pub l0: String,
    pub  mu: String,
    pub ac: String,
    pub st: String,
    pub  hy: String,
    pub an: String,
}

#[derive(Clone)]
pub struct DbMorphology {
    pub obs: String,
    pub xrf: String,
    pub pxr: String,
    pub rpo: String,
    pub upo: Vec<String>,
}

pub struct WeightedHeadword {
    pub wd: String,
    pub ct: i32,
}


