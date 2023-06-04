use crate::error::{Error, ErrorKind};
use std::{thread, time};

use reqwest;
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::Value;
use std::vec;

const RESAS_ENDPOINT: &str = "https://opendata.resas-portal.go.jp";

#[derive(Debug, Deserialize)]
pub struct ResasResponse<T> {
    message: Option<String>,
    #[serde(bound(deserialize = "Vec<T>: Deserialize<'de>"))]
    pub result: Vec<T>,
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
            let mut err = match self.send_request(url) {
                Ok(r) => return Ok(r),
                Err(err) => {
                    if !err.is_retriable() {
                        return Err(err);
                    }
                    err
                }
            };
            attempts += 1;
            if attempts == self.retry_policy.attempts {
                return Err(
                    err.to_fatal(Some(format!("Retried {} but couldn't recover", attempts)))
                );
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
                        return Err(Error::new(
                            ErrorKind::Retryable,
                            None,
                            Some(response_json["message"].to_string()),
                        ));
                    }
                    if status_code.to_string().starts_with("2") {
                        return Ok(resopnse_text);
                    }
                    return Err(Error::new(
                        ErrorKind::Fatal,
                        None,
                        Some(format!(
                            "{status_code} {message}",
                            status_code = status_code,
                            message = response_json["message"],
                        )),
                    ));
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
                        return Err(Error::new(
                            ErrorKind::Retryable,
                            Some(Box::from(err)),
                            Some(format!("Status code {}", status_code)),
                        ));
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
