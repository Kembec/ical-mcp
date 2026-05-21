//! Minimal iCalendar (RFC 5545) parser and builder.
//!
//! This module intentionally implements just the subset of RFC 5545 needed
//! to round-trip VEVENT objects through CalDAV. It is not a general-purpose
//! iCalendar implementation.

use anyhow::{anyhow, Result};

type ContentLine = Option<(String, Vec<(String, String)>, String)>;

/// A parsed VEVENT.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedEvent {
    pub uid: String,
    pub summary: String,
    pub start: String,
    pub end: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub all_day: bool,
}

const CRLF: &str = "\r\n";

/// Unfold continuation lines per RFC 5545 §3.1 ("folded" lines start with a
/// space or HTAB and concatenate to the previous line).
fn unfold(raw: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in raw.split('\n') {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let first = line.chars().next();
        if matches!(first, Some(' ') | Some('\t')) && !out.is_empty() {
            // Continuation: drop the leading whitespace char and append.
            let mut chars = line.chars();
            chars.next();
            let last = out.last_mut().expect("non-empty by guard");
            last.push_str(chars.as_str());
        } else {
            out.push(line.to_string());
        }
    }
    out
}

/// Fold a logical line to a maximum of 75 octets per RFC 5545 §3.1.
fn fold_line(line: &str) -> String {
    // RFC 5545 specifies octets, but folding on character boundaries is safe
    // because the resulting line stays well under 75 octets for typical
    // calendar content. We fold at 73 chars to keep a safety margin.
    let max = 73;
    if line.len() <= max {
        return line.to_string();
    }
    let mut out = String::new();
    let bytes = line.as_bytes();
    let mut idx = 0;
    let mut first = true;
    while idx < bytes.len() {
        // Advance to char boundary <= idx + max.
        let mut end = (idx + if first { max } else { max - 1 }).min(bytes.len());
        while end < bytes.len() && (bytes[end] & 0b1100_0000) == 0b1000_0000 {
            end -= 1;
        }
        let chunk = std::str::from_utf8(&bytes[idx..end]).unwrap_or("");
        if first {
            out.push_str(chunk);
            first = false;
        } else {
            out.push_str(CRLF);
            out.push(' ');
            out.push_str(chunk);
        }
        idx = end;
    }
    out
}

/// Split a content line into `(name, params, value)` per RFC 5545 §3.1.
fn split_content_line(line: &str) -> ContentLine {
    let colon = line.find(':')?;
    let head = &line[..colon];
    let value = line[colon + 1..].to_string();
    let mut parts = head.split(';');
    let name = parts.next()?.to_string();
    let mut params = Vec::new();
    for p in parts {
        if let Some(eq) = p.find('=') {
            params.push((p[..eq].to_string(), p[eq + 1..].to_string()));
        }
    }
    Some((name.to_ascii_uppercase(), params, value))
}

/// Escape a TEXT value per RFC 5545 §3.3.11.
pub fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            '\r' => {} // CR is dropped per RFC
            _ => out.push(ch),
        }
    }
    out
}

/// Unescape a TEXT value per RFC 5545 §3.3.11.
pub fn unescape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') | Some('N') => out.push('\n'),
                Some(',') => out.push(','),
                Some(';') => out.push(';'),
                Some('\\') => out.push('\\'),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// Parse the first VEVENT block in `ical_data`.
