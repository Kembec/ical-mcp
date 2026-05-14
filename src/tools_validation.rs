//! Pure parameter-validation helpers for each tool.
//!
//! These are split out from `tools.rs` so they can be unit-tested without
//! spinning up a CalDAV client or a tokio runtime.

use anyhow::{anyhow, Result};
use serde_json::Value;

fn require_str<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow!("missing required parameter: {key}"))
}

fn optional_bool(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| v.as_bool())
}

/// Validate `get-events` arguments.
pub fn validate_get_events(args: &Value) -> Result<()> {
    require_str(args, "calendar_id")?;
    require_str(args, "start_date")?;
    require_str(args, "end_date")?;
    Ok(())
}

/// Validate `create-event` arguments.
pub fn validate_create_event(args: &Value) -> Result<()> {
    require_str(args, "calendar_id")?;
    require_str(args, "title")?;
    let all_day = optional_bool(args, "all_day").unwrap_or(false);
    if all_day {
        require_str(args, "start_date")
            .map_err(|_| anyhow!("all-day events require start_date"))?;
    } else {
        require_str(args, "start_time")
            .map_err(|_| anyhow!("timed events require start_time"))?;
        require_str(args, "end_time")
            .map_err(|_| anyhow!("timed events require end_time"))?;
    }
    Ok(())
}

/// Validate that `event_id` is present (used by delete-event).
pub fn validate_event_id(args: &Value) -> Result<()> {
    require_str(args, "event_id")?;
    Ok(())
}

/// Validate `update-event` arguments — requires event_id plus at least one
/// updatable field.
pub fn validate_update_event(args: &Value) -> Result<()> {
    require_str(args, "event_id")?;
    let has_any = ["title", "start_time", "end_time", "description", "location"]
        .iter()
        .any(|k| args.get(*k).and_then(|v| v.as_str()).is_some());
    if !has_any {
        return Err(anyhow!(
            "update-event requires at least one field to change (title, start_time, end_time, description, location)"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn create_event_validates_branches() {
        assert!(validate_create_event(&json!({})).is_err());
        assert!(validate_create_event(&json!({
            "calendar_id": "c", "title": "t",
            "start_time": "2024-01-01T10:00:00Z",
            "end_time":   "2024-01-01T11:00:00Z"
        }))
        .is_ok());
    }
}
