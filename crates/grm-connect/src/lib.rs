// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The machine's connections to the world: a web API is asked, what it can give is listed (each field with its
//! kind and its unit), the human chooses, and the chosen fields become a network in a sandbox, with their rows,
//! each at its moment. The global network then tries the merge, and decides.
//!
//! - `discover`: the fields of an answer, whatever its shape (columns side by side as Open-Meteo gives them,
//!   records, a CSV as Our World in Data gives them): numbers, moments, texts.
//! - `table`: the chosen fields as rows, each at its moment (hours since 1970 for a date and time, the year for a
//!   year).
//! - `into_sandbox`: a sandbox with an input node per chosen number; texts are shown, not yet turned into nodes.
//! - Two APIs to begin with: Open-Meteo (the weather, no key) and Our World in Data (a chart's CSV and metadata).
//!
//! HTTP for now; WebSocket comes with the connectors milestone.

pub mod api;
pub mod csv;

use indexmap::IndexMap;
use serde_json::Value;

use grm_engine::error::{GrmError, Result};
use grm_engine::lang::is_name;
use grm_engine::world::World;

/// What a field holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Number,
    /// A date, or a date and time.
    Moment,
    Text,
}

/// A field an answer can give.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    /// Where it is: "hourly.temperature_2m", or a column of a CSV.
    pub path: String,
    pub kind: FieldKind,
    pub unit: Option<String>,
    pub count: usize,
    /// A few of its values, as they came.
    pub sample: Vec<String>,
}

impl Field {
    /// The name its node would have (a name of the language).
    pub fn node_name(&self) -> String {
        name_of(self.path.rsplit('.').next().unwrap_or(&self.path))
    }

    pub fn text(&self) -> String {
        let kind = match self.kind {
            FieldKind::Number => "number",
            FieldKind::Moment => "moment",
            FieldKind::Text => "text",
        };
        format!(
            "{} ({kind}{}, {} value(s), e.g. {})",
            self.path,
            self.unit.as_ref().map(|u| format!(", {u}")).unwrap_or_default(),
            self.count,
            self.sample.join(", ")
        )
    }
}

/// A heading as a name of the language: what is not a letter, a digit or _ becomes _, a leading digit gets a _.
pub fn name_of(heading: &str) -> String {
    let mut s: String =
        heading.trim().chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    while s.contains("__") {
        s = s.replace("__", "_");
    }
    let s = s.trim_matches('_').to_string();
    if s.is_empty() {
        "_".into()
    } else if s.chars().next().unwrap().is_ascii_digit() {
        format!("_{s}")
    } else {
        s
    }
}

