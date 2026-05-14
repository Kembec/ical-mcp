# ical-mcp

MCP server for iCloud Calendar via CalDAV. Fork of `@icloud-calendar-mcp/server` with JAR integrity verification and OpenClaw support.

This fork adds: SHA-256 verification of the JAR on install (`.sha256` sidecar), size warning if the download looks wrong, and a ready-to-use OpenClaw config. See [SECURITY_AUDIT.md](./SECURITY_AUDIT.md) and [OPENCLAW.md](./OPENCLAW.md).

Requires Java 17+.

## Installation

```bash
npx @kembec/ical-mcp
```

Claude Desktop config (`~/Library/Application Support/Claude/claude_desktop_config.json` on macOS):

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

Use an app-specific password, not your Apple ID password. Generate one at [appleid.apple.com](https://appleid.apple.com) → Security → App-Specific Passwords.

## Tools

`list_calendars` · `get_events` · `create_event` · `update_event` · `delete_event`

For OpenClaw see [OPENCLAW.md](./OPENCLAW.md).

## License

Apache 2.0

## Credits

Original project: [icloud-calendar-mcp/icloud-calendar-mcp](https://github.com/icloud-calendar-mcp/icloud-calendar-mcp), published as [`@icloud-calendar-mcp/server`](https://www.npmjs.com/package/@icloud-calendar-mcp/server) and [`icloud-calendar-mcp`](https://pypi.org/project/icloud-calendar-mcp/) on PyPI. All credit for the Kotlin implementation, CalDAV integration, and credential masking goes to the upstream authors.