pub fn parse_vevent(ical_data: &str) -> Result<ParsedEvent> {
    let lines = unfold(ical_data);
    let mut inside = false;
    let mut uid = None;
    let mut summary = None;
    let mut start = None;
    let mut end = None;
    let mut description = None;
    let mut location = None;
    let mut all_day = false;

    for line in &lines {
        let upper = line.to_ascii_uppercase();
        if !inside {
            if upper.starts_with("BEGIN:VEVENT") {
                inside = true;
            }
            continue;
        }
        if upper.starts_with("END:VEVENT") {
            break;
        }
        let (name, params, value) = match split_content_line(line) {
            Some(t) => t,
            None => continue,
        };
        match name.as_str() {
            "UID" => uid = Some(value),
            "SUMMARY" => summary = Some(unescape_text(&value)),
            "DESCRIPTION" => description = Some(unescape_text(&value)),
            "LOCATION" => location = Some(unescape_text(&value)),
            "DTSTART" => {
                if params
                    .iter()
                    .any(|(k, v)| k.eq_ignore_ascii_case("VALUE") && v.eq_ignore_ascii_case("DATE"))
                {
                    all_day = true;
                }
                start = Some(value);
            }
            "DTEND" => {
                end = Some(value);
            }
            _ => {}
        }
    }

    if !inside {
        return Err(anyhow!("no VEVENT block found"));
    }

    Ok(ParsedEvent {
        uid: uid.ok_or_else(|| anyhow!("VEVENT missing UID"))?,
        summary: summary.unwrap_or_default(),
        start: start.ok_or_else(|| anyhow!("VEVENT missing DTSTART"))?,
        end: end.unwrap_or_default(),
        description,
        location,
        all_day,
    })
}

/// Format an RFC 5545 UTC timestamp from an ISO 8601 input.
///
/// Accepts `YYYY-MM-DDTHH:MM:SSZ`, `YYYY-MM-DDTHH:MM:SS`, or already-formatted
/// `YYYYMMDDTHHMMSSZ`. Output is `YYYYMMDDTHHMMSSZ`.
pub fn normalize_datetime(input: &str) -> Result<String> {
    let trimmed = input.trim();
    // Already compact form?
    if trimmed.len() >= 15
        && trimmed.chars().take(8).all(|c| c.is_ascii_digit())
        && trimmed.as_bytes().get(8) == Some(&b'T')
    {
        return Ok(trimmed.to_string());
    }
    // ISO 8601 form: strip non-digit chars except T.
    let mut out = String::with_capacity(trimmed.len());
    let mut saw_t = false;
    for ch in trimmed.chars() {
        match ch {
            ':' | '.' | ' ' => {}
            '-' => {
                if saw_t {
                    // Timezone offset like `-05:00` — drop the rest.
                    break;
                }
                // Date separator — drop.
            }
            '+' => {
                if saw_t {
                    break;
                }
            }
            'T' | 't' => {
                if !saw_t {
                    out.push('T');
                    saw_t = true;
                }
            }
            'Z' | 'z' => out.push('Z'),
            _ if ch.is_ascii_digit() => out.push(ch),
            _ => {}
        }
    }
    if !out.contains('T') {
        // Date-only input: pad to start of day.
        out.push_str("T000000");
    }
    if !out.ends_with('Z') {
        out.push('Z');
    }
    Ok(out)
}

/// Convert `YYYY-MM-DD` to `YYYYMMDD` (RFC 5545 DATE value).
pub fn normalize_date(input: &str) -> Result<String> {
    let cleaned: String = input.chars().filter(|c| c.is_ascii_digit()).collect();
    if cleaned.len() != 8 {
        return Err(anyhow!("expected YYYY-MM-DD date, got `{input}`"));
    }
    Ok(cleaned)
}

