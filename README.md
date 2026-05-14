# ical-mcp

Fork of [icloud-calendar-mcp/icloud-calendar-mcp](https://github.com/icloud-calendar-mcp/icloud-calendar-mcp) with JAR integrity verification and OpenClaw compatibility. The Kotlin implementation, CalDAV integration, and credential masking are from the original project — all credit goes to its authors.

This fork adds: SHA-256 verification of the JAR on install (`.sha256` sidecar), warning if the JAR is smaller than expected, and a ready-to-use OpenClaw config block. See [SECURITY_AUDIT.md](./SECURITY_AUDIT.md) and [OPENCLAW.md](./OPENCLAW.md).

## What it does

Exposes iCloud Calendar as MCP tools via CalDAV: list calendars, get and create events, update, delete. Authenticates with an Apple app-specific password — never your main Apple ID password.

Requires Java 17+.

## Installation

```bash
npx @kembec/ical-mcp
```

With Claude Desktop:

```json
{
  "mcpServers": {
    "icloud-calendar": {
      "command": "npx",
      "args": ["@kembec/ical-mcp"],
      "env": {
        "ICLOUD_USERNAME": "your-apple-id@icloud.com",
        "ICLOUD_PASSWORD": "app-specific-password"
      }
    }
  }
}
```

Generate an app-specific password at [appleid.apple.com](https://appleid.apple.com) → Security → App-Specific Passwords. It can be revoked independently without touching your account.

## Available tools

`list_calendars`, `get_events`, `create_event`, `update_event`, `delete_event`

For OpenClaw see [OPENCLAW.md](./OPENCLAW.md).

## Credits

Original project: [icloud-calendar-mcp/icloud-calendar-mcp](https://github.com/icloud-calendar-mcp/icloud-calendar-mcp). Published on npm as [`@icloud-calendar-mcp/server`](https://www.npmjs.com/package/@icloud-calendar-mcp/server) and on PyPI as [`icloud-calendar-mcp`](https://pypi.org/project/icloud-calendar-mcp/).

## License

Apache 2.0
