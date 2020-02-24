use std::vec::IntoIter;

use log::debug;
use reqwest::blocking::{Client as ReqwestClient, Request, Response};
use reqwest::header::HeaderMap;
use serde::de::DeserializeOwned;
use serde_derive::{Deserialize, Serialize};
use url::Url;

use crate::error::Error;

use super::request::ResponseExt;

/// A single page from the Github search API.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct GithubSearchResults<T> {
    pub incomplete_results: bool,
    pub items: Vec<T>,
}

/// Represents a paginated search query over some collection of items `T`.
///
/// Used as an iterator, the `PaginatedSearch` will continue to fetch more results
/// until no more are available.
pub struct PaginatedSearch<'a, T>
where
    Self: Sized,
    T: DeserializeOwned,
{
    client: &'a ReqwestClient,
    headers: HeaderMap,
    page: IntoIter<T>,
    next_page_url: Option<Url>,
}

impl<'a, T> PaginatedSearch<'a, T>
where
    Self: Sized,
    T: DeserializeOwned,
{
    /// Create a new paginated search, and load the first page.
    pub fn new(client: &'a ReqwestClient, initial_request: Request) -> Result<Self, Error> {
        // The initial request is a special case
        // Copy the headers out first to use them for auth on subsequent requests
        let headers = initial_request.headers().clone();
        debug!("{} {}", initial_request.method(), initial_request.url());
        let response = client.execute(initial_request)?;

        // Apply our intial response to an empty struct
        let mut new_self = Self {
            client,
            headers,
            page: vec![].into_iter(),
            next_page_url: None,
        };
        new_self.apply_response(response)?;

        // From this state, we can continue to generate and execute new resopnses
        Ok(new_self)
    }

    /// Apply a response from the search API to update our state:
    /// - store the new items to iterate throught
    /// - extract and store the url for the next page
    fn apply_response(&mut self, response: Response) -> Result<(), Error> {
        self.next_page_url = response.next_page_url()?;
        self.page = response
            .into_github::<GithubSearchResults<T>>()?
            .items
            .into_iter();
        Ok(())
    }

    /// Fetch the next page, and apply the response to our state.
    fn update_page(&mut self, url: Url) -> Result<(), Error> {
        debug!("GET {}", &url);
        let request = self.client.get(url).headers(self.headers.clone()).build()?;
        let response = self.client.execute(request)?;
        self.apply_response(response)?;
        Ok(())
    }
}

impl<'a, T> Iterator for PaginatedSearch<'a, T>
where
    Self: Sized,
    T: DeserializeOwned,
{
    type Item = Result<T, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.page.next() {
            // if we still have the current page, iterate it
            Some(item) => Some(Ok(item)),
            // otherwise, get another page
            None => match self.next_page_url.clone() {
                None => None,
                Some(url) => match self.update_page(url) {
                    Err(e) => Some(Err(e)),
                    Ok(_) => self.page.next().map(Ok),
                },
            },
        }
    }
}

#[cfg(test)]
pub mod tests {
    use mockito::mock;
    use pretty_assertions::assert_eq;
    use serde_derive::{Deserialize, Serialize};

    use super::*;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestItem {
        data: u8,
    }

    #[test]
    fn test_paginated_search() {
        let page_one_path = "/url-for-page-one";
        let page_two_path = "/url-for-page-two";

        // First page should be fetched immediately
        let client = ReqwestClient::new();
        let initial_request = client
            .get(&format!("{}{}", &mockito::server_url(), &page_one_path))
            .build()
            .unwrap();
        let mock_page_one = mock("GET", page_one_path)
            // return the second page url as a link
            .with_header(
                "link",
                &format!(
                    r#"<{}{}>; rel="next""#,
                    &mockito::server_url(),
                    &page_two_path
                ),
            )
            .with_body(
                r#"{
  "incomplete_results": false,
  "items": [
    {
      "data": 0
    },
    {
      "data": 1
    }
  ]
}"#,
            )
            .create();

        let mut paginated_items =
            PaginatedSearch::<TestItem>::new(&client, initial_request).unwrap();

        mock_page_one.assert();
        assert_eq!(
            paginated_items.next().unwrap().unwrap(),
            TestItem { data: 0 }
        );
        assert_eq!(
            paginated_items.next().unwrap().unwrap(),
            TestItem { data: 1 }
        );

        // When we run out of items, we should fetch the next page
        let mock_page_two = mock("GET", page_two_path)
            .with_body(
                r#"{
  "incomplete_results": false,
  "items": [
    {
      "data": 2
    }
  ]
}"#,
            )
            .create();
        assert_eq!(
            paginated_items.next().unwrap().unwrap(),
            TestItem { data: 2 }
        );
        mock_page_two.assert();

        // As the last page didn't have a link, the next issue should be None
        assert!(paginated_items.next().is_none());
    }
}