// ---------------------------------------------------------------------- moments
/// Days since 1970-01-01 of a civil date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// "2026-11-16T10:00" or "2026-11-16" -> hours since 1970 (UTC, as Open-Meteo gives them by default).
pub fn moment(text: &str) -> Option<f64> {
    let t = text.trim();
    let (date, time) = t.split_once('T').or_else(|| t.split_once(' ')).unwrap_or((t, ""));
    let mut parts = date.split('-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let d: i64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let mut hours = days_from_civil(y, m, d) as f64 * 24.0;
    if !time.is_empty() {
        let mut hm = time.trim_end_matches('Z').split(':');
        let h: f64 = hm.next()?.parse().ok()?;
        let mi: f64 = hm.next().unwrap_or("0").parse().ok()?;
        hours += h + mi / 60.0;
    }
    Some(hours)
}

fn kind_of(values: &[&Value]) -> FieldKind {
    let present: Vec<&&Value> = values.iter().filter(|v| !v.is_null()).collect();
    if !present.is_empty() && present.iter().all(|v| v.is_number()) {
        FieldKind::Number
    } else if !present.is_empty() && present.iter().all(|v| v.as_str().is_some_and(|s| moment(s).is_some())) {
        FieldKind::Moment
    } else {
        FieldKind::Text
    }
}

fn shown(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------- what an answer can give
/// The fields of a JSON answer: arrays side by side in an object (columns), arrays of objects (records), and
/// single values. Units are read from a sibling object named `<block>_units` (Open-Meteo's way).
pub fn discover(json: &Value) -> Vec<Field> {
    let mut out = vec![];
    walk(json, "", json, &mut out);
    out
}

fn walk(v: &Value, path: &str, root: &Value, out: &mut Vec<Field>) {
    match v {
        Value::Object(map) => {
            for (k, child) in map {
                if k.ends_with("_units") {
                    continue;
                }
                let p = if path.is_empty() { k.clone() } else { format!("{path}.{k}") };
                walk(child, &p, root, out);
            }
        }
        Value::Array(items) if items.iter().all(|i| i.is_object()) && !items.is_empty() => {
            // records: each key of the objects is a field
            let mut keys: Vec<String> = vec![];
            for i in items {
                for k in i.as_object().unwrap().keys() {
                    if !keys.contains(k) {
                        keys.push(k.clone());
                    }
                }
            }
            for k in keys {
                let vals: Vec<&Value> = items.iter().map(|i| i.get(&k).unwrap_or(&Value::Null)).collect();
                out.push(field(&format!("{path}[].{k}"), &vals, None));
            }
        }
        Value::Array(items) => {
            let vals: Vec<&Value> = items.iter().collect();
            out.push(field(path, &vals, unit(root, path)));
        }
        single => out.push(field(path, &[single], unit(root, path))),
    }
}

fn unit(root: &Value, path: &str) -> Option<String> {
    let (block, name) = path.rsplit_once('.').unwrap_or(("", path));
    let units = if block.is_empty() { root.get("units") } else { root.get(format!("{block}_units")) };
    units.and_then(|u| u.get(name)).and_then(|u| u.as_str()).map(|s| s.to_string())
}

fn field(path: &str, vals: &[&Value], unit: Option<String>) -> Field {
    Field {
        path: path.to_string(),
        kind: kind_of(vals),
        unit,
        count: vals.iter().filter(|v| !v.is_null()).count(),
        sample: vals.iter().filter(|v| !v.is_null()).take(3).map(|v| shown(v)).collect(),
    }
}

/// A table: the chosen fields, row by row, each row at its moment.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Table {
    /// The moment of each row (hours since 1970, or a year, or the row's number when nothing says when).
    pub time: Vec<f64>,
    /// What the moments are counted in.
    pub time_unit: String,
    pub numbers: IndexMap<String, Vec<Option<f64>>>,
    pub texts: IndexMap<String, Vec<String>>,
    pub units: IndexMap<String, String>,
}

impl Table {
    /// The rows as the machine takes them: (moment, {node: value}); a missing value is no measurement.
    pub fn rows(&self) -> Vec<(f64, IndexMap<String, f64>)> {
        (0..self.time.len())
            .map(|k| {
                (self.time[k], self.numbers.iter().map(|(n, col)| (n.clone(), col[k].unwrap_or(f64::NAN))).collect())
            })
            .collect()
    }
}

fn column<'a>(json: &'a Value, path: &str) -> Option<Vec<&'a Value>> {
    let mut v = json;
    for part in path.split('.') {
        v = v.get(part)?;
    }
    v.as_array().map(|a| a.iter().collect())
}

/// The chosen fields of a JSON answer of columns side by side, as a table; the moment is the first field of the
/// same block that holds moments.
pub fn table(json: &Value, chosen: &[&str]) -> Result<Table> {
    let fields = discover(json);
    let mut t = Table { time_unit: "row".into(), ..Default::default() };
    let mut len = None;
    for path in chosen {
        let Some(f) = fields.iter().find(|f| f.path == *path) else {
            return Err(GrmError::new(format!("no field {path}")));
        };
        let Some(col) = column(json, path) else { return Err(GrmError::new(format!("{path} is not a column"))) };
        if *len.get_or_insert(col.len()) != col.len() {
            return Err(GrmError::new(format!("{path} is not as long as the other fields")));
        }
        let name = f.node_name();
        match f.kind {
            FieldKind::Number => {
                t.numbers.insert(name.clone(), col.iter().map(|v| v.as_f64()).collect());
            }
            _ => {
                t.texts.insert(name.clone(), col.iter().map(|v| shown(v)).collect());
            }
        }
        if let Some(u) = &f.unit {
            t.units.insert(name, u.clone());
        }
    }
    let n = len.unwrap_or(0);
    // the moment of each row: a field of moments in the same block
    let block = chosen.first().and_then(|p| p.rsplit_once('.')).map(|(b, _)| b.to_string()).unwrap_or_default();
    let clock = fields
        .iter()
        .find(|f| f.kind == FieldKind::Moment && f.path.rsplit_once('.').map(|(b, _)| b) == Some(block.as_str()));
    t.time = match clock.and_then(|f| column(json, &f.path)) {
        Some(col) if col.len() == n => {
            t.time_unit = "hour".into();
            col.iter().map(|v| v.as_str().and_then(moment).unwrap_or(f64::NAN)).collect()
        }
        _ => (0..n).map(|k| k as f64).collect(),
    };
    Ok(t)
}

