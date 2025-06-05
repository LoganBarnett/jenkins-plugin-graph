mod cli;
mod error;
mod input;
mod logger;
mod jenkins_plugin_version;

use std::{cmp::Ordering, collections::HashMap, hash::Hash, io::BufReader};

use clap::Parser;
use cli::Cli;
use error::AppError;
use input::{dependency, FlatPackage, Input, ResolvedPackage, SatisfiedPackage};
use itertools::Itertools;
use log::*;
use logger::logger_init;
use serde::Serialize;

#[derive(Serialize)]
pub struct JenkinsPuppetHashVersion {
  pub version: String,
}

fn group_by<Key, Value, F: Fn(&Value) -> Key>(
  grouping: F,
  xs: Vec<Value>,
) -> HashMap<Key, Vec<Value>>
  where Key: Eq, Key: Hash
{
  let mut map: HashMap<Key, Vec<Value>> = HashMap::new();
  for item in xs {
    let key = grouping(&item);
    map.entry(key).or_default().push(item);
  }
  map
}

fn resolve<Key, Value, Sort: Fn(&Value, &Value) -> Ordering>(
  sort: Sort,
  grouped: HashMap<Key, Vec<Value>>,
) -> HashMap<Key, Value> where Key: Eq, Key: Hash {
  let mut map: HashMap<Key, Value> = HashMap::new();
  for (key, values) in grouped {
    let val = values
      .into_iter()
      .sorted_by(&sort)
      .nth(0)
      ;
    match val {
      Some(value) => { map.insert(key, value); },
      None => unreachable!(),
    }
  }
  map
}

fn main() -> Result<(), AppError> {
  let cli = Cli::parse();
  logger_init(&cli.verbosity)?;
  let cache_dir = match dirs_next::home_dir() {
    Some(home_dir) => {
      let dir = cli.cache_dir.replace("$HOME", home_dir.to_str().unwrap());
      match std::fs::create_dir_all(&dir) {
        Ok(_) => (),
        Err(e) => warn!(
          "Could not create supplied --cache-dir '{}'.  Error: {}",
          &dir,
          e,
        ),
      };
      dir
    },
    // TODO: Check to see if we need the home directory first.
    None => {
      warn!("No home directory!  What do we do here?");
      cli.cache_dir
    },
  };
  let path = "input.yaml";
  let input: Input = serde_yaml::from_reader(
    BufReader::new(
      std::fs::File::open(path)
        .map_err(AppError::InputFileOpenError)
        ?
    )
  )
    .map_err(AppError::InputFileDeserializeError)
    ?;
  let specified_dependencies = input
    .plugins_hash
    .iter()
    .map(|(name, package)| {
      ResolvedPackage {
        name: name.to_string(),
        version: package.version.clone(),
      }
    })
    .collect();
  // Take the inputs and request them.
  // Then take the dependencies from that list and request those.
  // Keep going until there are no more unsatisfied dependencies.
  let graph = input
    .plugins_hash
    .into_iter()
    .map(|(name, package)| {
      // match package.version_constraint {
      //   VersionConstraint::Exact(v) => dependency(name, v.version),
      // }
      dependency(
        &specified_dependencies,
        cache_dir.clone(),
        name,
        &package.version,
      )
    })
    .collect::<Result<Vec<SatisfiedPackage>, AppError>>()
    ?;
  let packages = graph
    .into_iter()
    .map(|p| p.flatten())
    .flatten()
    .collect::<Vec<FlatPackage>>()
    // .map(|p| {
    //   (p.name, JenkinsPuppetHashVersion { version: p.version, })
    // })
    // .collect::<HashMap<String, JenkinsPuppetHashVersion>>()
    ;

  let grouped = group_by(
    |package| package.name.clone(),
    packages,
  );

  let resolved = resolve(
    |a, b| a.version.cmp(&b.version),
    grouped,
  );

  let yaml = serde_yaml::to_string(&resolved)
    .map_err(AppError::YamlSerializationError)
    ?;
  println!("{}", yaml);
  Ok(())
}
