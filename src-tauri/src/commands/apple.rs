//! Apple Calendar + Reminders via iCloud CalDAV.
//!
//! iCloud exposes both calendars (VEVENT) and reminder lists (VTODO) over
//! CalDAV. We authenticate with the user's Apple ID and an **app-specific
//! password** (generated at appleid.apple.com — a normal password won't work
//! because iCloud requires 2FA), both stored in the app's local settings.json
//! like the other credentials, never in the repo.
//!
//! Flow: PROPFIND for the principal → PROPFIND for the calendar-home-set →
//! PROPFIND (Depth 1) to enumerate the collections → REPORT calendar-query on
//! each to pull the actual events / to-dos, whose iCalendar bodies we parse.

use std::path::PathBuf;
use std::time::Duration;

use chrono::{
    Datelike, Duration as ChronoDuration, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc,
    Weekday,
};
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use serde::Serialize;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const CALDAV_ROOT: &str = "https://caldav.icloud.com";
/// How far ahead the schedule looks, and a small look-back so events earlier
/// today still show.
const LOOKBACK_HOURS: i64 = 6;
const LOOKAHEAD_DAYS: i64 = 21;
const MAX_EVENTS: usize = 40;
const MAX_REMINDERS: usize = 60;

#[derive(Debug, Clone, Serialize)]
pub struct AppleEvent {
    pub summary: String,
    /// RFC3339 (UTC) for timed events, or "YYYY-MM-DD" for all-day ones.
    pub start: String,
    pub end: Option<String>,
    pub all_day: bool,
    pub calendar: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppleReminder {
    pub summary: String,
    pub due: Option<String>,
    pub calendar: String,
}

fn creds() -> Result<(String, String), String> {
    let home = std::env::var("HOME").map_err(|_| "Apple account is not configured".to_string())?;
    let path = PathBuf::from(home).join(".local/share/com.jarvis.assistant/settings.json");
    let raw = std::fs::read_to_string(path)
        .map_err(|_| "Apple account is not configured — add it in Settings.".to_string())?;
    let json: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("Could not read settings: {e}"))?;
    let id = json
        .get("appleId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let pass = json
        .get("appleAppPassword")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if id.is_empty() || pass.is_empty() {
        return Err(
            "Apple account is not configured — add your Apple ID and an app-specific \
             password (from appleid.apple.com) in Settings → Apple."
                .to_string(),
        );
    }
    Ok((id, pass))
}

struct Dav {
    client: reqwest::Client,
    user: String,
    pass: String,
}

impl Dav {
    fn new(user: String, pass: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("failed to build CalDAV HTTP client");
        Dav { client, user, pass }
    }

    async fn dav_request(
        &self,
        method: &str,
        url: &str,
        depth: &str,
        body: String,
    ) -> Result<String, String> {
        let m = reqwest::Method::from_bytes(method.as_bytes())
            .map_err(|e| format!("bad method: {e}"))?;
        let resp = self
            .client
            .request(m, url)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Depth", depth)
            .header("Content-Type", "application/xml; charset=utf-8")
            .body(body)
            .send()
            .await
            .map_err(|e| format!("iCloud unreachable: {e}"))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err("iCloud rejected the credentials (401). Check the Apple ID and that \
                        the app-specific password is current."
                .to_string());
        }
        // CalDAV success is 207 Multi-Status (or 200).
        if !status.is_success() {
            return Err(format!("iCloud CalDAV error {status}: {}", text.chars().take(300).collect::<String>()));
        }
        Ok(text)
    }

    async fn propfind(&self, url: &str, depth: &str, body: String) -> Result<String, String> {
        self.dav_request("PROPFIND", url, depth, body).await
    }

    async fn report(&self, url: &str, body: String) -> Result<String, String> {
        self.dav_request("REPORT", url, "1", body).await
    }

    async fn principal_url(&self) -> Result<String, String> {
        let body = r#"<?xml version="1.0" encoding="utf-8"?>
<A:propfind xmlns:A="DAV:"><A:prop><A:current-user-principal/></A:prop></A:propfind>"#;
        let xml = self.propfind(CALDAV_ROOT, "0", body.to_string()).await?;
        let href = first_href_under(&xml, "current-user-principal")
            .ok_or_else(|| "Could not locate the iCloud principal.".to_string())?;
        abs_url(CALDAV_ROOT, &href)
    }

