//! Outgoing email through Mailgun's HTTP API.
//!
//! Configured by `MAILGUN_API_KEY` (a sending key), `MAILGUN_DOMAIN` (the
//! verified sending domain) and `MAILGUN_FROM` (the From header). Without them
//! nothing can be sent and every send fails.

use crate::error::prelude::*;
use reqwest::{Client, StatusCode};
use std::time::Duration;

const MAILGUN_API_URL: &str = "https://api.mailgun.net";
const ATTEMPTS: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingEmail {
    pub to: String,
    pub subject: String,
    pub text: String,
    pub html: Option<String>,
}

/// Holds the API key, so it must never reach an API response or log.
#[derive(Clone)]
pub struct MailgunSender {
    api_url: String,
    domain: String,
    api_key: String,
    from: String,
    client: Client,
}

impl std::fmt::Debug for MailgunSender {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MailgunSender")
            .field("domain", &self.domain)
            .field("from", &self.from)
            .finish_non_exhaustive()
    }
}

impl MailgunSender {
    pub fn from_env() -> Result<Self> {
        Self::from_vars(|name| std::env::var(name).ok())
    }

    fn from_vars(get: impl Fn(&str) -> Option<String>) -> Result<Self> {
        let require = |name: &str| {
            get(name)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    Error::new(
                        format!("email sending is not configured: {name} is not set"),
                        ErrorType::EmailSend,
                    )
                })
        };
        let api_key = require("MAILGUN_API_KEY")?;
        let domain = require("MAILGUN_DOMAIN")?;
        let from = require("MAILGUN_FROM")?;
        Ok(Self::new(str!(MAILGUN_API_URL), domain, api_key, from))
    }

    pub fn new(api_url: String, domain: String, api_key: String, from: String) -> Self {
        Self {
            api_url: api_url.trim_end_matches('/').into(),
            domain,
            api_key,
            from,
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("reqwest client"),
        }
    }

    /// Sending is not idempotent, so only failures where Mailgun cannot have
    /// accepted the message are retried: refused connections and 429s.
    pub async fn send(&self, email: &OutgoingEmail) -> Result<()> {
        let url = format!("{}/v3/{}/messages", self.api_url, self.domain);
        let mut form = vec![
            ("from", self.from.as_str()),
            ("to", email.to.as_str()),
            ("subject", email.subject.as_str()),
            ("text", email.text.as_str()),
        ];
        if let Some(html) = &email.html {
            form.push(("html", html.as_str()));
        }

        for attempt in 0..ATTEMPTS {
            let last = attempt + 1 == ATTEMPTS;
            let result = self
                .client
                .post(&url)
                .basic_auth("api", Some(&self.api_key))
                .form(&form)
                .send()
                .await;
            let delay = match result {
                Ok(response) if response.status().is_success() => return Ok(()),
                Ok(response)
                    if response.status() == StatusCode::TOO_MANY_REQUESTS && !last =>
                {
                    retry_after(&response).unwrap_or_else(|| backoff(attempt))
                }
                Ok(response) => {
                    bail!(Error::new(
                        format!(
                            "Mailgun refused the message: HTTP {}",
                            response.status()
                        ),
                        ErrorType::EmailSend,
                    ));
                }
                Err(error) if error.is_connect() && !last => backoff(attempt),
                Err(error) => {
                    bail!(Error::new(
                        format!("Mailgun unavailable: {error}"),
                        ErrorType::EmailSend,
                    ));
                }
            };
            if delay > Duration::from_secs(10) {
                bail!(Error::new(
                    "Mailgun Retry-After exceeds retry budget",
                    ErrorType::EmailSend,
                ));
            }
            tokio::time::sleep(delay).await;
        }
        unreachable!("the last attempt always returns")
    }
}

fn retry_after(response: &reqwest::Response) -> Option<Duration> {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
}

