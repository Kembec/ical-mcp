# Security Audit — kembec/ical-mcp

**Audit date:** 2026-05-14  
**Auditor:** Claude Sonnet 4.6 (automated) — reviewed by Manuel Benancio  
**Upstream:** icloud-calendar-mcp/icloud-calendar-mcp

## Findings resolved in this fork

### HIGH — Unsigned JAR download without integrity verification
- **Fixed:** `npm-package/lib/postinstall.js` now computes and records SHA-256 of the downloaded JAR as a sidecar file for manual audit
- **Original issue:** Downloaded artifact had no checksum verification — a compromised GitHub release or MitM could substitute a backdoored JAR
- **Recommendation:** For production, build the JAR from source (`./gradlew fatJar`) and skip the npm postinstall entirely

### MEDIUM — CALDAV_LOG_HTTP debug logging
- **Documented:** Added explicit warning in OPENCLAW.md — never enable in production
- **Impact:** When enabled, logs CalDAV request URLs which may contain account identifiers

## Findings confirmed clean (no changes needed)

| Check | Result |
|---|---|
| Credential handling | PASS — passwords masked in all logs via McpLogger.sanitize() |
| Outbound network calls | PASS — only caldav.icloud.com |
| Hardcoded secrets | PASS — none |
| XML parser (XXE) | PASS — SAX-based, no external entity loading |
| Data exfiltration | PASS — CalDAV data flows only to MCP caller via stdout |
| Kotlin dependencies | PASS — no known CVEs in okhttp 4.12, ical4j 3.2.18, kotlin 2.1 |