// ---------------------------------------------------------------------- CSV (Our World in Data)
/// A CSV line split at its commas (quotes respected).
fn csv_line(line: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// A CSV as JSON records (what `discover` reads).
pub fn csv_records(text: &str) -> Value {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let Some(header) = lines.next() else { return Value::Array(vec![]) };
    let names = csv_line(header);
    let records = lines
        .map(|l| {
            let cells = csv_line(l);
            let obj = names
                .iter()
                .zip(cells.iter().chain(std::iter::repeat(&String::new())))
                .map(|(n, c)| {
                    let v = if c.is_empty() {
                        Value::Null
                    } else {
                        c.parse::<f64>()
                            .ok()
                            .and_then(serde_json::Number::from_f64)
                            .map(Value::Number)
                            .unwrap_or(Value::String(c.clone()))
                    };
                    (n.clone(), v)
                })
                .collect();
            Value::Object(obj)
        })
        .collect();
    Value::Array(records)
}

/// The rows of a CSV of Our World in Data for one entity (a country...), the chosen columns, by year.
pub fn owid_table(csv: &str, entity: &str, chosen: &[&str], metadata: Option<&Value>) -> Result<Table> {
    let records = csv_records(csv);
    let rows: Vec<&Value> = records
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r.get("Entity").and_then(|e| e.as_str()) == Some(entity))
        .collect();
    if rows.is_empty() {
        return Err(GrmError::new(format!("no row for {entity}")));
    }
    let time_key = ["Year", "Day", "year", "date"].into_iter().find(|k| rows[0].get(*k).is_some()).unwrap_or("Year");
    let mut t = Table { time_unit: time_key.to_lowercase(), ..Default::default() };
    t.time = rows
        .iter()
        .map(|r| match r.get(time_key) {
            Some(Value::Number(n)) => n.as_f64().unwrap_or(f64::NAN),
            Some(Value::String(s)) => moment(s).unwrap_or(f64::NAN),
            _ => f64::NAN,
        })
        .collect();
    for c in chosen {
        let name = name_of(c);
        t.numbers.insert(name.clone(), rows.iter().map(|r| r.get(*c).and_then(|v| v.as_f64())).collect());
        if let Some(u) = metadata
            .and_then(|m| m.get("columns"))
            .and_then(|cols| cols.get(*c))
            .and_then(|col| col.get("unit"))
            .and_then(|u| u.as_str())
        {
            t.units.insert(name, u.to_string());
        }
    }
    Ok(t)
}

// ---------------------------------------------------------------------- the two first APIs
/// Open-Meteo: the weather of a place, hour by hour (past days and coming days), no key needed.
pub fn open_meteo_url(latitude: f64, longitude: f64, hourly: &[&str], past_days: u32, forecast_days: u32) -> String {
    format!(
        "https://api.open-meteo.com/v1/forecast?latitude={latitude}&longitude={longitude}&hourly={}&past_days={past_days}&forecast_days={forecast_days}&timezone=UTC",
        hourly.join(",")
    )
}

/// The weather variables asked by default (each becomes a field to choose from).
pub const WEATHER: [&str; 5] =
    ["temperature_2m", "relative_humidity_2m", "pressure_msl", "wind_speed_10m", "precipitation"];

