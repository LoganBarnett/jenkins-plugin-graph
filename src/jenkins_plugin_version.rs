// We could've used the semver, if Jenkins Plugins used semver...
// They just use... whatever?  Let's just assume it's dotted segments, and we'll
// do numeric comparisons.  That may not always be safe because some patch
// versions are like "a" and "e", but whatever.

use std::{cmp::Ordering, fmt::{Display, Formatter}};
use log::*;

use serde::{de::{self, Visitor}, Deserialize, Deserializer, Serialize, Serializer};

use crate::error::AppError;

#[derive(Debug, Clone, Eq, PartialOrd, PartialEq)]
pub struct JenkinsPluginVersion {
  pub segments: Vec<String>,
}

impl JenkinsPluginVersion {

  pub fn parse(s: &String) -> Result<Self, AppError> {
    let segments = s
      .split(".")
      .into_iter()
      .map(|s| s.to_string())
      .collect::<Vec<String>>();
    // TODO: Maybe validate more?
    if Self::segments_valid(&segments) {
      Ok(
        JenkinsPluginVersion {
          segments,
        }
      )
    } else {
      Err(AppError::VersionParseError())
    }
  }

  fn segments_valid(segments: &Vec<String>) -> bool {
    segments.len() != 0
  }

  fn numeric_segments(&self) -> Vec<u64> {
    self.segments
      .iter()
      .map(|segment| {
        match segment.parse::<u64>() {
          Ok(num) => num,
          Err(_) => {
            let (total, _) = segment
              .chars()
              .into_iter()
              .fold((0 as u64, 0 as u64), |(acc, power), num | {
                (
                  acc + (
                    num as u64 * (8 as u64).pow(power.try_into().unwrap())
                  ),
                  power + 1,
                )
              });
            total
          }
        }
      })
      .collect::<Vec<u64>>()
  }

}

impl Ord for JenkinsPluginVersion {

  fn cmp(&self, other: &Self) -> Ordering {
    self
      .numeric_segments()
      .into_iter()
      .zip(other.numeric_segments())
      .fold(Ordering::Equal, |acc, (a, b)| {
        // Essentially, continue until we hit a non-equal segment.
        if acc != Ordering::Equal {
          acc
        } else {
          a.cmp(&b)
        }
      })
  }
}

impl Display for JenkinsPluginVersion {

  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.segments.join("."))
  }

}

impl<'de> Deserialize<'de> for JenkinsPluginVersion {
  fn deserialize<D>(deserializer: D) -> Result<JenkinsPluginVersion, D::Error>
  where
    D: Deserializer<'de>,
  {
    deserializer.deserialize_string(JenkinsPluginVersionVisitor)
  }
}

struct JenkinsPluginVersionVisitor;

impl<'de> Visitor<'de> for JenkinsPluginVersionVisitor {
  type Value = JenkinsPluginVersion;

  fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
    formatter.write_str("a string with dotted segments")
  }

  fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
  where E: de::Error,
  {
    // TODO: Provide error information in the parse, and then map it along here.
    JenkinsPluginVersion::parse(&value.to_string())
      .map_err(|e| {
        error!("Somehow this is validating incorrect: {} {}", value, e);
        E::custom(format!("invalid value for JenkinsPluginVersion: {}", value))
      })
    // Ok(Self::Value {
    //   segments: value
    //     .split(".")
    //     .into_iter()
    //     .map(|s| s.to_string())
    //     .collect::<Vec<String>>()
    // })
  }

  fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
  where E: de::Error,
  {
    self.visit_str(&value)
  }

}

impl Serialize for JenkinsPluginVersion {

  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer,
  {
    serializer.serialize_str(&self.segments.join("."))
  }

}