    async fn calendar_home(&self, principal: &str) -> Result<String, String> {
        let body = r#"<?xml version="1.0" encoding="utf-8"?>
<A:propfind xmlns:A="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav"><A:prop><C:calendar-home-set/></A:prop></A:propfind>"#;
        let xml = self.propfind(principal, "0", body.to_string()).await?;
        let href = first_href_under(&xml, "calendar-home-set")
            .ok_or_else(|| "Could not locate the iCloud calendar home.".to_string())?;
        abs_url(principal, &href)
    }

    async fn list_collections(&self, home: &str) -> Result<Vec<CalCollection>, String> {
        let body = r#"<?xml version="1.0" encoding="utf-8"?>
<A:propfind xmlns:A="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <A:prop>
    <A:displayname/>
    <A:resourcetype/>
    <C:supported-calendar-component-set/>
  </A:prop>
</A:propfind>"#;
        let xml = self.propfind(home, "1", body.to_string()).await?;
        Ok(parse_collections(&xml))
    }
}

#[derive(Default, Debug)]
struct CalCollection {
    href: String,
    display: String,
    is_calendar: bool,
    comps: Vec<String>,
}

impl CalCollection {
    fn supports_events(&self) -> bool {
        self.is_calendar && (self.comps.is_empty() || self.comps.iter().any(|c| c == "VEVENT"))
    }
    fn supports_todos(&self) -> bool {
        self.is_calendar && self.comps.iter().any(|c| c == "VTODO")
    }
}

// ---- XML helpers (namespace-prefix agnostic — matches on local names) ----

fn local_of(e: &BytesStart) -> String {
    String::from_utf8_lossy(e.local_name().as_ref()).to_string()
}

fn attr_value(e: &BytesStart, key: &str) -> Option<String> {
    for a in e.attributes().flatten() {
        if String::from_utf8_lossy(a.key.local_name().as_ref()) == key {
            return a.unescape_value().ok().map(|v| v.into_owned());
        }
    }
    None
}

/// Returns the text of the first <href> found within the subtree of the first
/// element whose local name matches `target`.
fn first_href_under(xml: &str, target: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    let mut depth = 0i32;
    let mut target_depth: Option<i32> = None;
    let mut text = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                depth += 1;
                if local_of(&e) == target && target_depth.is_none() {
                    target_depth = Some(depth);
                }
                text.clear();
            }
            Ok(Event::Text(e)) => {
                text.push_str(&e.unescape().unwrap_or_default());
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if name == "href" {
                    if let Some(td) = target_depth {
                        if depth >= td {
                            return Some(text.trim().to_string());
                        }
                    }
                }
                if let Some(td) = target_depth {
                    if depth == td && name == target {
                        target_depth = None;
                    }
                }
                depth -= 1;
                text.clear();
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    None
}

/// Parses a Depth-1 multistatus into calendar collections.
fn parse_collections(xml: &str) -> Vec<CalCollection> {
    let mut reader = Reader::from_str(xml);
    let mut out: Vec<CalCollection> = Vec::new();
    let mut cur: Option<CalCollection> = None;
    let mut text = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                match local_of(&e).as_str() {
                    "response" => cur = Some(CalCollection::default()),
                    "calendar" => {
                        if let Some(c) = cur.as_mut() {
                            c.is_calendar = true;
                        }
                    }
                    "comp" => {
                        if let (Some(c), Some(n)) = (cur.as_mut(), attr_value(&e, "name")) {
                            c.comps.push(n);
                        }
                    }
                    _ => {}
                }
                text.clear();
            }
            Ok(Event::Empty(e)) => match local_of(&e).as_str() {
                "calendar" => {
                    if let Some(c) = cur.as_mut() {
                        c.is_calendar = true;
                    }
                }
                "comp" => {
                    if let (Some(c), Some(n)) = (cur.as_mut(), attr_value(&e, "name")) {
                        c.comps.push(n);
                    }
                }
                _ => {}
            },
            Ok(Event::Text(e)) => text.push_str(&e.unescape().unwrap_or_default()),
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                let t = text.trim().to_string();
                match name.as_str() {
                    "href" => {
                        if let Some(c) = cur.as_mut() {
                            if c.href.is_empty() {
                                c.href = t;
                            }
                        }
                    }
                    "displayname" => {
                        if let Some(c) = cur.as_mut() {
                            if !t.is_empty() {
                                c.display = t;
                            }
                        }
                    }
                    "response" => {
                        if let Some(c) = cur.take() {
                            out.push(c);
                        }
                    }
                    _ => {}
                }
                text.clear();
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

