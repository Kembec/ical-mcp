//! Tool definitions and dispatch.
//!
//! Each tool validates its required parameters up front and produces a
//! JSON value describing the result. The MCP layer wraps that value in the
//! standard `content[0].text` response envelope.

use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::caldav::{CalDavClient, Calendar, Event};
use crate::ical;
use crate::mcp::ServerState;
use crate::tools_validation;

/// Static `tools/list` payload.
pub fn tools_list() -> Value {
    json!({
        "tools": [
            {
                "name": "list-calendars",
                "description": "List all calendars in the authenticated iCloud account.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }
            },
            {
                "name": "get-events",
                "description": "List events from a calendar between two dates (inclusive start, exclusive end).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "calendar_id": {
                            "type": "string",
                            "description": "Calendar URL or display name."
                        },
                        "start_date": {
                            "type": "string",
                            "description": "Start date, YYYY-MM-DD."
                        },
                        "end_date": {
                            "type": "string",
                            "description": "End date, YYYY-MM-DD (exclusive)."
                        }
                    },
                    "required": ["calendar_id", "start_date", "end_date"],
                    "additionalProperties": false
                }
            },
            {
                "name": "create-event",
                "description": "Create a new event. Provide start_time/end_time (ISO 8601) for timed events, or start_date/end_date plus all_day=true for all-day events.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "calendar_id":  {"type": "string", "description": "Calendar URL or display name."},
                        "title":        {"type": "string", "description": "Event title (SUMMARY)."},
                        "start_time":   {"type": "string", "description": "Start as ISO 8601 (timed events)."},
                        "end_time":     {"type": "string", "description": "End as ISO 8601 (timed events)."},
                        "start_date":   {"type": "string", "description": "Start date YYYY-MM-DD (all-day events)."},
                        "end_date":     {"type": "string", "description": "End date YYYY-MM-DD, exclusive (all-day events)."},
                        "all_day":      {"type": "boolean", "description": "Set true for an all-day event."},
                        "description":  {"type": "string"},
                        "location":     {"type": "string"},
                        "timezone":     {"type": "string", "description": "IANA tz name (e.g. America/Lima). Omit for UTC."}
                    },
                    "required": ["calendar_id", "title"],
                    "additionalProperties": false
                }
            },
            {
                "name": "update-event",
                "description": "Update fields of an existing event. event_id can be a UID or a full event URL.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "event_id":    {"type": "string"},
                        "calendar_id": {"type": "string", "description": "Required when event_id is a UID (not a URL)."},
                        "title":       {"type": "string"},
                        "start_time":  {"type": "string"},
                        "end_time":    {"type": "string"},
                        "description": {"type": "string"},
                        "location":    {"type": "string"}
                    },
                    "required": ["event_id"],
                    "additionalProperties": false
                }
            },
            {
                "name": "delete-event",
                "description": "Delete an event by UID or full URL.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "event_id":    {"type": "string"},
                        "calendar_id": {"type": "string", "description": "Required when event_id is a UID."}
                    },
                    "required": ["event_id"],
                    "additionalProperties": false
                }
            }
        ]
    })
}

/// Validate a required string parameter is present and non-empty.
pub fn require_str<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow!("missing required parameter: {key}"))
}

fn optional_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str()).filter(|s| !s.is_empty())
}

fn optional_bool(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| v.as_bool())
}

/// Dispatch a tool call.
pub async fn call(state: Arc<ServerState>, name: &str, arguments: Value) -> Result<Value> {
    match name {
        "list-calendars" => list_calendars(&state).await,
        "get-events" => get_events(&state, &arguments).await,
        "create-event" => create_event(&state, &arguments).await,
        "update-event" => update_event(&state, &arguments).await,
        "delete-event" => delete_event(&state, &arguments).await,
        other => Err(anyhow!("unknown tool: {other}")),
    }
}

async fn list_calendars(state: &Arc<ServerState>) -> Result<Value> {
    let client = state.caldav().await?;
    let calendars = client.list_calendars().await?;
    Ok(json!({
        "message": format!("Found {} calendar(s).", calendars.len()),
        "calendars": calendars.iter().map(calendar_to_json).collect::<Vec<_>>()
    }))
}

async fn get_events(state: &Arc<ServerState>, args: &Value) -> Result<Value> {
    tools_validation::validate_get_events(args)?;
    let calendar_id = require_str(args, "calendar_id")?;
    let start_date = require_str(args, "start_date")?;
    let end_date = require_str(args, "end_date")?;

    let client = state.caldav().await?;
    let cal = resolve_calendar(client, calendar_id).await?;
    let start_dt = format!("{}T000000Z", ical::normalize_date(start_date)?);
    let end_dt = format!("{}T000000Z", ical::normalize_date(end_date)?);

    let events = client.get_events(&cal.url, &start_dt, &end_dt).await?;
    Ok(json!({
        "message": format!(
            "Found {} event(s) in `{}` between {} and {}.",
            events.len(), cal.display_name, start_date, end_date
        ),
        "calendar": calendar_to_json(&cal),
        "events": events.iter().map(event_to_json).collect::<Vec<_>>()
    }))
}

