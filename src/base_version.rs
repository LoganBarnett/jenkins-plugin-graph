use std::{cmp::Ordering, fmt::{Display, Formatter}};

use crate::error::AppError;

// The beginning segment of the Jenkins Plugin Version is a dot-segmented
// version string.  It doesn't necessarily follow SemVer so we need a more
// flexible approach.
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd)]
pub struct BaseVersion {
  pub segments: Vec<String>,
}

impl BaseVersion {

  pub fn parse<S: AsRef<str>>(s: S) -> Result<Self, AppError> {
    let s = s.as_ref();
    let segments = s
      .split(".")
      .into_iter()
      .map(|s| s.to_string())
      .collect::<Vec<String>>();
    // TODO: Maybe validate more?
    if Self::segments_valid(&segments) {
      Ok(BaseVersion { segments })
    } else {
      Err(AppError::VersionParseError(format!(
        "Segments are not valid: {}",
        segments.join("."),
      )))
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

impl Ord for BaseVersion {

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

impl Display for BaseVersion {

  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.segments.join("."))
  }

}
