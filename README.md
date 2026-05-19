# ical-mcp

[![npm](https://img.shields.io/npm/v/@kembec/ical-mcp)](https://www.npmjs.com/package/@kembec/ical-mcp)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

A Model Context Protocol server for iCloud Calendar, written in Rust.

It talks CalDAV directly to `caldav.icloud.com`, exposes five tools over
JSON-RPC on stdio, and ships as a single static binary — no JVM, no Python
runtime, no daemons.

## Prerequisites

- An Apple ID
- An [app-specific password](https://appleid.apple.com/account/manage) for
  that Apple ID (regular Apple ID passwords will not work with CalDAV)

## Installation

```bash
npm install -g @kembec/ical-mcp
```

Or run with `npx`:

```bash
npx @kembec/ical-mcp
```

## Configuration

Set two environment variables before launching the server:

```bash
export ICLOUD_USERNAME="you@icloud.com"
export ICLOUD_PASSWORD="xxxx-xxxx-xxxx-xxxx"   # app-specific password
```

Credentials are read at startup and never written to disk.

### Cursor

Add to your `~/.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "ical": {
      "command": "npx",
      "args": ["-y", "@kembec/ical-mcp"],
      "env": {
        "ICLOUD_USERNAME": "you@icloud.com",
        "ICLOUD_PASSWORD": "xxxx-xxxx-xxxx-xxxx"
      }
    }
  }
}
```

### Claude Desktop

Add to your `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "ical": {
      "command": "npx",
      "args": ["-y", "@kembec/ical-mcp"],
      "env": {
        "ICLOUD_USERNAME": "you@icloud.com",
        "ICLOUD_PASSWORD": "xxxx-xxxx-xxxx-xxxx"
      }
    }
  }
}
```

## Tools

- **list-calendars** — list every calendar in the account.
- **get-events** — `calendar_id`, `start_date`, `end_date` (YYYY-MM-DD).
- **create-event** — `calendar_id`, `title`, then either `start_time`/`end_time`
  (ISO 8601) for timed events or `start_date`/`end_date` with `all_day: true`
  for all-day events. Optional: `description`, `location`, `timezone`.
- **update-event** — `event_id` (UID or full URL) plus any of `title`,
  `start_time`, `end_time`, `description`, `location`.
- **delete-event** — `event_id` (UID or full URL).

`calendar_id` accepts either the calendar's display name or its full CalDAV
URL. `event_id` accepts the iCalendar UID or the resource URL returned by
`create-event`.

## Building from source

```bash
cargo build --release
./target/release/ical-mcp
```

## License

Apache-2.0
