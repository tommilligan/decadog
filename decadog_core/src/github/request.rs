use lazy_static::lazy_static;
use regex::Regex;
use reqwest::blocking::{RequestBuilder, Response};
use reqwest::header::LINK;
use serde::de::DeserializeOwned;
use url::Url;

use crate::error::Error;

lazy_static! {
    static ref RX_LINK_NEXT: Regex =
        Regex::new(r#"<(?P<url>[^>]+)>;[^,]* rel="next""#).expect("Invalid link regex.");
}

/// Interpret a response with potential JSON errors from the Github API.
pub trait ResponseExt {
    fn into_github<T>(self) -> Result<T, Error>
    where
        Self: Sized,
        T: DeserializeOwned;

    fn next_page_url(&self) -> Result<Option<Url>, Error>;
}

impl ResponseExt for Response {
    fn into_github<T>(self) -> Result<T, Error>
    where
        Self: Sized,
        T: DeserializeOwned,
    {
        let status = self.status();
        if status.is_success() {
            Ok(self.json()?)
        } else if status.is_client_error() {
            Err(Error::Github {
                error: self.json()?,
                status,
            })
        } else {
            Err(Error::Api {
                description: "Unexpected response status code.".to_owned(),
                status,
            })
        }
    }

    fn next_page_url(&self) -> Result<Option<Url>, Error> {
        // TODO(tommilligan) Find an actual Link header parsing implementation
        match self.headers().get(LINK) {
            None => Ok(None),
            Some(header_value) => {
                match RX_LINK_NEXT.captures(header_value.to_str().map_err(|_| Error::Unknown {
                    description: "Expected Github Link header to be valid.".to_owned(),
                })?) {
                    None => Ok(None),
                    Some(captured_groups) => Ok(Some(Url::parse(&captured_groups["url"])?)),
                }
            }
        }
    }
}

/// Send a HTTP request to Github, and return the resulting struct.
pub trait RequestBuilderExt {
    fn send_github<T>(self) -> Result<T, Error>
    where
        Self: Sized,
        T: DeserializeOwned;
}

impl RequestBuilderExt for RequestBuilder {
    fn send_github<T>(self) -> Result<T, Error>
    where
        Self: Sized,
        T: DeserializeOwned,
    {
        let response = self.send()?;
        response.into_github()
    }
}
