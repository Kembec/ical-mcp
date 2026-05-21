//! Integration tests for the iCalendar parser/builder. We import the source
//! files via `#[path]` so we don't need to expose them as a library crate.

#[path = "../src/ical.rs"]
mod ical;

#[test]
fn test_parse_vevent_basic() {
    let raw = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\n\
               UID:event-1\r\nSUMMARY:Standup\r\nDTSTART:20240301T140000Z\r\n\
               DTEND:20240301T143000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let ev = ical::parse_vevent(raw).unwrap();
    assert_eq!(ev.uid, "event-1");
    assert_eq!(ev.summary, "Standup");
    assert_eq!(ev.start, "20240301T140000Z");
    assert_eq!(ev.end, "20240301T143000Z");
    assert!(!ev.all_day);
}

#[test]
fn test_parse_vevent_all_day() {
    let raw = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:d\r\nSUMMARY:Holiday\r\n\
               DTSTART;VALUE=DATE:20240701\r\nDTEND;VALUE=DATE:20240702\r\n\
               END:VEVENT\r\nEND:VCALENDAR\r\n";
    let ev = ical::parse_vevent(raw).unwrap();
    assert!(ev.all_day);
    assert_eq!(ev.start, "20240701");
    assert_eq!(ev.end, "20240702");
}

#[test]
fn test_build_vevent_round_trips() {
    let built = ical::build_vevent(
        "uid-abc",
        "Coffee",
        "2024-04-05T09:00:00Z",
        "2024-04-05T09:30:00Z",
        Some("with Sam"),
        Some("Cafe Nero"),
        None,
        false,
    )
    .unwrap();
    let ev = ical::parse_vevent(&built).unwrap();
    assert_eq!(ev.uid, "uid-abc");
    assert_eq!(ev.summary, "Coffee");
    assert_eq!(ev.start, "20240405T090000Z");
    assert_eq!(ev.end, "20240405T093000Z");
    assert_eq!(ev.description.as_deref(), Some("with Sam"));
    assert_eq!(ev.location.as_deref(), Some("Cafe Nero"));
    assert!(!ev.all_day);
    // RFC 5545 line endings.
    assert!(built.contains("\r\n"));
}

#[test]
fn test_update_vevent_summary() {
    let original = ical::build_vevent(
        "u-1",
        "Old",
        "2024-04-05T09:00:00Z",
        "2024-04-05T09:30:00Z",
        Some("desc-orig"),
        Some("loc-orig"),
        None,
        false,
    )
    .unwrap();
    let updated =
        ical::update_vevent(&original, Some("New title"), None, None, None, None).unwrap();
    let ev = ical::parse_vevent(&updated).unwrap();
    assert_eq!(ev.summary, "New title");
    assert_eq!(ev.description.as_deref(), Some("desc-orig"));
    assert_eq!(ev.location.as_deref(), Some("loc-orig"));
    assert_eq!(ev.start, "20240405T090000Z");
}

#[test]
fn test_build_vevent_escaping() {
    let built = ical::build_vevent(
        "u-2",
        "Lunch, with Bob; in NYC",
        "2024-04-05T12:00:00Z",
        "2024-04-05T13:00:00Z",
        Some("multi\nline"),
        None,
        None,
        false,
    )
    .unwrap();
    // Raw should contain escaped commas and semicolons.
    assert!(built.contains("Lunch\\, with Bob\\; in NYC"));
    assert!(built.contains("multi\\nline"));

    // Parsing should reverse the escapes.
    let ev = ical::parse_vevent(&built).unwrap();
    assert_eq!(ev.summary, "Lunch, with Bob; in NYC");
    assert_eq!(ev.description.as_deref(), Some("multi\nline"));
}

#[test]
fn test_normalize_datetime_variants() {
    assert_eq!(
        ical::normalize_datetime("2024-04-05T09:00:00Z").unwrap(),
        "20240405T090000Z"
    );
    assert_eq!(
        ical::normalize_datetime("2024-04-05T09:00:00").unwrap(),
        "20240405T090000Z"
    );
    assert_eq!(
        ical::normalize_datetime("20240405T090000Z").unwrap(),
        "20240405T090000Z"
    );
    // Offset is dropped.
    assert!(ical::normalize_datetime("2024-04-05T09:00:00-05:00")
        .unwrap()
        .ends_with('Z'));
}

#[test]
fn test_normalize_date_rejects_garbage() {
    assert!(ical::normalize_date("not-a-date").is_err());
    assert_eq!(ical::normalize_date("2024-04-05").unwrap(), "20240405");
}