/// Our World in Data: a chart's data (CSV) and its metadata (units, sources), by the chart's slug ("co2").
pub fn owid_urls(slug: &str) -> (String, String) {
    (
        format!("https://ourworldindata.org/grapher/{slug}.csv?useColumnShortNames=true"),
        format!("https://ourworldindata.org/grapher/{slug}.metadata.json"),
    )
}

/// Asks a URL (HTTP GET) and returns its answer as text.
pub fn fetch(url: &str) -> Result<String> {
    let answer = ureq::get(url).set("User-Agent", "grm-vm").call().map_err(|e| GrmError::new(format!("{url}: {e}")))?;
    answer.into_string().map_err(|e| GrmError::new(format!("{url}: {e}")))
}

// ---------------------------------------------------------------------- into the machine
/// A sandbox holding the chosen numbers as input nodes (each with its unit in its provenance's version field
/// for now). The rows wait in the table: once merged, `World::feed_rows` gives them, each at its moment.
pub fn into_sandbox(world: &mut World, name: &str, table: &Table) -> Result<Vec<String>> {
    world.open_sandbox(name)?;
    let mut nodes = vec![];
    for n in table.numbers.keys() {
        if !is_name(n) {
            return Err(GrmError::new(format!("{n} is not a name of the language")));
        }
        let first = table.numbers[n].iter().flatten().next().copied().unwrap_or(0.0);
        world.say_in(name, &format!("input {n} = {first}"))?;
        nodes.push(n.clone());
    }
    Ok(nodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use grm_engine::network::Provenance;
    use grm_engine::time::{civil, Rng};

    /// An answer shaped exactly as Open-Meteo gives it: 30 days, hour by hour (made here: the machine that wrote
    /// this could not reach the site).
    fn open_meteo_like(days: usize) -> Value {
        let mut rng = Rng::new(11);
        let start = moment("2026-10-01T00:00").unwrap();
        let (mut time, mut temp, mut hum, mut text) = (vec![], vec![], vec![], vec![]);
        let mut weather = 0.0;
        let (mut humidity, mut temp_before) = (80.0, 12.0);
        for h in 0..days * 24 {
            let hours = start + h as f64;
            let days_since = (hours / 24.0).floor() as i64;
            let (y, m, d) = civil(days_since);
            time.push(Value::String(format!("{y:04}-{m:02}-{d:02}T{:02}:00", h % 24)));
            weather = 0.97 * weather + 0.4 * rng.normal(); // fronts passing
            let t = 12.0
                + weather
                + 5.0 * (std::f64::consts::TAU * ((h % 24) as f64 - 9.0) / 24.0).sin()
                + 0.2 * rng.normal();
            temp.push(serde_json::json!((t * 10.0).round() / 10.0));
            // the air dries as it warms, an hour later (it follows the temperature the moment before)
            humidity = 0.7 * humidity + 0.3 * (80.0 - 2.5 * (temp_before - 12.0)) + 0.3 * rng.normal();
            temp_before = t;
            hum.push(serde_json::json!((humidity * 10.0).round() / 10.0));
            text.push(Value::String(if t > 14.0 { "mild".into() } else { "cool".into() }));
        }
        serde_json::json!({
            "latitude": 48.86, "longitude": 2.35, "generationtime_ms": 0.1, "utc_offset_seconds": 0, "timezone": "UTC",
            "elevation": 43.0,
            "hourly_units": {"time": "iso8601", "temperature_2m": "°C", "relative_humidity_2m": "%", "sky": ""},
            "hourly": {"time": time, "temperature_2m": temp, "relative_humidity_2m": hum, "sky": text}
        })
    }

    #[test]
    fn moments_are_counted_in_hours_since_1970() {
        assert_eq!(moment("1970-01-02T01:30"), Some(25.5));
        assert_eq!(moment("2000-03-01"), Some(days_from_civil(2000, 3, 1) as f64 * 24.0));
        assert_eq!(days_from_civil(2000, 3, 1), 11017);
        assert_eq!(civil(11017), (2000, 3, 1));
        assert_eq!(grm_engine::time::moment_text(moment("2026-11-16T10:30").unwrap(), "hour"), "2026-11-16 10:30");
        assert!(moment("hello").is_none());
        assert_eq!(name_of("CO2 (ppm)"), "CO2_ppm");
        assert_eq!(name_of("2020"), "_2020");
    }

    #[test]
    fn what_an_answer_can_give_is_listed_with_kinds_and_units() {
        let json = open_meteo_like(2);
        let fields = discover(&json);
        let f = |p: &str| fields.iter().find(|f| f.path == p).unwrap().clone();
        assert_eq!(f("hourly.temperature_2m").kind, FieldKind::Number);
        assert_eq!(f("hourly.temperature_2m").unit.as_deref(), Some("°C"));
        assert_eq!(f("hourly.time").kind, FieldKind::Moment);
        assert_eq!(f("hourly.sky").kind, FieldKind::Text);
        assert_eq!(f("hourly.temperature_2m").count, 48);
        assert_eq!(f("latitude").kind, FieldKind::Number);
    }

    #[test]
    fn the_weather_comes_in_through_a_sandbox_and_is_foreseen_with_its_reasons() {
        let json = open_meteo_like(30);
        let table = table(&json, &["hourly.temperature_2m", "hourly.relative_humidity_2m", "hourly.sky"]).unwrap();
        assert_eq!(table.time_unit, "hour");
        assert_eq!(table.texts["sky"].len(), 720);
        let mut w = World::new();
        let nodes = into_sandbox(&mut w, "meteo", &table).unwrap();
        assert_eq!(nodes, vec!["temperature_2m", "relative_humidity_2m"]);
        let report = w.trial_merge("meteo", 1000).unwrap();
        assert!(report.compatible(), "{}", report.text());
        w.accept_merge("meteo").unwrap();
        w.track("temperature_2m", 2, &[]).unwrap();
        w.track("relative_humidity_2m", 1, &[]).unwrap();
        let rows = table.rows();
        let (past, future) = rows.split_at(rows.len() - 24);
        w.feed_rows(past).unwrap();
        // the humidity follows the temperature: the machine finds the link by itself
        assert!(
            w.journal
                .iter()
                .any(|e| e.kind == "insight" && e.text.starts_with("relative_humidity_2m follows temperature_2m")),
            "{:?}",
            w.journal
        );
        // foresight: the next 24 hours, with their spread
        let f = w.time.forecast(24, 300, 1.0);
        let path = &f.paths["temperature_2m"];
        let truth: Vec<f64> = future.iter().map(|(_, r)| r["temperature_2m"]).collect();
        let inside = (0..24).filter(|&h| (truth[h] - path.mean[h]).abs() <= 2.0 * path.sigma[h]).count();
        assert!(inside >= 20, "{inside}/24 within two sigma");
        let why = w.time.why("relative_humidity_2m", 6, 300).unwrap();
        let text = why.text(&w.node_sources());
        println!("{text}\n{}", w.time.why("temperature_2m", 24, 300).unwrap().text(&w.node_sources()));
        assert!(why.nodes.contains(&"temperature_2m".to_string()), "{text}");
        assert!(text.contains("sandbox:meteo"), "{text}");
        // rewind: what the temperature was ten days ago, as remembered
        let ten_days_ago = past.last().unwrap().0 - 240.0;
        let (v, spread) = w.time.rewind("temperature_2m", ten_days_ago).unwrap();
        let truth_then = past.iter().find(|(t, _)| *t == ten_days_ago).unwrap().1["temperature_2m"];
        assert!((v - truth_then).abs() <= 2.0 * spread + 1.0, "{v} ± {spread} vs {truth_then}");
        let _ = Provenance::default();
    }

    #[test]
    fn a_csv_of_our_world_in_data_gives_its_rows_by_year() {
        let csv = "Entity,Code,Year,co2,\"note, with comma\"\nFrance,FRA,2020,276.6,a\nFrance,FRA,2021,299.1,b\nWorld,OWID_WRL,2020,34800.5,c\n";
        let meta = serde_json::json!({"columns": {"co2": {"unit": "million tonnes"}}});
        let t = owid_table(csv, "France", &["co2"], Some(&meta)).unwrap();
        assert_eq!(t.time, vec![2020.0, 2021.0]);
        assert_eq!(t.numbers["co2"], vec![Some(276.6), Some(299.1)]);
        assert_eq!(t.units["co2"], "million tonnes");
        let fields = discover(&csv_records(csv));
        assert!(fields.iter().any(|f| f.path == "[].note, with comma" && f.kind == FieldKind::Text));
    }
}

// ---------------------------------------------------------------------- pictures
/// A PNG as the eye takes it: grey or colour, its values in 0 .. 1 (its transparency left out).
pub fn read_picture(path: &std::path::Path) -> Result<grm_engine::vision::Picture> {
    let file = std::fs::File::open(path).map_err(|e| GrmError::new(format!("{}: {e}", path.display())))?;
    let mut decoder = png::Decoder::new(file);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| GrmError::new(format!("{}: {e}", path.display())))?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| GrmError::new(format!("{}: {e}", path.display())))?;
    let (w, h) = (info.width as usize, info.height as usize);
    let (stride, channels) = match info.color_type {
        png::ColorType::Grayscale => (1, 1),
        png::ColorType::GrayscaleAlpha => (2, 1),
        png::ColorType::Rgb => (3, 3),
        png::ColorType::Rgba => (4, 3),
        png::ColorType::Indexed => (3, 3),
    };
    let mut data = Vec::with_capacity(w * h * channels);
    for y in 0..h {
        let row = &buf[y * info.line_size..];
        for x in 0..w {
            for c in 0..channels {
                data.push(row[x * stride + c] as f32 / 255.0);
            }
        }
    }
    Ok(grm_engine::vision::Picture { width: w, height: h, channels, data })
}

