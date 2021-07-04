//    HipparchiaRustDBHelper: search and vector helper app and functions for HipparchiaServer
//    Copyright: E Gunderson 2021
//    License: GNU GENERAL PUBLIC LICENSE 3
//        (see LICENSE in the top level directory of the distribution)

use std::collections::HashMap;
use std::net::TcpListener;
use std::net::TcpStream;
use std::thread;
use std::thread::spawn;
use std::time::Duration;

use json::JsonValue;
use tungstenite::{accept_hdr, handshake::server::{Request, Response}, Message, WebSocket};

use crate::dbfunctions::*;
use crate::helpers::*;

static POLLINGINTERVAL: Duration = Duration::from_millis(400);

pub fn websocket(ft: &str, ll: i32, ip: &str, port: &str, save: i32, rc: String) {
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
                let msg: Message = ws.read_message().unwrap_or(Message::from("".to_string()));
                if msg.is_text() && msg != Message::from("".to_string()) {
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
                    // deletewhendone()
                    if save != 0 {
                        let thekey: String = format!("{}_poolofwork", rediskey);
                        let _ = rs_set_int(thekey.as_str(), -1, &mut redisconn);
                        let fields =  ws_fields();
                        for f in fields {
                            let _ = rs_del(f, &mut redisconn);
                        }
                        let m = format!("deleted redis keys for {}", &rediskey);
                        lfl(m, ll, 4);
                    }
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