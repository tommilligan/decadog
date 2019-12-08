use std::io;
use std::iter::FromIterator;

use dialoguer;
pub use dialoguer::Input;
use indexmap::IndexMap;
use scout;
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
    lookup: IndexMap<&'a String, &'a V>,
}

impl<'a, V> Select<'a, V> {
    pub fn new<I>(prompt: &'a str, iter: I) -> Result<Self, InteractError>
    where
        I: IntoIterator<Item = (&'a String, &'a V)>,
    {
        let lookup: IndexMap<_, _> = iter.into_iter().collect();
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
            .items(&self.lookup.keys().cloned().collect::<Vec<&String>>())
            .interact()?;

        Ok(self
            .lookup
            .get_index(selection_index)
            .expect("Selected index out of lookup bounds.")
            .1)
    }
}

pub struct Confirmation<'a> {
    confirmation: dialoguer::Confirmation<'a>,
}

impl<'a> Confirmation<'a> {
    pub fn new(text: &'a str) -> Self {
        let mut confirmation = dialoguer::Confirmation::new();
        confirmation.with_text(text);

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
