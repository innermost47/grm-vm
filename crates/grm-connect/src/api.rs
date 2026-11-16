// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! APIs that live: an answer asked again every so often (HTTP GET), values sent every so often (HTTP POST), a
//! stream that pushes (WebSocket). Each runs in a thread of its own and hands what it read to the machine as
//! (node, value, sigma); what it sends, it asks the machine for. Said in the language of apps:
//!
//! ```text
//! api weather: get https://api.open-meteo.com/v1/forecast?latitude=45.76&longitude=4.84&current=temperature_2m every 600
//! read weather current.temperature_2m -> temp_lyon ± 0.5
//! api hook: post https://example.org/in every 60 with temp_lyon humidity
//! api prices: websocket wss://example.org/stream send volume every 1
//! read prices data.price -> price ± 0.01
//! ```
//!
//! A path goes into the JSON answer: names separated by dots, `[k]` an element of a list (`[-1]` its last), `.` the
//! whole answer (a number said alone).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use grm_engine::error::{GrmError, Result};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum Way {
    /// Asked again every `every` seconds.
    Get { url: String, every: f64 },
    /// The values of `with` sent every `every` seconds, as a JSON object.
    Post { url: String, every: f64, with: Vec<String> },
    /// A stream: each message read; the values of `send` sent every `every` seconds (if any).
    Websocket { url: String, send: Vec<String>, every: f64 },
}

/// What is read from an answer: where (a path), into which node, how sure.
#[derive(Debug, Clone, PartialEq)]
pub struct Read {
    pub path: String,
    pub node: String,
    pub sigma: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Api {
    pub name: String,
    pub way: Way,
    pub reads: Vec<Read>,
}

fn number(s: &str) -> Result<f64> {
    s.parse::<f64>().map_err(|_| GrmError::new(format!("{s} is not a number")))
}

/// `api NAME: get URL [every S]` | `api NAME: post URL [every S] with A B..` | `api NAME: websocket URL [send A B.. [every S]]`
pub fn parse_api(rest: &str) -> Result<Api> {
    let Some((name, body)) = rest.split_once(':') else {
        return Err(GrmError::new("api NAME: get|post|websocket URL ..."));
    };
    let name = name.trim().to_string();
    if !grm_engine::lang::is_name(&name) {
        return Err(GrmError::new(format!("{name} is not a name")));
    }
    let words: Vec<&str> = body.split_whitespace().collect();
    let (Some(kind), Some(url)) = (words.first(), words.get(1)) else {
        return Err(GrmError::new("api NAME: get|post|websocket URL ..."));
    };
    let after = |w: &str| words.iter().position(|x| *x == w);
    let every = match after("every") {
        Some(k) => number(words.get(k + 1).copied().unwrap_or(""))?,
        None => 60.0,
    }
    .max(0.05);
    let list = |w: &str| -> Vec<String> {
        after(w)
            .map(|k| words[k + 1..].iter().take_while(|x| **x != "every").map(|x| x.to_string()).collect())
            .unwrap_or_default()
    };
    let way = match kind.to_lowercase().as_str() {
        "get" => Way::Get { url: url.to_string(), every },
        "post" => {
            let with = list("with");
            if with.is_empty() {
                return Err(GrmError::new("api NAME: post URL every S with NODE.."));
            }
            Way::Post { url: url.to_string(), every, with }
        }
        "websocket" | "ws" => Way::Websocket { url: url.to_string(), send: list("send"), every },
        k => return Err(GrmError::new(format!("{k}: get, post or websocket"))),
    };
    Ok(Api { name, way, reads: vec![] })
}

/// `read NAME PATH -> NODE [± SIGMA]`: returns (api, read).
pub fn parse_read(rest: &str) -> Result<(String, Read)> {
    let Some((left, right)) = rest.split_once("->") else {
        return Err(GrmError::new("read API PATH -> NODE [± SIGMA]"));
    };
    let mut l = left.split_whitespace();
    let (Some(api), Some(path)) = (l.next(), l.next()) else {
        return Err(GrmError::new("read API PATH -> NODE [± SIGMA]"));
    };
    let right = right.replace("+/-", "±");
    let (node, sigma) = match right.split_once('±') {
        Some((n, s)) => (n.trim().to_string(), Some(number(s.trim())?)),
        None => (right.trim().to_string(), None),
    };
    if !grm_engine::lang::is_name(&node) {
        return Err(GrmError::new(format!("{node} is not a name")));
    }
    Ok((api.to_string(), Read { path: path.to_string(), node, sigma }))
}

/// The value at a path of an answer.
pub fn at_path(v: &Value, path: &str) -> Option<f64> {
    let mut cur = v;
    if path != "." {
        for part in path.split('.') {
            let (key, index) = match part.split_once('[') {
                Some((k, i)) => (k, Some(i.trim_end_matches(']'))),
                None => (part, None),
            };
            if !key.is_empty() {
                cur = match cur {
                    Value::Object(m) => m.get(key)?,
                    Value::Array(a) => a.get(key.parse::<usize>().ok()?)?,
                    _ => return None,
                };
            }
            if let Some(i) = index {
                let a = cur.as_array()?;
                let k: i64 = i.parse().ok()?;
                let k = if k < 0 { a.len() as i64 + k } else { k };
                cur = a.get(usize::try_from(k).ok()?)?;
            }
        }
    }
    match cur {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

/// Asked for the values an api sends: given the names, gives what the machine holds now.
pub type Values = Arc<dyn Fn(&[String]) -> Vec<(String, f64)> + Send + Sync>;

/// What an api hands the machine, and what it says about itself.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// (node, value, sigma) read from an answer.
    Values(String, Vec<(String, f64, Option<f64>)>),
    /// A line to show (it connected, it failed, what it sent).
    Said(String, String),
}

/// A running api: stops when dropped.
pub struct Running {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    pub api: Api,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            // a thread waiting on the network is not waited for: it ends by itself after its call
            if t.is_finished() {
                let _ = t.join();
            }
        }
    }
}

