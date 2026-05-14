//! CalDAV (RFC 4791) client tuned for iCloud.
//!
//! iCloud quirks:
//!   * The advertised CalDAV root is `https://caldav.icloud.com`, but every
//!     user is redirected to a per-account host like `pXX-caldav.icloud.com`.
//!     We follow the `Location` header and capture the host for later calls.
//!   * The principal URL is found via PROPFIND on `/` requesting
//!     `current-user-principal`. The calendar home is then found via
//!     PROPFIND on the principal requesting `calendar-home-set`.

use anyhow::{anyhow, Context, Result};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::{Method, Url};
use roxmltree::Document;

use crate::auth::{basic_auth_header, Credentials};
use crate::ical;

const CALDAV_ROOT: &str = "https://caldav.icloud.com";

#[derive(Debug, Clone)]
pub struct Calendar {
    pub display_name: String,
    pub url: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Event {
    pub uid: String,
    pub url: String,
    pub etag: String,
    pub summary: String,
    pub start: String,
    pub end: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub all_day: bool,
}

pub struct CalDavClient {
    client: reqwest::Client,
    credentials: Credentials,
    /// Resolved calendar-home-set URL (per-user, includes the trailing `/`).
    pub calendar_home: String,
}

impl CalDavClient {
    /// Construct a client and discover the calendar home for the current
    /// principal. Network calls happen here.
    pub async fn new(client: reqwest::Client, credentials: Credentials) -> Result<Self> {
        let mut me = Self {
            client,
            credentials,
            calendar_home: String::new(),
        };
        me.discover().await?;
        Ok(me)
    }

    /// Construct a client with a pre-resolved calendar home. Useful for tests.
    #[allow(dead_code)]
    pub fn with_home(
        client: reqwest::Client,
        credentials: Credentials,
        calendar_home: String,
    ) -> Self {
        Self {
            client,
            credentials,
            calendar_home,
        }
    }

    fn auth_headers(&self) -> HeaderMap {
        let mut h = HeaderMap::new();
        if let Ok(v) = HeaderValue::from_str(&basic_auth_header(&self.credentials)) {
            h.insert(AUTHORIZATION, v);
        }
        h.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/xml; charset=utf-8"),
        );
        h
    }

    async fn request(
        &self,
        method: &str,
        url: &str,
        depth: Option<&str>,
        body: Option<String>,
        extra_headers: &[(&str, &str)],
    ) -> Result<(u16, HeaderMap, String)> {
        let method = Method::from_bytes(method.as_bytes())
            .with_context(|| format!("invalid HTTP method `{method}`"))?;
        let url_parsed = Url::parse(url).with_context(|| format!("invalid URL `{url}`"))?;
        let mut req = self.client.request(method, url_parsed);
        let mut headers = self.auth_headers();
        if let Some(d) = depth {
            headers.insert(
                HeaderName::from_static("depth"),
                HeaderValue::from_str(d).context("invalid Depth header")?,
            );
        }
        for (k, v) in extra_headers {
            let name = HeaderName::from_bytes(k.as_bytes())
                .with_context(|| format!("invalid header name `{k}`"))?;
            let val = HeaderValue::from_str(v)
                .with_context(|| format!("invalid header value for `{k}`"))?;
            headers.insert(name, val);
        }
        req = req.headers(headers);
        if let Some(b) = body {
            req = req.body(b);
        }
        let resp = req.send().await.context("CalDAV request failed")?;
        let status = resp.status().as_u16();
        let resp_headers = resp.headers().clone();
        let text = resp.text().await.unwrap_or_default();
        Ok((status, resp_headers, text))
    }

