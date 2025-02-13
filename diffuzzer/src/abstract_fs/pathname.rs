/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::fmt::Display;

use serde::{Deserialize, Serialize};

/// Represents an abstract filesystem path.
#[derive(Debug, Clone, Hash, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct PathName(String);

impl Display for PathName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for PathName {
    fn from(value: &str) -> Self {
        PathName(value.to_owned())
    }
}

impl From<String> for PathName {
    fn from(value: String) -> Self {
        PathName(value)
    }
}

impl PathName {
    /// Splits path into parent directory path and file name.
    pub fn split(&self) -> (PathName, Name) {
        let split_at = self.0.rfind('/').unwrap();
        let (parent, name) = (&self.0[..split_at], &self.0[split_at + 1..]);
        if parent.is_empty() {
            ("/".into(), name.to_owned())
        } else {
            (parent.into(), name.to_owned())
        }
    }

    pub fn segments(&self) -> Vec<&str> {
        self.0.split("/").filter(|s| !s.is_empty()).collect()
    }

    pub fn join(&self, name: Name) -> PathName {
        if self.is_root() {
            format!("/{}", name).into()
        } else {
            format!("{}/{}", self.0, name).into()
        }
    }

    pub fn is_valid(&self) -> bool {
        !(self.0.is_empty()
            || !self.0.starts_with('/')
            || (!self.is_root() && self.0.ends_with('/')))
    }

    pub fn is_root(&self) -> bool {
        self.0 == "/"
    }

    pub fn is_prefix_of(&self, other: &PathName) -> bool {
        let segments = self.segments();
        let other_segments = other.segments();
        if other_segments.len() < segments.len() {
            return false;
        } else {
            for i in 0..segments.len() {
                if segments[i] != other_segments[i] {
                    return false;
                }
            }
            return true;
        }
    }
}

/// Abstract filesystem file name.
pub type Name = String;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_prefix_of() {
        assert!(PathName::from("/1/2").is_prefix_of(&PathName::from("/1/2/3")));
        assert!(!PathName::from("/1/2").is_prefix_of(&PathName::from("/1/20/3")));
        assert!(!PathName::from("/1/2").is_prefix_of(&PathName::from("/1")));
        assert!(PathName::from("/").is_prefix_of(&PathName::from("/1")));
        assert!(PathName::from("/1").is_prefix_of(&PathName::from("/1")));
    }
}
