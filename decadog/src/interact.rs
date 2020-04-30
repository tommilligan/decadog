use std::fmt::Display;
use std::io;
use std::iter::FromIterator;

pub use dialoguer::Input;
use indexmap::IndexMap;
use snafu::Snafu;

use crate::error::Error;

/// A read-only `HashMap`, keyed by human readable description.
pub struct FuzzySelect<V> {
    lookup: IndexMap<String, V>,
}

impl<V> FuzzySelect<V> {
    pub fn interact(&self) -> Result<&V, Error> {
        let chosen_key = scout::start(self.keys(), vec![])?;
        self.get(&chosen_key).ok_or(Error::User {
            description: format!("Unknown pipeline choice '{}'", chosen_key),
        })
    }

    fn get(&self, key: &str) -> Option<&V> {
        self.lookup.get(key)
    }

    fn keys(&self) -> Vec<&str> {
        self.lookup.keys().map(|key| &**key).collect()
    }
}

impl<V> FromIterator<(String, V)> for FuzzySelect<V> {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (String, V)>,
    {
        Self {
            lookup: iter.into_iter().collect(),
        }
    }
}

pub struct Select<'a, V> {
    prompt: &'a str,
    lookup: IndexMap<String, &'a V>,
}

impl<'a, V> Select<'a, V> {
    pub fn new<I>(prompt: &'a str, iter: I) -> Result<Self, InteractError>
    where
        I: IntoIterator<Item = &'a V>,
        V: Display,
    {
        let lookup: IndexMap<_, _> = iter
            .into_iter()
            .map(|value| (format!("{}", value), value))
            .collect();
        if lookup.is_empty() {
            return Err(InteractError::Options {
                description: "Select requires at least 1 option.".to_owned(),
            });
        }
        Ok(Self { prompt, lookup })
    }

    pub fn interact(&self) -> Result<&V, Error> {
        let selection_index = dialoguer::Select::new()
            .with_prompt(self.prompt)
            .default(0)
            .items(&self.lookup.keys().collect::<Vec<&String>>())
            .interact()?;

        Ok(self
            .lookup
            .get_index(selection_index)
            .expect("Selected index out of lookup bounds.")
            .1)
    }
}

pub struct Confirm<'a> {
    confirmation: dialoguer::Confirm<'a>,
}

impl<'a> Confirm<'a> {
    pub fn new(text: &'a str) -> Self {
        let mut confirmation = dialoguer::Confirm::new();
        confirmation.with_prompt(text);

        Self { confirmation }
    }

    pub fn interact(&self) -> Result<bool, io::Error> {
        self.confirmation.interact()
    }
}

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub")]
pub enum InteractError {
    #[snafu(display("Options error: {}", description))]
    Options { description: String },
}