    /// PROPFIND on `/` to find the principal, then PROPFIND on the principal
    /// to find the calendar home.
    async fn discover(&mut self) -> Result<()> {
        let body = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:">
  <d:prop>
    <d:current-user-principal/>
  </d:prop>
</d:propfind>"#;

        let (status, _h, text) = self
            .request(
                "PROPFIND",
                CALDAV_ROOT,
                Some("0"),
                Some(body.to_string()),
                &[],
            )
            .await?;
        if !(200..300).contains(&status) {
            return Err(anyhow!(
                "CalDAV discovery failed (status {status}): {}",
                truncate(&text, 400)
            ));
        }
        let principal_href = extract_href(&text, "current-user-principal")
            .ok_or_else(|| anyhow!("could not find current-user-principal in response"))?;
        let principal_url = join_url(CALDAV_ROOT, &principal_href)?;

        let body2 = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
  <d:prop>
    <c:calendar-home-set/>
  </d:prop>
</d:propfind>"#;
        let (status2, _h2, text2) = self
            .request(
                "PROPFIND",
                &principal_url,
                Some("0"),
                Some(body2.to_string()),
                &[],
            )
            .await?;
        if !(200..300).contains(&status2) {
            return Err(anyhow!(
                "calendar-home-set lookup failed (status {status2}): {}",
                truncate(&text2, 400)
            ));
        }
        let home_href = extract_href(&text2, "calendar-home-set")
            .ok_or_else(|| anyhow!("could not find calendar-home-set in response"))?;
        let home_url = join_url(&principal_url, &home_href)?;
        self.calendar_home = ensure_trailing_slash(&home_url);
        Ok(())
    }

    /// PROPFIND on the calendar home, depth 1, to list collections that
    /// support VEVENT.
    pub async fn list_calendars(&self) -> Result<Vec<Calendar>> {
        let body = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
  <d:prop>
    <d:displayname/>
    <c:calendar-description/>
    <d:resourcetype/>
    <c:supported-calendar-component-set/>
  </d:prop>
</d:propfind>"#;
        let (status, _h, text) = self
            .request(
                "PROPFIND",
                &self.calendar_home,
                Some("1"),
                Some(body.to_string()),
                &[],
            )
            .await?;
        if !(200..300).contains(&status) {
            return Err(anyhow!(
                "list_calendars failed (status {status}): {}",
                truncate(&text, 400)
            ));
        }
        parse_calendar_list(&text, &self.calendar_home)
    }