fn read_all(api: &Api, text: &str) -> Vec<(String, f64, Option<f64>)> {
    let value: Value = serde_json::from_str(text)
        .unwrap_or_else(|_| text.trim().parse::<f64>().map(|x| serde_json::json!(x)).unwrap_or(Value::Null));
    api.reads.iter().filter_map(|r| at_path(&value, &r.path).map(|x| (r.node.clone(), x, r.sigma))).collect()
}

/// An error said without the address it is about (it is long, and said already), on one line.
fn short(message: &str, url: &str) -> String {
    let m = message.replace(&format!("{url}: "), "").replace(url, "").replace('\n', " ");
    if m.contains("Dns Failed") || m.contains("failed to lookup") {
        format!("cannot reach the server ({})", url.split('/').nth(2).unwrap_or(url))
    } else {
        m
    }
}

fn sleep_until(stop: &AtomicBool, until: Instant) -> bool {
    while Instant::now() < until {
        if stop.load(Ordering::Relaxed) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50).min(until.saturating_duration_since(Instant::now())));
    }
    !stop.load(Ordering::Relaxed)
}

pub fn post(url: &str, body: &str) -> Result<String> {
    let answer = ureq::post(url)
        .set("User-Agent", "grm-vm")
        .set("Content-Type", "application/json")
        .send_string(body)
        .map_err(|e| GrmError::new(format!("{url}: {e}")))?;
    answer.into_string().map_err(|e| GrmError::new(format!("{url}: {e}")))
}

