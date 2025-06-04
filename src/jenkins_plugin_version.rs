// We could've used the semver, if Jenkins Plugins used semver...
// They just use... whatever?  Let's just assume it's dotted segments, and we'll
// do numeric comparisons.  That may not always be safe because some patch
// versions are like "a" and "e", but whatever.

use std::fmt::Formatter;

use serde::{de::{self, Visitor}, Deserialize, Deserializer, Serialize, Serializer};

pub struct JenkinsPluginVersion {
  pub segments: Vec<String>,
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

  fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
  where E: de::Error,
  {
    Ok(Self::Value {
      segments: value
        .split(".")
        .into_iter()
        .map(|s| s.to_string())
        .collect::<Vec<String>>()
    })
  }

}

impl Serialize for JenkinsPluginVersion {

  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer,
  {
    serializer.serialize_str(&self.segments.join("."))
  }

}
