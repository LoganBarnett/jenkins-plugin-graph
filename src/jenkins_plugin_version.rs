#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_version_comparison_patch_and_no_patch() {

    let version_a = JenkinsPluginVersion::parse("9.7-33.v4d23ef79fcc8").unwrap();
    let version_b = JenkinsPluginVersion::parse("9.7.1-97.v4cc844130d97").unwrap();
    assert!(version_a < version_b);
  }

  #[test]
  fn test_version_comparison_majors_only() {
    let version_a = JenkinsPluginVersion::parse("625.vd896b_f445a_f8").unwrap();
    let version_b = JenkinsPluginVersion::parse("639.v6eca_cd8c04a_a_").unwrap();
    assert!(version_a < version_b);
    let version_a = JenkinsPluginVersion::parse("1873.vea_5814ca_9c93").unwrap();
    let version_b = JenkinsPluginVersion::parse("1958.vddc0d369b_e16").unwrap();
    assert!(version_a < version_b);
  }

  #[test]
  fn test_parse_without_build_number() {
    JenkinsPluginVersion::parse("2.1240.vca_710512d944").unwrap();
  }

  #[test]
  fn version_round_trip() {
    let versions = [
      "392.v27a_482d90083",
    ];
    for version in versions {
      let jpv = JenkinsPluginVersion::parse(version).unwrap();
      assert_eq!(jpv.to_string(), version);
    }
  }

}
// They just use... whatever?  Let's just assume it's dotted segments, and we'll
// do numeric comparisons.  That may not always be safe because some patch
// versions are like "a" and "e", but whatever.

use std::{cmp::Ordering, fmt::{Display, Formatter}};
use log::*;

use serde::{de::{self, Visitor}, Deserialize, Deserializer, Serialize, Serializer};

use crate::{base_version::BaseVersion, error::AppError};

#[derive(Debug, Clone, Eq, PartialOrd, PartialEq)]
pub struct JenkinsPluginVersion {
  pub base_version: BaseVersion,
  pub build_number: Option<usize>,
  pub git_hash: Option<String>,
}

impl JenkinsPluginVersion {

  pub fn parse<S: AsRef<str>>(s: S) -> Result<Self, AppError> {
    let parts: Vec<&str> = s.as_ref().split('-').collect();
    let hashless_parts: Vec<&str> = parts[0].split(".v").collect();
    let base_version = BaseVersion::parse(hashless_parts[0])?;
    let build_number = if parts.len() == 2 {
      let build_parts: Vec<&str> = parts[1].split(".v").collect();
      Some(
        build_parts[0]
          .parse::<usize>()
          .map_err(|e| {
            AppError::VersionParseError(format!(
              "Invalid build number in {}: {} - {}",
              build_parts[0],
              s.as_ref(),
              e,
            ))
          })?,
      )
    } else {
      None
    };
    let git_hash_parts: Vec<&str> = s.as_ref().split(".v").collect();
    let git_hash: Option<String> = if git_hash_parts.len() == 2 {
      Some(git_hash_parts[1].to_string())
    } else {
      None
    };
    Ok(JenkinsPluginVersion {
      base_version,
      build_number,
      git_hash,
    })
  }

}

impl Ord for JenkinsPluginVersion {

  fn cmp(&self, other: &Self) -> Ordering {
    match self.base_version.cmp(&other.base_version) {
      Ordering::Equal => self.build_number.cmp(&other.build_number),
      ord => ord,
    }
  }


}

impl Display for JenkinsPluginVersion {

  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_str(format!(
      "{}{}{}",
      self.base_version,
      self.build_number.clone().map(|s| format!("-{}", s)).unwrap_or("".into()),
      self.git_hash.clone().map(|s| format!(".v{}", s)).unwrap_or("".into()),
    ).as_str())
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
    formatter.write_str(
      "a string of format <version>-<build-number>.v<git-hash>",
    )
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
    serializer.serialize_str(&self.to_string())
  }

}
