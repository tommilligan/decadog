use std::iter::FromIterator;

use indexmap::IndexMap;
use scout;

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