/// Starts an api. `events`: where what it reads and says goes. `values`: asked for the values it sends (it is
/// given the names, it gives back what the machine holds now).
pub fn start(api: Api, events: Sender<Event>, values: Values) -> Running {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let a = api.clone();
    let name = api.name.clone();
    let thread = std::thread::Builder::new()
        .name(format!("api-{name}"))
        .spawn(move || {
            let say = |t: String| {
                let _ = events.send(Event::Said(a.name.clone(), t));
            };
            match &a.way {
                Way::Get { url, every } => loop {
                    let next = Instant::now() + Duration::from_secs_f64(*every);
                    match crate::fetch(url) {
                        Ok(text) => {
                            let got = read_all(&a, &text);
                            if got.is_empty() && !a.reads.is_empty() {
                                say(format!("{}: nothing read at its paths", a.name));
                            }
                            let _ = events.send(Event::Values(a.name.clone(), got));
                        }
                        Err(e) => say(format!("{}: {}", a.name, short(&e.message, url))),
                    }
                    if !sleep_until(&flag, next) {
                        return;
                    }
                },
                Way::Post { url, every, with } => loop {
                    let next = Instant::now() + Duration::from_secs_f64(*every);
                    let body: serde_json::Map<String, Value> =
                        values(with).into_iter().map(|(k, v)| (k, serde_json::json!(v))).collect();
                    let body = Value::Object(body).to_string();
                    match post(url, &body) {
                        Ok(text) => {
                            let got = read_all(&a, &text);
                            if !got.is_empty() {
                                let _ = events.send(Event::Values(a.name.clone(), got));
                            }
                        }
                        Err(e) => say(format!("{}: {}", a.name, short(&e.message, url))),
                    }
                    if !sleep_until(&flag, next) {
                        return;
                    }
                },
                Way::Websocket { url, send, every } => {
                    let mut wait = 1.0;
                    while !flag.load(Ordering::Relaxed) {
                        match tungstenite::connect(url.as_str()) {
                            Ok((mut socket, _)) => {
                                say(format!("{}: connected to {url}", a.name));
                                wait = 1.0;
                                if let tungstenite::stream::MaybeTlsStream::Plain(s) = socket.get_mut() {
                                    let _ = s.set_read_timeout(Some(Duration::from_millis(100)));
                                }
                                let mut next = Instant::now();
                                loop {
                                    if flag.load(Ordering::Relaxed) {
                                        let _ = socket.close(None);
                                        return;
                                    }
                                    if !send.is_empty() && Instant::now() >= next {
                                        next = Instant::now() + Duration::from_secs_f64(*every);
                                        let body: serde_json::Map<String, Value> =
                                            values(send).into_iter().map(|(k, v)| (k, serde_json::json!(v))).collect();
                                        if socket
                                            .send(tungstenite::Message::text(Value::Object(body).to_string()))
                                            .is_err()
                                        {
                                            break;
                                        }
                                    }
                                    match socket.read() {
                                        Ok(tungstenite::Message::Text(t)) => {
                                            let got = read_all(&a, t.as_str());
                                            if !got.is_empty() {
                                                let _ = events.send(Event::Values(a.name.clone(), got));
                                            }
                                        }
                                        Ok(tungstenite::Message::Close(_)) => break,
                                        Ok(_) => {}
                                        Err(tungstenite::Error::Io(e))
                                            if matches!(
                                                e.kind(),
                                                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                                            ) => {}
                                        Err(e) => {
                                            say(format!("{}: {e}", a.name));
                                            break;
                                        }
                                    }
                                }
                            }
                            Err(e) => say(format!("{}: {e} (tries again in {wait:.0} s)", a.name)),
                        }
                        if !sleep_until(&flag, Instant::now() + Duration::from_secs_f64(wait)) {
                            return;
                        }
                        wait = (wait * 2.0).min(60.0);
                    }
                }
            }
        })
        .expect("an api's thread");
    Running { stop, thread: Some(thread), api }
}

