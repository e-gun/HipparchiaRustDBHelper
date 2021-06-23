static SHORTNAME: &str = "HRH";

pub fn lfl(message: String, loglevel: i32, threshold: i32) {
    // log if logging
    if loglevel >= threshold {
        println!("[{}] {}", SHORTNAME, message);
    }
}

// https://stackoverflow.com/questions/38406793/why-is-capitalizing-the-first-letter-of-a-string-so-convoluted-in-rust/53571882#53571882
pub fn make_ascii_title_case(s: &mut str) {
    if let Some(r) = s.get_mut(0..1) {
        r.make_ascii_uppercase();
    }
}

