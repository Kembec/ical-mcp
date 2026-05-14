# OpenClaw Compatibility

This MCP server is compatible with [OpenClaw](https://openclaw.ai) gateways.

## Configuration (openclaw.json)

```json
"icloud-calendar": {
  "command": "npx",
  "args": ["-y", "github:Kembec/ical-mcp#main", "--prefix", "npm-package"],
  "env": {
    "ICLOUD_USERNAME": "${ICLOUD_USER}",
    "ICLOUD_PASSWORD": "${ICLOUD_PW}"
  }
}
```

## Auth

Uses Apple app-specific password. Generate one at appleid.apple.com → Security → App-Specific Passwords.  
Set `ICLOUD_USER` and `ICLOUD_PW` in your `.env.life` file.

**Do not set `CALDAV_LOG_HTTP=true` in production** — it logs request URLs which may contain account information.

## Security notes (vs upstream)

- Postinstall: JAR download now records SHA-256 sidecar for manual audit
- CALDAV_LOG_HTTP: explicitly documented as production-unsafe
- Upstream: [icloud-calendar-mcp/icloud-calendar-mcp](https://github.com/icloud-calendar-mcp/icloud-calendar-mcp)