/// Writes a picture as a PNG (values clamped to 0 .. 1; what was not seen, black).
pub fn write_picture(path: &std::path::Path, picture: &grm_engine::vision::Picture) -> Result<()> {
    let file = std::fs::File::create(path).map_err(|e| GrmError::new(format!("{}: {e}", path.display())))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), picture.width as u32, picture.height as u32);
    encoder.set_color(if picture.channels == 3 { png::ColorType::Rgb } else { png::ColorType::Grayscale });
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| GrmError::new(format!("{}: {e}", path.display())))?;
    let channels = if picture.channels == 3 { 3 } else { 1 };
    let mut bytes = Vec::with_capacity(picture.width * picture.height * channels);
    for p in 0..picture.width * picture.height {
        for c in 0..channels {
            let v = picture.data[p * picture.channels + c.min(picture.channels - 1)];
            bytes.push(if v.is_finite() { (v.clamp(0.0, 1.0) * 255.0).round() as u8 } else { 0 });
        }
    }
    writer.write_image_data(&bytes).map_err(|e| GrmError::new(format!("{}: {e}", path.display())))
}

/// A map of sigmas as a picture: calm is black, three sigmas is grey, far beyond is white.
pub fn sigma_picture(map: &grm_engine::vision::Picture) -> grm_engine::vision::Picture {
    let mut p = map.clone();
    for v in p.data.iter_mut() {
        *v = if v.is_finite() { *v / (*v + 3.0) } else { 0.0 };
    }
    p
}

#[cfg(test)]
mod picture_tests {
    use super::*;

    #[test]
    fn a_picture_goes_out_and_comes_back() {
        let mut p = grm_engine::vision::Picture::new(5, 3, 3);
        for (k, v) in p.data.iter_mut().enumerate() {
            *v = (k % 7) as f32 / 6.0;
        }
        let path = std::env::temp_dir().join(format!("grm-picture-{}.png", std::process::id()));
        write_picture(&path, &p).unwrap();
        let back = read_picture(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!((back.width, back.height, back.channels), (5, 3, 3));
        assert!(back.data.iter().zip(&p.data).all(|(a, b)| (a - b).abs() < 1.0 / 255.0 + 1e-6));
    }
}
