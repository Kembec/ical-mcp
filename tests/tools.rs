//! Tool-level parameter validation tests. We exercise the validation
//! helpers directly to avoid needing live CalDAV access.

#[path = "../src/ical.rs"]
mod ical;

use serde_json::json;

#[path = "../src/tools_validation.rs"]
mod tools_validation;

#[test]
fn test_get_events_requires_calendar_id() {
    let err = tools_validation::validate_get_events(&json!({
        "start_date": "2024-01-01",
        "end_date": "2024-01-02"
    }))
    .unwrap_err();
    assert!(format!("{err}").contains("calendar_id"));
}

#[test]
fn test_get_events_requires_dates() {
    let err = tools_validation::validate_get_events(&json!({"calendar_id": "Home"})).unwrap_err();
    assert!(format!("{err}").contains("start_date"));
}

#[test]
fn test_create_event_requires_title() {
    let err = tools_validation::validate_create_event(&json!({
        "calendar_id": "Home",
        "start_time": "2024-01-01T10:00:00Z",
        "end_time": "2024-01-01T11:00:00Z"
    }))
    .unwrap_err();
    assert!(format!("{err}").contains("title"));
}

#[test]
fn test_create_event_timed_requires_start_end() {
    let err = tools_validation::validate_create_event(&json!({
        "calendar_id": "Home",
        "title": "Run"
    }))
    .unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("start_time") || msg.contains("end_time"));
}

#[test]
fn test_create_event_all_day_requires_start_date() {
    let err = tools_validation::validate_create_event(&json!({
        "calendar_id": "Home",
        "title": "Vacation",
        "all_day": true
    }))
    .unwrap_err();
    assert!(format!("{err}").contains("start_date"));
}

#[test]
fn test_create_event_all_day_ok() {
    tools_validation::validate_create_event(&json!({
        "calendar_id": "Home",
        "title": "Vacation",
        "all_day": true,
        "start_date": "2024-08-01",
        "end_date": "2024-08-08"
    }))
    .unwrap();
}

#[test]
fn test_create_event_timed_ok() {
    tools_validation::validate_create_event(&json!({
        "calendar_id": "Home",
        "title": "Run",
        "start_time": "2024-01-01T10:00:00Z",
        "end_time": "2024-01-01T11:00:00Z"
    }))
    .unwrap();
}

#[test]
fn test_delete_event_requires_event_id() {
    let err = tools_validation::validate_event_id(&json!({})).unwrap_err();
    assert!(format!("{err}").contains("event_id"));
}

#[test]
fn test_update_event_requires_event_id() {
    let err = tools_validation::validate_update_event(&json!({"title": "x"})).unwrap_err();
    assert!(format!("{err}").contains("event_id"));
}

#[test]
fn test_update_event_requires_some_field() {
    let err = tools_validation::validate_update_event(&json!({"event_id": "u-1"})).unwrap_err();
    assert!(format!("{err}").contains("at least one"));
}

#[test]
fn test_update_event_ok_with_title() {
    tools_validation::validate_update_event(&json!({"event_id": "u-1", "title": "New"})).unwrap();
}