/// Build a complete VCALENDAR/VEVENT document.
#[allow(clippy::too_many_arguments)]
pub fn build_vevent(
    uid: &str,
    summary: &str,
    start: &str,
    end: &str,
    description: Option<&str>,
    location: Option<&str>,
    timezone: Option<&str>,
    all_day: bool,
) -> Result<String> {
    let mut lines: Vec<String> = Vec::new();
    lines.push("BEGIN:VCALENDAR".to_string());
    lines.push("VERSION:2.0".to_string());
    lines.push("PRODID:-//kembec//ical-mcp//EN".to_string());
    lines.push("CALSCALE:GREGORIAN".to_string());
    lines.push("BEGIN:VEVENT".to_string());
    lines.push(format!("UID:{uid}"));

    let now = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    lines.push(format!("DTSTAMP:{now}"));

    if all_day {
        let s = normalize_date(start)?;
        let e = if end.trim().is_empty() {
            // RFC: DTEND for an all-day event is exclusive — same date works
            // for a one-day event but most clients prefer next day. Caller
            // should pass the correct exclusive end.
            s.clone()
        } else {
            normalize_date(end)?
        };
        lines.push(format!("DTSTART;VALUE=DATE:{s}"));
        lines.push(format!("DTEND;VALUE=DATE:{e}"));
    } else {
        let s = normalize_datetime(start)?;
        let e = normalize_datetime(end)?;
        match timezone {
            Some(tz) if !tz.is_empty() && tz != "UTC" => {
                // For a named timezone, strip the trailing Z because the time
                // is local to the tzid, not UTC.
                let s_local = s.trim_end_matches('Z');
                let e_local = e.trim_end_matches('Z');
                lines.push(format!("DTSTART;TZID={tz}:{s_local}"));
                lines.push(format!("DTEND;TZID={tz}:{e_local}"));
            }
            _ => {
                lines.push(format!("DTSTART:{s}"));
                lines.push(format!("DTEND:{e}"));
            }
        }
    }

    lines.push(format!("SUMMARY:{}", escape_text(summary)));
    if let Some(d) = description {
        if !d.is_empty() {
            lines.push(format!("DESCRIPTION:{}", escape_text(d)));
        }
    }
    if let Some(l) = location {
        if !l.is_empty() {
            lines.push(format!("LOCATION:{}", escape_text(l)));
        }
    }
    lines.push("END:VEVENT".to_string());
    lines.push("END:VCALENDAR".to_string());

    let mut out = String::new();
    for line in lines {
        out.push_str(&fold_line(&line));
        out.push_str(CRLF);
    }
    Ok(out)
}