async fn create_event(state: &Arc<ServerState>, args: &Value) -> Result<Value> {
    tools_validation::validate_create_event(args)?;
    let calendar_id = require_str(args, "calendar_id")?;
    let title = require_str(args, "title")?;
    let all_day = optional_bool(args, "all_day").unwrap_or(false);

    let (start, end) = if all_day {
        let s = require_str(args, "start_date").context(
            "all-day events require `start_date`",
        )?;
        let e = optional_str(args, "end_date").unwrap_or(s);
        (s.to_string(), e.to_string())
    } else {
        let s = require_str(args, "start_time").context(
            "timed events require `start_time`",
        )?;
        let e = require_str(args, "end_time").context("timed events require `end_time`")?;
        (s.to_string(), e.to_string())
    };

    let description = optional_str(args, "description");
    let location = optional_str(args, "location");
    let timezone = optional_str(args, "timezone");

    let client = state.caldav().await?;
    let cal = resolve_calendar(client, calendar_id).await?;
    let uid = format!("{}@ical-mcp", Uuid::new_v4());
    let ical_data = ical::build_vevent(
        &uid,
        title,
        &start,
        &end,
        description,
        location,
        timezone,
        all_day,
    )?;
    let event_url = client.create_event(&cal.url, &uid, &ical_data).await?;

    Ok(json!({
        "message": format!("Created event `{}` in `{}`.", title, cal.display_name),
        "event": {
            "uid": uid,
            "url": event_url,
            "summary": title,
            "start": start,
            "end": end,
            "all_day": all_day,
        }
    }))
}

async fn update_event(state: &Arc<ServerState>, args: &Value) -> Result<Value> {
    tools_validation::validate_update_event(args)?;
    let event_id = require_str(args, "event_id")?;
    let title = optional_str(args, "title");
    let start = optional_str(args, "start_time");
    let end = optional_str(args, "end_time");
    let description = optional_str(args, "description");
    let location = optional_str(args, "location");

    let client = state.caldav().await?;
    let event_url = resolve_event_url(client, args, event_id).await?;
    let (existing, etag) = client.get_event(&event_url).await?;
    let new_ical = ical::update_vevent(
        &format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//kembec//ical-mcp//EN\r\n\
             BEGIN:VEVENT\r\nUID:{uid}\r\nSUMMARY:{summary}\r\n\
             DTSTART:{start}\r\nDTEND:{end}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
            uid = existing.uid,
            summary = ical::escape_text(&existing.summary),
            start = existing.start,
            end = existing.end,
        ),
        title,
        start,
        end,
        description,
        location,
    )?;
    client.update_event(&event_url, &etag, &new_ical).await?;

    Ok(json!({
        "message": format!("Updated event `{}`.", existing.uid),
        "event": {
            "uid": existing.uid,
            "url": event_url,
        }
    }))
}

async fn delete_event(state: &Arc<ServerState>, args: &Value) -> Result<Value> {
    tools_validation::validate_event_id(args)?;
    let event_id = require_str(args, "event_id")?;
    let client = state.caldav().await?;
    let event_url = resolve_event_url(client, args, event_id).await?;
    let (_ev, etag) = match client.get_event(&event_url).await {
        Ok(v) => v,
        Err(_) => (
            // If the event can't be fetched, attempt an unconditional delete.
            Event {
                uid: event_id.to_string(),
                url: event_url.clone(),
                etag: String::new(),
                summary: String::new(),
                start: String::new(),
                end: String::new(),
                description: None,
                location: None,
                all_day: false,
            },
            String::new(),
        ),
    };
    client.delete_event(&event_url, &etag).await?;
    Ok(json!({
        "message": format!("Deleted event `{}`.", event_id),
        "url": event_url,
    }))
}

async fn resolve_calendar(client: &CalDavClient, id: &str) -> Result<Calendar> {
    if id.starts_with("http://") || id.starts_with("https://") {
        return Ok(Calendar {
            display_name: id.to_string(),
            url: id.to_string(),
            description: None,
        });
    }
    let calendars = client.list_calendars().await?;
    calendars
        .into_iter()
        .find(|c| c.display_name.eq_ignore_ascii_case(id) || c.url == id)
        .ok_or_else(|| anyhow!("no calendar matching `{id}`"))
}

async fn resolve_event_url(
    client: &CalDavClient,
    args: &Value,
    event_id: &str,
) -> Result<String> {
    if event_id.starts_with("http://") || event_id.starts_with("https://") {
        return Ok(event_id.to_string());
    }
    let calendar_id = require_str(args, "calendar_id")
        .context("calendar_id is required when event_id is a UID, not a URL")?;
    let cal = resolve_calendar(client, calendar_id).await?;
    Ok(format!("{}{}.ics", trailing_slash(&cal.url), event_id))
}

fn trailing_slash(s: &str) -> String {
    if s.ends_with('/') {
        s.to_string()
    } else {
        format!("{s}/")
    }
}

fn calendar_to_json(c: &Calendar) -> Value {
    json!({
        "display_name": c.display_name,
        "url": c.url,
        "description": c.description,
    })
}

fn event_to_json(e: &Event) -> Value {
    json!({
        "uid": e.uid,
        "url": e.url,
        "etag": e.etag,
        "summary": e.summary,
        "start": e.start,
        "end": e.end,
        "description": e.description,
        "location": e.location,
        "all_day": e.all_day,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_list_has_five_tools() {
        let v = tools_list();
        let arr = v.get("tools").and_then(|v| v.as_array()).unwrap();
        assert_eq!(arr.len(), 5);
        let names: Vec<&str> = arr
            .iter()
            .map(|t| t.get("name").and_then(|n| n.as_str()).unwrap())
            .collect();
        for expected in [
            "list-calendars",
            "get-events",
            "create-event",
            "update-event",
            "delete-event",
        ] {
            assert!(names.contains(&expected), "missing tool {expected}");
        }
    }

    #[test]
    fn require_str_rejects_missing() {
        assert!(require_str(&json!({}), "x").is_err());
        assert!(require_str(&json!({"x": ""}), "x").is_err());
        assert_eq!(require_str(&json!({"x": "v"}), "x").unwrap(), "v");
    }
}
