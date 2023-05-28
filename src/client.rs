use std::{thread, time};

use reqwest;
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::Value;
use std::fmt;
use std::vec;

const RESAS_ENDPOINT: &str = "https://opendata.resas-portal.go.jp";

#[derive(Debug, Deserialize)]
pub struct ResasResponse<T> {
    message: Option<String>,
    #[serde(bound(deserialize = "Vec<T>: Deserialize<'de>"))]
    pub result: Vec<T>,
}

#[derive(Debug)]
pub enum ErrorKind {
    Fatal,
    Retryable,
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    source: Option<Box<dyn std::error::Error>>,
    message: Option<String>,
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|e| e.as_ref())
    }
}

impl std::convert::From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Error {
        Error {
            kind: ErrorKind::Fatal,
            source: Some(Box::from(err)),
            message: None,
        }
    }
}

impl std::convert::From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Error {
        Error {
            kind: ErrorKind::Fatal,
            source: Some(Box::from(err)),
            message: None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self.kind {
            ErrorKind::Fatal => write!(f, "Fatal error: ")?,
            ErrorKind::Retryable => write!(f, "Retryable error: ")?,
        }
        if let Some(message) = self.message.as_ref() {
            write!(f, "{}", message)?;
        }
        if let Some(source) = self.source.as_ref() {
            return source.fmt(f);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct RetryPolicy {
    retriable_codes: Vec<String>,
    interval: u64,
    attempts: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            retriable_codes: vec![String::from("500"), String::from("502")],
            interval: 60,
            attempts: 3,
        }
    }
}

pub struct Client {
    client: reqwest::blocking::Client,
    api_key: String,
    retry_policy: RetryPolicy,
}

impl Client {
    pub fn new(api_key: String, retry_policy: RetryPolicy) -> Self {
        Client {
            client: reqwest::blocking::Client::new(),
            api_key: api_key,
            retry_policy: retry_policy,
        }
    }
    fn send_request_with_retry(&self, url: &str) -> Result<String, Error> {
        let mut attempts = 0;
        loop {
            let err = match self.send_request(url) {
                Ok(r) => return Ok(r),
                Err(err) => match err.kind {
                    ErrorKind::Fatal => return Err(err),
                    ErrorKind::Retryable => err,
                },
            };
            attempts += 1;
            if attempts == self.retry_policy.attempts {
                return Err(Error {
                    kind: ErrorKind::Fatal,
                    source: err.source,
                    message: Some(format!("Retried {} but couldn't recover", attempts)),
                });
            }
            thread::sleep(time::Duration::from_secs(self.retry_policy.interval));
        }
    }
    fn send_request(&self, url: &str) -> Result<String, Error> {
        let result = self
            .client
            .get(url)
            .header("X-API-KEY", &self.api_key)
            .send();

        match result?.error_for_status() {
            Ok(response) => {
                let resopnse_text = response.text()?;
                let response_json: Value = serde_json::from_str(resopnse_text.as_str())?;

                // RESAS-API returns error status code in its body although the status in its response header is 200.
                if let Some(status_code) = response_json.get("statusCode") {
                    if self
                        .retry_policy
                        .retriable_codes
                        .contains(&status_code.to_string())
                    {
                        return Err(Error {
                            kind: ErrorKind::Retryable,
                            source: None,
                            message: Some(response_json["message"].to_string()),
                        });
                    }
                    if status_code.to_string().starts_with("2") {
                        return Ok(resopnse_text);
                    }
                    return Err(Error {
                        kind: ErrorKind::Fatal,
                        source: None,
                        message: Some(format!(
                            "{status_code} {message}",
                            status_code = status_code,
                            message = response_json["message"],
                        )),
                    });
                }
                Ok(resopnse_text)
            }
            Err(err) => {
                if let Some(status_code) = err.status() {
                    if self
                        .retry_policy
                        .retriable_codes
                        .contains(&status_code.to_string())
                    {
                        return Err(Error {
                            kind: ErrorKind::Retryable,
                            source: Some(Box::from(err)),
                            message: Some(format!("Status code {}", status_code)),
                        });
                    }
                }
                Err(Error::from(err))
            }
        }
    }
    pub fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        parameters: Option<&str>,
        with_retry: bool,
    ) -> Result<ResasResponse<T>, Error> {
        let url = format!("{}/{}", RESAS_ENDPOINT, path);
        let url = match parameters {
            None => url,
            Some(p) => format!("{}?{}", url, p),
        };
        let response_text = match with_retry {
            true => self.send_request_with_retry(&url),
            false => self.send_request(&url),
        }?;
        Ok(serde_json::from_str(response_text.as_str())?)
    }
}