/// Update selected fields of an existing VEVENT, preserving everything else.
pub fn update_vevent(
    ical_data: &str,
    summary: Option<&str>,
    start: Option<&str>,
    end: Option<&str>,
    description: Option<&str>,
    location: Option<&str>,
) -> Result<String> {
    let lines = unfold(ical_data);
    let mut out: Vec<String> = Vec::new();
    let mut inside_vevent = false;
    let mut have_description = false;
    let mut have_location = false;
    let mut have_summary = false;

    for line in lines {
        let upper = line.to_ascii_uppercase();
        if upper.starts_with("BEGIN:VEVENT") {
            inside_vevent = true;
            out.push(line);
            continue;
        }
        if upper.starts_with("END:VEVENT") {
            // Insert any new fields that didn't previously exist.
            if let Some(s) = summary {
                if !have_summary {
                    out.push(format!("SUMMARY:{}", escape_text(s)));
                }
            }
            if let Some(d) = description {
                if !have_description {
                    out.push(format!("DESCRIPTION:{}", escape_text(d)));
                }
            }
            if let Some(l) = location {
                if !have_location {
                    out.push(format!("LOCATION:{}", escape_text(l)));
                }
            }
            inside_vevent = false;
            out.push(line);
            continue;
        }
        if !inside_vevent {
            out.push(line);
            continue;
        }

        let (name, params, _value) = match split_content_line(&line) {
            Some(t) => t,
            None => {
                out.push(line);
                continue;
            }
        };
        match name.as_str() {
            "SUMMARY" => {
                have_summary = true;
                if let Some(s) = summary {
                    out.push(format!("SUMMARY:{}", escape_text(s)));
                } else {
                    out.push(line);
                }
            }
            "DESCRIPTION" => {
                have_description = true;
                if let Some(d) = description {
                    out.push(format!("DESCRIPTION:{}", escape_text(d)));
                } else {
                    out.push(line);
                }
            }
            "LOCATION" => {
                have_location = true;
                if let Some(l) = location {
                    out.push(format!("LOCATION:{}", escape_text(l)));
                } else {
                    out.push(line);
                }
            }
            "DTSTART" => {
                if let Some(s) = start {
                    let is_date = params.iter().any(|(k, v)| {
                        k.eq_ignore_ascii_case("VALUE") && v.eq_ignore_ascii_case("DATE")
                    });
                    if is_date {
                        out.push(format!("DTSTART;VALUE=DATE:{}", normalize_date(s)?));
                    } else {
                        out.push(format!("DTSTART:{}", normalize_datetime(s)?));
                    }
                } else {
                    out.push(line);
                }
            }
            "DTEND" => {
                if let Some(e) = end {
                    let is_date = params.iter().any(|(k, v)| {
                        k.eq_ignore_ascii_case("VALUE") && v.eq_ignore_ascii_case("DATE")
                    });
                    if is_date {
                        out.push(format!("DTEND;VALUE=DATE:{}", normalize_date(e)?));
                    } else {
                        out.push(format!("DTEND:{}", normalize_datetime(e)?));
                    }
                } else {
                    out.push(line);
                }
            }
            _ => out.push(line),
        }
    }

    let mut result = String::new();
    for line in out {
        result.push_str(&fold_line(&line));
        result.push_str(CRLF);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_vevent() {
        let raw = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\n\
                   UID:abc-123\r\nSUMMARY:Hello\r\n\
                   DTSTART:20240115T130000Z\r\nDTEND:20240115T140000Z\r\n\
                   END:VEVENT\r\nEND:VCALENDAR\r\n";
        let ev = parse_vevent(raw).unwrap();
        assert_eq!(ev.uid, "abc-123");
        assert_eq!(ev.summary, "Hello");
        assert_eq!(ev.start, "20240115T130000Z");
        assert_eq!(ev.all_day, false);
    }

    #[test]
    fn parse_all_day_vevent() {
        let raw = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:x\r\nSUMMARY:Day\r\n\
                   DTSTART;VALUE=DATE:20240115\r\nDTEND;VALUE=DATE:20240116\r\n\
                   END:VEVENT\r\nEND:VCALENDAR\r\n";
        let ev = parse_vevent(raw).unwrap();
        assert!(ev.all_day);
        assert_eq!(ev.start, "20240115");
    }

    #[test]
    fn escape_handles_special_chars() {
        assert_eq!(escape_text("a,b;c\nd\\e"), "a\\,b\\;c\\nd\\\\e");
    }

    #[test]
    fn round_trip_build_and_parse() {
        let ical = build_vevent(
            "uid-1",
            "Lunch, with Bob; in NYC",
            "2024-01-15T13:00:00Z",
            "2024-01-15T14:00:00Z",
            Some("Notes\nline2"),
            Some("Office"),
            None,
            false,
        )
        .unwrap();
        let ev = parse_vevent(&ical).unwrap();
        assert_eq!(ev.uid, "uid-1");
        assert_eq!(ev.summary, "Lunch, with Bob; in NYC");
        assert_eq!(ev.description.as_deref(), Some("Notes\nline2"));
        assert_eq!(ev.location.as_deref(), Some("Office"));
        assert_eq!(ev.start, "20240115T130000Z");
    }

    #[test]
    fn update_changes_summary_only() {
        let original = build_vevent(
            "u",
            "Old",
            "2024-01-15T13:00:00Z",
            "2024-01-15T14:00:00Z",
            Some("desc"),
            None,
            None,
            false,
        )
        .unwrap();
        let updated = update_vevent(&original, Some("New"), None, None, None, None).unwrap();
        let ev = parse_vevent(&updated).unwrap();
        assert_eq!(ev.summary, "New");
        assert_eq!(ev.description.as_deref(), Some("desc"));
        assert_eq!(ev.start, "20240115T130000Z");
    }

    #[test]
    fn normalize_dt_handles_iso() {
        assert_eq!(
            normalize_datetime("2024-01-15T13:00:00Z").unwrap(),
            "20240115T130000Z"
        );
        assert_eq!(
            normalize_datetime("2024-01-15T13:00:00").unwrap(),
            "20240115T130000Z"
        );
    }

    #[test]
    fn normalize_date_strips_dashes() {
        assert_eq!(normalize_date("2024-01-15").unwrap(), "20240115");
    }
}