/// A channel for events.
pub fn events() -> (Sender<Event>, Receiver<Event>) {
    channel()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read as _, Write};

    #[test]
    fn it_says_and_reads_what_it_is_told() {
        let a = parse_api("w: get http://x/y?a=1&b=2 every 600").unwrap();
        assert_eq!(a.way, Way::Get { url: "http://x/y?a=1&b=2".into(), every: 600.0 });
        let p = parse_api("h: post http://x every 5 with a b").unwrap();
        assert_eq!(p.way, Way::Post { url: "http://x".into(), every: 5.0, with: vec!["a".into(), "b".into()] });
        let s = parse_api("s: websocket ws://x send v every 1").unwrap();
        assert_eq!(s.way, Way::Websocket { url: "ws://x".into(), send: vec!["v".into()], every: 1.0 });
        let (api, r) = parse_read("w hourly.temperature_2m[-1] -> temp ± 0.5").unwrap();
        assert_eq!((api.as_str(), r.node.as_str(), r.sigma), ("w", "temp", Some(0.5)));
        let v: Value =
            serde_json::from_str(r#"{"hourly": {"temperature_2m": [1, 2, 3.5]}, "list": [{"p": "4.5"}]}"#).unwrap();
        assert_eq!(at_path(&v, "hourly.temperature_2m[-1]"), Some(3.5));
        assert_eq!(at_path(&v, "hourly.temperature_2m[0]"), Some(1.0));
        assert_eq!(at_path(&v, "list[0].p"), Some(4.5));
        assert_eq!(at_path(&serde_json::json!(7), "."), Some(7.0));
        assert!(parse_api("x: fly http://x").is_err());
    }

    #[test]
    fn a_get_asks_again_and_hands_what_it_read() {
        // a small server on this machine: it answers twice
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for (k, stream) in listener.incoming().take(2).enumerate() {
                let mut s = stream.unwrap();
                let mut buf = [0u8; 1024];
                let _ = s.read(&mut buf);
                let body = format!("{{\"current\": {{\"t\": {}}}}}", 20 + k);
                let _ =
                    write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            }
        });
        let mut api = parse_api(&format!("w: get http://127.0.0.1:{port}/ every 0.2")).unwrap();
        api.reads.push(parse_read("w current.t -> temp ± 0.5").unwrap().1);
        let (tx, rx) = events();
        let running = start(api, tx, Arc::new(|_| vec![]));
        let mut got = vec![];
        while got.len() < 2 {
            if let Event::Values(_, v) = rx.recv_timeout(Duration::from_secs(5)).unwrap() {
                got.extend(v);
            }
        }
        assert_eq!(got[0], ("temp".into(), 20.0, Some(0.5)));
        assert_eq!(got[1].1, 21.0);
        drop(running);
    }

    #[test]
    fn a_websocket_pushes_and_is_sent_to() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (heard_tx, heard) = channel::<String>();
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut ws = tungstenite::accept(stream).unwrap();
            for k in 0..3 {
                ws.send(tungstenite::Message::text(format!("{{\"data\": {{\"price\": {}}}}}", 100 + k))).unwrap();
            }
            if let Ok(tungstenite::Message::Text(t)) = ws.read() {
                let _ = heard_tx.send(t.to_string());
            }
        });
        let mut api = parse_api(&format!("s: websocket ws://127.0.0.1:{port}/ send volume every 0.1")).unwrap();
        api.reads.push(parse_read("s data.price -> price").unwrap().1);
        let (tx, rx) = events();
        let running = start(api, tx, Arc::new(|names: &[String]| names.iter().map(|n| (n.clone(), 0.5)).collect()));
        let mut got = vec![];
        while got.len() < 3 {
            if let Event::Values(_, v) = rx.recv_timeout(Duration::from_secs(5)).unwrap() {
                got.extend(v);
            }
        }
        assert_eq!(got.iter().map(|g| g.1).collect::<Vec<_>>(), vec![100.0, 101.0, 102.0]);
        assert_eq!(heard.recv_timeout(Duration::from_secs(5)).unwrap(), r#"{"volume":0.5}"#);
        drop(running);
    }
}