    /// REPORT calendar-query for VEVENT in a time range.
    pub async fn get_events(
        &self,
        calendar_url: &str,
        start: &str,
        end: &str,
    ) -> Result<Vec<Event>> {
        let start_n = ical::normalize_datetime(start)?;
        let end_n = ical::normalize_datetime(end)?;
        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<c:calendar-query xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
  <d:prop>
    <d:getetag/>
    <c:calendar-data/>
  </d:prop>
  <c:filter>
    <c:comp-filter name="VCALENDAR">
      <c:comp-filter name="VEVENT">
        <c:time-range start="{start}" end="{end}"/>
      </c:comp-filter>
    </c:comp-filter>
  </c:filter>
</c:calendar-query>"#,
            start = start_n,
            end = end_n,
        );
        let (status, _h, text) = self
            .request("REPORT", calendar_url, Some("1"), Some(body), &[])
            .await?;
        if !(200..300).contains(&status) {
            return Err(anyhow!(
                "get_events failed (status {status}): {}",
                truncate(&text, 400)
            ));
        }
        parse_event_report(&text, calendar_url)
    }

    /// PUT a new event to `<calendar_url>/<uid>.ics`. Uses `If-None-Match: *`
    /// to fail loudly if the UID is already taken.
    pub async fn create_event(
        &self,
        calendar_url: &str,
        uid: &str,
        ical_data: &str,
    ) -> Result<String> {
        let event_url = format!("{}{}.ics", ensure_trailing_slash(calendar_url), uid);
        let (status, _h, text) = self
            .request(
                "PUT",
                &event_url,
                None,
                Some(ical_data.to_string()),
                &[
                    ("Content-Type", "text/calendar; charset=utf-8"),
                    ("If-None-Match", "*"),
                ],
            )
            .await?;
        if !(200..300).contains(&status) {
            return Err(anyhow!(
                "create_event failed (status {status}): {}",
                truncate(&text, 400)
            ));
        }
        Ok(event_url)
    }

    /// PUT an updated event. `If-Match` enforces optimistic concurrency via
    /// the prior ETag; pass `"*"` to overwrite unconditionally.
    pub async fn update_event(&self, event_url: &str, etag: &str, ical_data: &str) -> Result<()> {
        let (status, _h, text) = self
            .request(
                "PUT",
                event_url,
                None,
                Some(ical_data.to_string()),
                &[
                    ("Content-Type", "text/calendar; charset=utf-8"),
                    ("If-Match", etag),
                ],
            )
            .await?;
        if !(200..300).contains(&status) {
            return Err(anyhow!(
                "update_event failed (status {status}): {}",
                truncate(&text, 400)
            ));
        }
        Ok(())
    }

    /// DELETE the event resource.
    pub async fn delete_event(&self, event_url: &str, etag: &str) -> Result<()> {
        let headers: Vec<(&str, &str)> = if etag.is_empty() {
            Vec::new()
        } else {
            vec![("If-Match", etag)]
        };
        let (status, _h, text) = self
            .request("DELETE", event_url, None, None, &headers)
            .await?;
        if !(200..300).contains(&status) && status != 404 {
            return Err(anyhow!(
                "delete_event failed (status {status}): {}",
                truncate(&text, 400)
            ));
        }
        Ok(())
    }

    /// GET a single event resource and return the parsed event + ETag.
    pub async fn get_event(&self, event_url: &str) -> Result<(Event, String)> {
        let (status, headers, body) = self.request("GET", event_url, None, None, &[]).await?;
        if !(200..300).contains(&status) {
            return Err(anyhow!(
                "get_event failed (status {status}): {}",
                truncate(&body, 400)
            ));
        }
        let etag = headers
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let parsed = ical::parse_vevent(&body)?;
        let ev = Event {
            uid: parsed.uid,
            url: event_url.to_string(),
            etag: etag.clone(),
            summary: parsed.summary,
            start: parsed.start,
            end: parsed.end,
            description: parsed.description,
            location: parsed.location,
            all_day: parsed.all_day,
        };
        Ok((ev, etag))
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

fn ensure_trailing_slash(s: &str) -> String {
    if s.ends_with('/') {
        s.to_string()
    } else {
        format!("{s}/")
    }
}

fn join_url(base: &str, href: &str) -> Result<String> {
    let base_url = Url::parse(base).with_context(|| format!("invalid base URL `{base}`"))?;
    let joined = base_url
        .join(href)
        .with_context(|| format!("could not join `{href}` onto `{base}`"))?;
    Ok(joined.to_string())
}

/// Extract the first `<d:href>` inside the named element (e.g.
/// `current-user-principal` or `calendar-home-set`).
fn extract_href(xml: &str, element: &str) -> Option<String> {
    let doc = Document::parse(xml).ok()?;
    for node in doc.descendants() {
        if node.has_tag_name(element) {
            for child in node.descendants() {
                if child.has_tag_name("href") {
                    if let Some(t) = child.text() {
                        return Some(t.trim().to_string());
                    }
                }
            }
        }
    }
    None
}

fn parse_calendar_list(xml: &str, base_url: &str) -> Result<Vec<Calendar>> {
    let doc = Document::parse(xml).context("invalid CalDAV XML response")?;
    let mut out = Vec::new();
    for response in doc.descendants().filter(|n| n.has_tag_name("response")) {
        let href = response
            .descendants()
            .find(|n| n.has_tag_name("href"))
            .and_then(|n| n.text())
            .unwrap_or("")
            .trim()
            .to_string();
        if href.is_empty() {
            continue;
        }
        // Must be a calendar collection.
        let is_calendar = response
            .descendants()
            .filter(|n| n.has_tag_name("resourcetype"))
            .any(|rt| rt.descendants().any(|c| c.has_tag_name("calendar")));
        if !is_calendar {
            continue;
        }
        // Must support VEVENT.
        let supports_vevent = response
            .descendants()
            .filter(|n| n.has_tag_name("supported-calendar-component-set"))
            .flat_map(|s| s.descendants())
            .any(|c| {
                c.has_tag_name("comp")
                    && c.attribute("name")
                        .map(|v| v.eq_ignore_ascii_case("VEVENT"))
                        .unwrap_or(false)
            });
        // If supported-calendar-component-set is absent, assume VEVENT is fine.
        let component_present = response
            .descendants()
            .any(|n| n.has_tag_name("supported-calendar-component-set"));
        if component_present && !supports_vevent {
            continue;
        }

        let display_name = response
            .descendants()
            .find(|n| n.has_tag_name("displayname"))
            .and_then(|n| n.text())
            .unwrap_or("")
            .trim()
            .to_string();
        let description = response
            .descendants()
            .find(|n| n.has_tag_name("calendar-description"))
            .and_then(|n| n.text())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let abs_url = join_url(base_url, &href).unwrap_or_else(|_| href.clone());
        out.push(Calendar {
            display_name: if display_name.is_empty() {
                href.clone()
            } else {
                display_name
            },
            url: abs_url,
            description,
        });
    }
    Ok(out)
}

fn parse_event_report(xml: &str, base_url: &str) -> Result<Vec<Event>> {
    let doc = Document::parse(xml).context("invalid CalDAV REPORT response")?;
    let mut out = Vec::new();
    for response in doc.descendants().filter(|n| n.has_tag_name("response")) {
        let href = response
            .descendants()
            .find(|n| n.has_tag_name("href"))
            .and_then(|n| n.text())
            .unwrap_or("")
            .trim()
            .to_string();
        if href.is_empty() {
            continue;
        }
        let etag = response
            .descendants()
            .find(|n| n.has_tag_name("getetag"))
            .and_then(|n| n.text())
            .unwrap_or("")
            .trim()
            .to_string();
        let cdata = response
            .descendants()
            .find(|n| n.has_tag_name("calendar-data"))
            .and_then(|n| n.text())
            .unwrap_or("")
            .to_string();
        if cdata.trim().is_empty() {
            continue;
        }
        let parsed = match ical::parse_vevent(&cdata) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let abs_url = join_url(base_url, &href).unwrap_or_else(|_| href.clone());
        out.push(Event {
            uid: parsed.uid,
            url: abs_url,
            etag,
            summary: parsed.summary,
            start: parsed.start,
            end: parsed.end,
            description: parsed.description,
            location: parsed.location,
            all_day: parsed.all_day,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_trailing_slash_appends() {
        assert_eq!(ensure_trailing_slash("https://a/b"), "https://a/b/");
        assert_eq!(ensure_trailing_slash("https://a/b/"), "https://a/b/");
    }

    #[test]
    fn join_url_handles_absolute_and_relative() {
        let abs = join_url("https://x.example/", "/p/").unwrap();
        assert_eq!(abs, "https://x.example/p/");
        let rel = join_url("https://x.example/a/", "b").unwrap();
        assert_eq!(rel, "https://x.example/a/b");
    }

    #[test]
    fn extract_href_finds_principal() {
        let xml = r#"<?xml version="1.0"?>
<multistatus xmlns="DAV:">
  <response>
    <href>/</href>
    <propstat><prop>
      <current-user-principal>
        <href>/12345/principal/</href>
      </current-user-principal>
    </prop></propstat>
  </response>
</multistatus>"#;
        assert_eq!(
            extract_href(xml, "current-user-principal").as_deref(),
            Some("/12345/principal/")
        );
    }

    #[test]
    fn parse_calendar_list_finds_one() {
        let xml = r#"<?xml version="1.0"?>
<multistatus xmlns="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
  <response>
    <href>/12345/calendars/home/</href>
    <propstat><prop>
      <resourcetype><collection/></resourcetype>
    </prop></propstat>
  </response>
  <response>
    <href>/12345/calendars/work/</href>
    <propstat><prop>
      <displayname>Work</displayname>
      <resourcetype><collection/><c:calendar/></resourcetype>
      <c:supported-calendar-component-set>
        <c:comp name="VEVENT"/>
      </c:supported-calendar-component-set>
    </prop></propstat>
  </response>
</multistatus>"#;
        let cals = parse_calendar_list(xml, "https://p01.example/12345/calendars/").unwrap();
        assert_eq!(cals.len(), 1);
        assert_eq!(cals[0].display_name, "Work");
        assert!(cals[0].url.ends_with("/12345/calendars/work/"));
    }
}