fn backoff(attempt: u32) -> Duration {
    Duration::from_millis(200 * (1 << attempt) + rand::random_range(0..100))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Answers each connection with the next status and records the raw request.
    async fn fake_mailgun(statuses: Vec<u16>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = requests.clone();
        tokio::spawn(async move {
            for status in statuses {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let header_end = loop {
                    let mut chunk = [0; 4096];
                    let size = socket.read(&mut chunk).await.unwrap();
                    assert!(size > 0);
                    bytes.extend_from_slice(&chunk[..size]);
                    if let Some(position) =
                        bytes.windows(4).position(|part| part == b"\r\n\r\n")
                    {
                        break position + 4;
                    }
                };
                let header = String::from_utf8_lossy(&bytes[..header_end]);
                let length: usize = header
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|value| value.trim().parse().ok())
                    })
                    .unwrap_or(0);
                while bytes.len() < header_end + length {
                    let mut chunk = [0; 4096];
                    let size = socket.read(&mut chunk).await.unwrap();
                    assert!(size > 0);
                    bytes.extend_from_slice(&chunk[..size]);
                }
                seen.lock().unwrap().push(String::from_utf8(bytes).unwrap());
                let body =
                    r#"{"id":"<1@mg.example.org>","message":"Queued. Thank you."}"#;
                let reply = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nRetry-After: 0\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len(),
                );
                socket.write_all(reply.as_bytes()).await.unwrap();
            }
        });
        (url, requests)
    }

    fn sender(url: String) -> MailgunSender {
        MailgunSender::new(
            url,
            str!("mg.example.org"),
            str!("key-secret"),
            str!("Cobalt Company <noreply@mg.example.org>"),
        )
    }

    fn form_fields(request: &str) -> HashMap<String, String> {
        let body = request.split_once("\r\n\r\n").unwrap().1;
        form_urlencoded::parse(body.as_bytes())
            .into_owned()
            .collect()
    }

    fn invite() -> OutgoingEmail {
        OutgoingEmail {
            to: str!("alice@example.com"),
            subject: str!("Set your password"),
            text: str!("Open https://cobalt.example/-/set-password/abc & choose one."),
            html: None,
        }
    }

    #[tokio::test]
    async fn sends_exact_form_fields_with_basic_auth() {
        let (url, requests) = fake_mailgun(vec![200]).await;
        let email = OutgoingEmail {
            html: Some(str!("<p>Choose a password</p>")),
            ..invite()
        };
        sender(url).send(&email).await.unwrap();

        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].starts_with("POST /v3/mg.example.org/messages HTTP/1.1"));
        // base64("api:key-secret")
        assert!(
            requests[0]
                .to_ascii_lowercase()
                .contains("authorization: basic yxbpomtles1zzwnyzxq=")
        );
        let expected: HashMap<String, String> = [
            ("from", "Cobalt Company <noreply@mg.example.org>"),
            ("to", "alice@example.com"),
            ("subject", "Set your password"),
            (
                "text",
                "Open https://cobalt.example/-/set-password/abc & choose one.",
            ),
            ("html", "<p>Choose a password</p>"),
        ]
        .into_iter()
        .map(|(key, value)| (str!(key), str!(value)))
        .collect();
        assert_eq!(form_fields(&requests[0]), expected);
    }

    #[tokio::test]
    async fn text_only_email_sends_no_html_field() {
        let (url, requests) = fake_mailgun(vec![200]).await;
        sender(url).send(&invite()).await.unwrap();
        let fields = form_fields(&requests.lock().unwrap()[0]);
        let mut keys: Vec<_> = fields.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["from", "subject", "text", "to"]);
    }

    #[tokio::test]
    async fn rate_limited_send_is_retried_but_rejection_is_not() {
        let (url, requests) = fake_mailgun(vec![429, 200]).await;
        sender(url).send(&invite()).await.unwrap();
        assert_eq!(requests.lock().unwrap().len(), 2);

        let (url, requests) = fake_mailgun(vec![500, 200]).await;
        let error = sender(url).send(&invite()).await.unwrap_err();
        assert_eq!(error.error_type, ErrorType::EmailSend);
        assert_eq!(
            requests.lock().unwrap().len(),
            1,
            "a 500 may have been accepted"
        );
    }

    #[test]
    fn unconfigured_sender_fails_naming_the_missing_variable() {
        let vars: HashMap<&str, &str> = [
            ("MAILGUN_API_KEY", "key-secret"),
            ("MAILGUN_DOMAIN", "mg.example.org"),
            ("MAILGUN_FROM", ""),
        ]
        .into();
        let error =
            MailgunSender::from_vars(|name| vars.get(name).map(|v| str!(v))).unwrap_err();
        assert_eq!(error.error_type, ErrorType::EmailSend);
        assert!(error.message.contains("MAILGUN_FROM"), "{}", error.message);

        let error = MailgunSender::from_vars(|_| None).unwrap_err();
        assert!(
            error.message.contains("MAILGUN_API_KEY"),
            "{}",
            error.message
        );

        let configured = MailgunSender::from_vars(|name| {
            vars.get(name)
                .map(|v| if v.is_empty() { "Cobalt <a@b.c>" } else { v })
                .map(|v| str!(v))
        })
        .unwrap();
        assert_eq!(configured.api_url, MAILGUN_API_URL);
        assert!(!format!("{configured:?}").contains("key-secret"));
    }
}
