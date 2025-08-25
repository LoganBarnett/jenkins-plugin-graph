mod base_version;
mod cli;
mod error;
mod input;
mod logger;
mod jenkins_plugin_version;

use std::{cmp::Ordering, collections::{BTreeMap, HashMap, HashSet}, hash::Hash, io::BufReader};

use clap::Parser;
use cli::Cli;
use error::AppError;
use input::{dependency, FlatPackage, Input, ResolvedPackage, SatisfiedPackage};
use itertools::Itertools;
use log::*;
use logger::logger_init;
use serde::Serialize;
use tap::Tap;

#[derive(Serialize)]
pub struct JenkinsPuppetHashVersion {
  pub version: String,
}

fn group_by<Key, Value, F: Fn(&Value) -> Key>(
  grouping: F,
  xs: Vec<Value>,
) -> BTreeMap<Key, Vec<Value>>
  where Key: Eq, Key: Hash, Key: Ord
{
  let mut map: BTreeMap<Key, Vec<Value>> = BTreeMap::new();
  for item in xs {
    let key = grouping(&item);
    map.entry(key).or_default().push(item);
  }
  map
}

fn resolve<Key, Value, Sort: Fn(&Value, &Value) -> Ordering>(
  sort: Sort,
  grouped: BTreeMap<Key, Vec<Value>>,
) -> BTreeMap<Key, Value> where Key: Eq, Key: Hash, Key: Ord {
  let mut map: BTreeMap<Key, Value> = BTreeMap::new();
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
  let input: Input = serde_yaml::from_reader(
    BufReader::new(
      std::fs::File::open(&cli.dependency_file)
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
    .clone()
    .into_iter()
    .map(|pkg| pkg.flatten())
    .flatten()
    .collect::<Vec<FlatPackage>>()
    ;

  let grouped = group_by(
    |package| package.name.clone(),
    packages,
  );

  let resolved = resolve(
    |a, b| b.version.cmp(&a.version),
    grouped,
  );

  let filtered = if cli.root_only {
    info!("Filtering root packages...");
    let all_dependencies = flatten_dependencies(&graph);
    let filtered_dependencies = filter_referenced(&all_dependencies);
    let flattened = filtered_dependencies
      .iter()
      .map(|p| FlatPackage {
        name: p.name.clone(),
        version: p.version.clone(),
        digest_string: p.digest_string.clone(),
        digest_type: p.digest_type.clone(),
        pin: true,
      })
      .collect::<Vec<FlatPackage>>();
    resolve(
      |a, b| b.version.cmp(&a.version),
      group_by(|pkg| pkg.name.clone(), flattened),
    )
  } else {
    resolved
  };

  let mut output_helper = BTreeMap::new();
  output_helper.insert("jenkins::plugin_hash", &filtered);
  let yaml = serde_yaml::to_string(&output_helper)
    .map_err(AppError::YamlSerializationError)
    ?;
  println!("{}", yaml);
  Ok(())
}


pub fn flatten_dependencies(
  pkgs: &Vec<SatisfiedPackage>,
) -> Vec<SatisfiedPackage> {
  let mut seen = HashSet::new();
  let mut result = Vec::new();
  fn visit(
    pkgs: &Vec<SatisfiedPackage>,
    seen: &mut HashSet<String>,
    result: &mut Vec<SatisfiedPackage>,
  ) {
    for pkg in pkgs {
      for dep in &pkg.dependencies {
        if seen.insert(dep.name.clone()) {
          result.push(dep.clone());
        }
        visit(&pkg.dependencies, seen, result);
      }
    }
  }
  visit(pkgs, &mut seen, &mut result);
  result
}

fn filter_referenced(packages: &Vec<SatisfiedPackage>) -> Vec<&SatisfiedPackage> {
  let mut referenced = HashMap::new();
  for pkg in packages {
    for dep in &pkg.dependencies {
      referenced.insert(dep.name.clone(), dep.clone());
    }
  }
  packages
    .iter()
    .filter(|pkg| {
      (!referenced.contains_key(&pkg.name)).tap(|result| {
        info!("Package {} referenced? {}", pkg.name, result);
      })
    })
    .collect()
}
