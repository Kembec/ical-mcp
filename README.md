# ical-mcp

Fork de [icloud-calendar-mcp/icloud-calendar-mcp](https://github.com/icloud-calendar-mcp/icloud-calendar-mcp) con verificación de integridad del JAR y compatibilidad OpenClaw. La implementación en Kotlin, la integración CalDAV y el enmascaramiento de credenciales son del proyecto original — todo el crédito es de sus autores.

Este fork agrega: verificación SHA-256 del JAR al instalar (sidecar `.sha256`), advertencia si el JAR pesa menos de lo esperado, y bloque de configuración listo para OpenClaw. Ver [SECURITY_AUDIT.md](./SECURITY_AUDIT.md) y [OPENCLAW.md](./OPENCLAW.md).

## Qué hace

Expone iCloud Calendar como herramientas MCP vía CalDAV: leer calendarios, listar y crear eventos, actualizar, borrar. Se autentica con contraseña de app de Apple — nunca tu contraseña principal.

Requiere Java 17+.

## Instalación

```bash
npx @kembec/ical-mcp
```

Con Claude Desktop:

```json
{
  "mcpServers": {
    "icloud-calendar": {
      "command": "npx",
      "args": ["@kembec/ical-mcp"],
      "env": {
        "ICLOUD_USERNAME": "tu-apple-id@icloud.com",
        "ICLOUD_PASSWORD": "contraseña-de-app"
      }
    }
  }
}
```

La contraseña de app la generás en [appleid.apple.com](https://appleid.apple.com) → Seguridad → Contraseñas de aplicaciones. Se puede revocar independientemente sin tocar tu cuenta.

## Herramientas disponibles

`list_calendars`, `get_events`, `create_event`, `update_event`, `delete_event`

Para OpenClaw ver [OPENCLAW.md](./OPENCLAW.md).

## Créditos

Proyecto original: [icloud-calendar-mcp/icloud-calendar-mcp](https://github.com/icloud-calendar-mcp/icloud-calendar-mcp). Publicado en npm como [`@icloud-calendar-mcp/server`](https://www.npmjs.com/package/@icloud-calendar-mcp/server) y en PyPI como [`icloud-calendar-mcp`](https://pypi.org/project/icloud-calendar-mcp/).

## Licencia

Apache 2.0