/// Collects the text of every <calendar-data> element in a REPORT response.
fn extract_calendar_data(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    let mut out = Vec::new();
    let mut capturing = false;
    let mut buf = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if local_of(&e) == "calendar-data" {
                    capturing = true;
                    buf.clear();
                }
            }
            Ok(Event::Text(e)) if capturing => {
                buf.push_str(&e.unescape().unwrap_or_default());
            }
            Ok(Event::CData(e)) if capturing => {
                buf.push_str(&String::from_utf8_lossy(e.as_ref()));
            }
            Ok(Event::End(e)) => {
                if String::from_utf8_lossy(e.local_name().as_ref()) == "calendar-data" {
                    capturing = false;
                    if !buf.trim().is_empty() {
                        out.push(buf.clone());
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

fn abs_url(base: &str, href: &str) -> Result<String, String> {
    let base_url = reqwest::Url::parse(base).map_err(|e| format!("bad base url: {e}"))?;
    base_url
        .join(href.trim())
        .map(|u| u.to_string())
        .map_err(|e| format!("bad href {href}: {e}"))
}

// ---- iCalendar (RFC 5545) minimal parsing ----

fn unfold(ics: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in ics.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if (line.starts_with(' ') || line.starts_with('\t')) && !out.is_empty() {
            out.last_mut().unwrap().push_str(&line[1..]);
        } else {
            out.push(line.to_string());
        }
    }
    out
}

/// Splits a property line into (UPPERCASE name, full head incl. params, value).
fn split_prop(line: &str) -> Option<(String, String, String)> {
    let colon = line.find(':')?;
    let head = &line[..colon];
    let value = &line[colon + 1..];
    let name = head.split(';').next().unwrap_or(head).to_uppercase();
    Some((name, head.to_string(), value.to_string()))
}

fn unescape_text(v: &str) -> String {
    v.replace("\\n", "\n")
        .replace("\\N", "\n")
        .replace("\\,", ",")
        .replace("\\;", ";")
        .replace("\\\\", "\\")
}

/// Returns (normalized value, all_day).
fn parse_dt(head: &str, value: &str) -> (String, bool) {
    let is_date = head.to_uppercase().contains("VALUE=DATE")
        || (value.len() == 8 && value.chars().all(|c| c.is_ascii_digit()));
    if is_date && value.len() >= 8 {
        return (
            format!("{}-{}-{}", &value[0..4], &value[4..6], &value[6..8]),
            true,
        );
    }
    if value.ends_with('Z') {
        if let Ok(dt) = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%SZ") {
            return (Utc.from_utc_datetime(&dt).to_rfc3339(), false);
        }
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S") {
        // Floating or TZID-qualified: emit without a zone so the frontend
        // renders it in local time.
        return (dt.format("%Y-%m-%dT%H:%M:%S").to_string(), false);
    }
    (value.to_string(), false)
}

/// Parses VEVENTs out of an iCalendar body. Shared with the Google Calendar
/// integration (which fetches the same iCalendar format from a secret feed URL).
pub fn parse_ics_events(ics: &str, calendar: &str) -> Vec<AppleEvent> {
    let mut out = Vec::new();
    let mut in_ev = false;
    let mut summary = String::new();
    let mut start: Option<String> = None;
    let mut end: Option<String> = None;
    let mut all_day = false;
    for line in unfold(ics) {
        let u = line.to_uppercase();
        if u.starts_with("BEGIN:VEVENT") {
            in_ev = true;
            summary = String::new();
            start = None;
            end = None;
            all_day = false;
            continue;
        }
        if u.starts_with("END:VEVENT") {
            if in_ev {
                if let Some(s) = start.take() {
                    out.push(AppleEvent {
                        summary: if summary.is_empty() {
                            "(no title)".to_string()
                        } else {
                            summary.clone()
                        },
                        start: s,
                        end: end.take(),
                        all_day,
                        calendar: calendar.to_string(),
                    });
                }
            }
            in_ev = false;
            continue;
        }
        if !in_ev {
            continue;
        }
        if let Some((name, head, value)) = split_prop(&line) {
            match name.as_str() {
                "SUMMARY" => summary = unescape_text(&value),
                "DTSTART" => {
                    let (s, ad) = parse_dt(&head, &value);
                    start = Some(s);
                    all_day = ad;
                }
                "DTEND" => {
                    let (e, _) = parse_dt(&head, &value);
                    end = Some(e);
                }
                _ => {}
            }
        }
    }
    out
}

// ---- Recurring-event (RRULE) expansion ----
//
// The iCalendar format stores a repeating event once, at its series start, with
// an RRULE describing the repetition — so to show "what's on my schedule" we
// have to generate the concrete upcoming occurrences ourselves. This handles
// the common cases (DAILY / WEEKLY+BYDAY / MONTHLY / YEARLY, with INTERVAL,
// COUNT, UNTIL and EXDATE); anything more exotic falls back to the series start.

#[derive(Clone)]
struct RawEvent {
    summary: String,
    all_day: bool,
    is_utc: bool,
    has_start: bool,
    start_date: NaiveDate,
    start_time: Option<NaiveTime>,
    rrule: Option<String>,
    exdates: Vec<NaiveDate>,
}

fn parse_start(head: &str, value: &str) -> Option<(NaiveDate, Option<NaiveTime>, bool, bool)> {
    let is_date = head.to_uppercase().contains("VALUE=DATE")
        || (value.len() == 8 && value.chars().all(|c| c.is_ascii_digit()));
    if is_date && value.len() >= 8 {
        let d = NaiveDate::parse_from_str(&value[..8], "%Y%m%d").ok()?;
        return Some((d, None, false, true));
    }
    let is_utc = value.ends_with('Z');
    let core = value.trim_end_matches('Z');
    let dt = NaiveDateTime::parse_from_str(core, "%Y%m%dT%H%M%S").ok()?;
    Some((dt.date(), Some(dt.time()), is_utc, false))
}

fn occ_start_string(ev: &RawEvent, date: NaiveDate) -> String {
    match ev.start_time {
        Some(t) if !ev.all_day => {
            let ndt = NaiveDateTime::new(date, t);
            if ev.is_utc {
                Utc.from_utc_datetime(&ndt).to_rfc3339()
            } else {
                ndt.format("%Y-%m-%dT%H:%M:%S").to_string()
            }
        }
        _ => date.format("%Y-%m-%d").to_string(),
    }
}

fn weekday_from(code: &str) -> Option<Weekday> {
    let c = code.trim().to_uppercase();
    if c.ends_with("MO") {
        Some(Weekday::Mon)
    } else if c.ends_with("TU") {
        Some(Weekday::Tue)
    } else if c.ends_with("WE") {
        Some(Weekday::Wed)
    } else if c.ends_with("TH") {
        Some(Weekday::Thu)
    } else if c.ends_with("FR") {
        Some(Weekday::Fri)
    } else if c.ends_with("SA") {
        Some(Weekday::Sat)
    } else if c.ends_with("SU") {
        Some(Weekday::Sun)
    } else {
        None
    }
}

fn rrule_param<'a>(rrule: &'a str, key: &str) -> Option<&'a str> {
    for part in rrule.split(';') {
        let mut kv = part.splitn(2, '=');
        if kv.next().map(|k| k.eq_ignore_ascii_case(key)).unwrap_or(false) {
            return kv.next();
        }
    }
    None
}

fn add_months(d: NaiveDate, months: i64) -> NaiveDate {
    let total = (d.year() as i64) * 12 + (d.month() as i64 - 1) + months;
    let y = total.div_euclid(12) as i32;
    let m = (total.rem_euclid(12) + 1) as u32;
    let last = (28..=31)
        .rev()
        .find(|&day| NaiveDate::from_ymd_opt(y, m, day).is_some())
        .unwrap_or(28);
    NaiveDate::from_ymd_opt(y, m, d.day().min(last)).unwrap_or(d)
}

/// Records an occurrence toward the COUNT limit and, if it lands in the window
/// and isn't excluded, into `out`. Returns false when COUNT is exhausted.
fn push_occ(
    date: NaiveDate,
    count: Option<i64>,
    from: NaiveDate,
    until: NaiveDate,
    exdates: &[NaiveDate],
    out: &mut Vec<NaiveDate>,
    generated: &mut i64,
) -> bool {
    if let Some(c) = count {
        if *generated >= c {
            return false;
        }
    }
    *generated += 1;
    if date >= from && date <= until && !exdates.contains(&date) {
        out.push(date);
    }
    true
}

fn expand_occurrences(ev: &RawEvent, from: NaiveDate, until: NaiveDate) -> Vec<NaiveDate> {
    let start = ev.start_date;
    let rrule = match &ev.rrule {
        None => {
            return if start >= from && start <= until {
                vec![start]
            } else {
                vec![]
            };
        }
        Some(r) => r,
    };
    let freq = rrule_param(rrule, "FREQ").unwrap_or("").to_uppercase();
    let interval: i64 = rrule_param(rrule, "INTERVAL")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
        .max(1);
    let count: Option<i64> = rrule_param(rrule, "COUNT").and_then(|v| v.parse().ok());
    let r_until: Option<NaiveDate> = rrule_param(rrule, "UNTIL").and_then(|v| {
        let v = v.trim();
        if v.len() >= 8 {
            NaiveDate::parse_from_str(&v[..8], "%Y%m%d").ok()
        } else {
            None
        }
    });
    let bydays: Vec<Weekday> = rrule_param(rrule, "BYDAY")
        .map(|v| v.split(',').filter_map(weekday_from).collect())
        .unwrap_or_default();
    let hard_until = r_until.map(|u| u.min(until)).unwrap_or(until);

    let mut out = Vec::new();
    let mut generated: i64 = 0;
    let max_iter = 3000;
    let mut iter = 0;

    match freq.as_str() {
        "DAILY" => {
            let mut d = start;
            while d <= hard_until && iter < max_iter {
                if !push_occ(d, count, from, hard_until, &ev.exdates, &mut out, &mut generated) {
                    break;
                }
                d += ChronoDuration::days(interval);
                iter += 1;
            }
        }
        "WEEKLY" if bydays.is_empty() => {
            let mut d = start;
            while d <= hard_until && iter < max_iter {
                if !push_occ(d, count, from, hard_until, &ev.exdates, &mut out, &mut generated) {
                    break;
                }
                d += ChronoDuration::weeks(interval);
                iter += 1;
            }
        }
        "WEEKLY" => {
            let mut sorted = bydays.clone();
            sorted.sort_by_key(|w| w.num_days_from_monday());
            let week0 = start - ChronoDuration::days(start.weekday().num_days_from_monday() as i64);
            let mut k: i64 = 0;
            'weeks: while iter < max_iter {
                let wk = week0 + ChronoDuration::weeks(interval * k);
                if wk > hard_until {
                    break;
                }
                for wd in &sorted {
                    let d = wk + ChronoDuration::days(wd.num_days_from_monday() as i64);
                    if d < start {
                        continue;
                    }
                    if d > hard_until {
                        break 'weeks;
                    }
                    if !push_occ(d, count, from, hard_until, &ev.exdates, &mut out, &mut generated) {
                        break 'weeks;
                    }
                }
                k += 1;
                iter += 1;
            }
        }
        "MONTHLY" => {
            let mut k: i64 = 0;
            while iter < max_iter {
                let d = add_months(start, interval * k);
                if d > hard_until {
                    break;
                }
                if !push_occ(d, count, from, hard_until, &ev.exdates, &mut out, &mut generated) {
                    break;
                }
                k += 1;
                iter += 1;
            }
        }
        "YEARLY" => {
            let mut k: i64 = 0;
            while iter < max_iter {
                let d = add_months(start, 12 * interval * k);
                if d > hard_until {
                    break;
                }
                if !push_occ(d, count, from, hard_until, &ev.exdates, &mut out, &mut generated) {
                    break;
                }
                k += 1;
                iter += 1;
            }
        }
        _ => {
            if start >= from && start <= until {
                out.push(start);
            }
        }
    }
    out
}

/// Parses VEVENTs and expands recurrences into concrete occurrences that fall
/// within [from, until]. Shared by the Apple and Google calendar paths.
pub fn expand_ics_events(
    ics: &str,
    calendar: &str,
    from: NaiveDate,
    until: NaiveDate,
) -> Vec<AppleEvent> {
    let mut raws: Vec<RawEvent> = Vec::new();
    let mut cur: Option<RawEvent> = None;
    for line in unfold(ics) {
        let u = line.to_uppercase();
        if u.starts_with("BEGIN:VEVENT") {
            cur = Some(RawEvent {
                summary: String::new(),
                all_day: false,
                is_utc: false,
                has_start: false,
                start_date: NaiveDate::from_ymd_opt(1970, 1, 1).unwrap(),
                start_time: None,
                rrule: None,
                exdates: Vec::new(),
            });
            continue;
        }
        if u.starts_with("END:VEVENT") {
            if let Some(e) = cur.take() {
                if e.has_start {
                    raws.push(e);
                }
            }
            continue;
        }
        let ev = match cur.as_mut() {
            Some(e) => e,
            None => continue,
        };
        if let Some((name, head, value)) = split_prop(&line) {
            match name.as_str() {
                "SUMMARY" => ev.summary = unescape_text(&value),
                "DTSTART" => {
                    if let Some((d, t, isutc, allday)) = parse_start(&head, &value) {
                        ev.start_date = d;
                        ev.start_time = t;
                        ev.is_utc = isutc;
                        ev.all_day = allday;
                        ev.has_start = true;
                    }
                }
                "RRULE" => ev.rrule = Some(value.trim().to_string()),
                "EXDATE" => {
                    for v in value.split(',') {
                        let v = v.trim();
                        if v.len() >= 8 {
                            if let Ok(d) = NaiveDate::parse_from_str(&v[..8], "%Y%m%d") {
                                ev.exdates.push(d);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let mut out = Vec::new();
    for ev in &raws {
        for date in expand_occurrences(ev, from, until) {
            out.push(AppleEvent {
                summary: if ev.summary.is_empty() {
                    "(no title)".to_string()
                } else {
                    ev.summary.clone()
                },
                start: occ_start_string(ev, date),
                end: None,
                all_day: ev.all_day,
                calendar: calendar.to_string(),
            });
        }
    }
    out
}

fn parse_ics_todos(ics: &str, calendar: &str) -> Vec<AppleReminder> {
    let mut out = Vec::new();
    let mut in_todo = false;
    let mut summary = String::new();
    let mut due: Option<String> = None;
    let mut completed = false;
    for line in unfold(ics) {
        let u = line.to_uppercase();
        if u.starts_with("BEGIN:VTODO") {
            in_todo = true;
            summary = String::new();
            due = None;
            completed = false;
            continue;
        }
        if u.starts_with("END:VTODO") {
            if in_todo && !completed && !summary.is_empty() {
                out.push(AppleReminder {
                    summary: summary.clone(),
                    due: due.take(),
                    calendar: calendar.to_string(),
                });
            }
            in_todo = false;
            continue;
        }
        if !in_todo {
            continue;
        }
        if let Some((name, head, value)) = split_prop(&line) {
            match name.as_str() {
                "SUMMARY" => summary = unescape_text(&value),
                "DUE" => {
                    let (d, _) = parse_dt(&head, &value);
                    due = Some(d);
                }
                "STATUS" => {
                    if value.trim().eq_ignore_ascii_case("COMPLETED") {
                        completed = true;
                    }
                }
                "COMPLETED" => completed = true,
                _ => {}
            }
        }
    }
    out
}

fn events_query_body(start: &str, end: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<C:calendar-query xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop><C:calendar-data/></D:prop>
  <C:filter><C:comp-filter name="VCALENDAR">
    <C:comp-filter name="VEVENT">
      <C:time-range start="{start}" end="{end}"/>
    </C:comp-filter>
  </C:comp-filter></C:filter>
</C:calendar-query>"#
    )
}

fn todos_query_body() -> String {
    r#"<?xml version="1.0" encoding="utf-8"?>
<C:calendar-query xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop><C:calendar-data/></D:prop>
  <C:filter><C:comp-filter name="VCALENDAR">
    <C:comp-filter name="VTODO">
      <C:prop-filter name="COMPLETED"><C:is-not-defined/></C:prop-filter>
    </C:comp-filter>
  </C:comp-filter></C:filter>
</C:calendar-query>"#
        .to_string()
}

pub async fn list_events_impl() -> Result<Vec<AppleEvent>, String> {
    let (user, pass) = creds()?;
    let dav = Dav::new(user, pass);
    let principal = dav.principal_url().await?;
    let home = dav.calendar_home(&principal).await?;
    let collections = dav.list_collections(&home).await?;

    let now = Utc::now();
    let start = (now - ChronoDuration::hours(LOOKBACK_HOURS))
        .format("%Y%m%dT%H%M%SZ")
        .to_string();
    let end = (now + ChronoDuration::days(LOOKAHEAD_DAYS))
        .format("%Y%m%dT%H%M%SZ")
        .to_string();
    let body = events_query_body(&start, &end);

    let today = now.date_naive();
    let from_date = today - ChronoDuration::days(1);
    let until_date = today + ChronoDuration::days(LOOKAHEAD_DAYS);

    let mut events: Vec<AppleEvent> = Vec::new();
    for c in collections.iter().filter(|c| c.supports_events()) {
        let url = match abs_url(&home, &c.href) {
            Ok(u) => u,
            Err(_) => continue,
        };
        let xml = match dav.report(&url, body.clone()).await {
            Ok(x) => x,
            Err(_) => continue, // one bad calendar shouldn't sink the rest
        };
        for ics in extract_calendar_data(&xml) {
            // Expand recurrences so repeating events show their upcoming
            // occurrences, not just the series start.
            events.extend(expand_ics_events(&ics, &c.display, from_date, until_date));
        }
    }
    events.sort_by(|a, b| a.start.cmp(&b.start));
    events.truncate(MAX_EVENTS);
    Ok(events)
}

pub async fn list_reminders_impl() -> Result<Vec<AppleReminder>, String> {
    let (user, pass) = creds()?;
    let dav = Dav::new(user, pass);
    let principal = dav.principal_url().await?;
    let home = dav.calendar_home(&principal).await?;
    let collections = dav.list_collections(&home).await?;
    let body = todos_query_body();

    let mut reminders: Vec<AppleReminder> = Vec::new();
    for c in collections.iter().filter(|c| c.supports_todos()) {
        let url = match abs_url(&home, &c.href) {
            Ok(u) => u,
            Err(_) => continue,
        };
        let xml = match dav.report(&url, body.clone()).await {
            Ok(x) => x,
            Err(_) => continue,
        };
        for ics in extract_calendar_data(&xml) {
            reminders.extend(parse_ics_todos(&ics, &c.display));
        }
    }
    // Sort: items with a due date first (earliest), then undated.
    reminders.sort_by(|a, b| match (&a.due, &b.due) {
        (Some(x), Some(y)) => x.cmp(y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.summary.cmp(&b.summary),
    });
    reminders.truncate(MAX_REMINDERS);
    Ok(reminders)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn list_apple_events() -> Result<Vec<AppleEvent>, String> {
    list_events_impl().await
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn list_apple_reminders() -> Result<Vec<AppleReminder>, String> {
    list_reminders_impl().await
}
