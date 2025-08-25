use crate::{cli::Cli, error::AppError, jenkins_plugin_version::JenkinsPluginVersion};
use bytes::Bytes;
use log::*;
use reqwest::blocking;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fs::File, io::{Cursor, Read}};
use serde::{Deserialize, Serialize};
use cached::proc_macro::cached;
use cached::SizedCache;
use regex::Regex;
use std::io::Write;

#[derive(Clone, Debug, Deserialize)]
pub struct Input {
  pub plugins_hash: HashMap<String, InputPackage>,
}

// The input package is what is desired from our input file or CLI arguments.
#[derive(Clone, Debug, Deserialize)]
pub struct InputPackage {
  // TODO: Consider making this a constraint.
  pub version: JenkinsPluginVersion,
}

// #[derive(Clone, Debug, Deserialize)]
// pub enum VersionConstraint {
//   Exact(VersionConstraintExact),
//   // Latest(VersionConstraintLatest),
//   // Between(VersionConstraintBetween),
// }

// #[derive(Clone, Debug, Deserialize)]
// pub struct VersionConstraintExact {
//   pub version: String,
// }

// #[derive(Clone, Debug, Deserialize)]
// pub struct VersionConstraintLatest { }

// #[derive(Clone, Debug, Deserialize)]
// pub struct VersionConstraintBetween {
//   pub version_upper_bound: String,
//   pub version_lower_bound: String,
// }

// A resolved package is a transient structure that shows us what we found, but
// doesn't include its dependencies and thus is incomplete.
pub struct ResolvedPackage {
  pub name: String,
  pub version: JenkinsPluginVersion,
}

// A satisfied package is a package that has been completely resolved as well as
// all of its dependents.
#[derive(Clone, Debug)]
pub struct SatisfiedPackage {
  pub name: String,
  pub version: JenkinsPluginVersion,
  pub dependencies: Vec<SatisfiedPackage>,
  pub digest_string: String,
  pub digest_type: String,
}

impl Eq for SatisfiedPackage {}

impl PartialEq for SatisfiedPackage {
  fn eq(&self, other: &Self) -> bool {
    self.name == other.name
  }
}

#[derive(Clone, Debug, Serialize)]
pub struct FlatPackage {
  pub name: String,
  pub version: JenkinsPluginVersion,
  pub digest_string: String,
  pub digest_type: String,
  pub pin: bool,
}

impl PartialEq for FlatPackage {
  fn eq(&self, other: &Self) -> bool {
    self.name == other.name
  }
}

impl Eq for FlatPackage {}

impl Ord for FlatPackage {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.name.cmp(&other.name)
  }
}

impl PartialOrd for FlatPackage {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}

impl SatisfiedPackage {

  pub fn flatten(&self) -> Vec<FlatPackage> {
    let mut packages = self
      .dependencies
      .clone()
      .into_iter()
      .map(|d| d.flatten())
      .flatten()
      .collect::<Vec<FlatPackage>>();
    packages.push(FlatPackage {
      name: self.name.clone(),
      version: self.version.clone(),
      digest_string: self.digest_string.clone(),
      digest_type: self.digest_type.clone(),
      pin: true,
    });
    packages.sort_by(|a, b| a.name.cmp(&b.name));
    packages
  }

}

fn archive_cache_path(
  cache_dir: &String,
  name: &String,
  version: &JenkinsPluginVersion,
) -> String {
  format!("{}/{}--{}.hpi", cache_dir, name, version)
}

fn archive_hash_file(
  cache_dir: &String,
  name: &String,
  version: &JenkinsPluginVersion,
) -> Result<(String, String), AppError> {
  let archive_path = archive_cache_path(&cache_dir, &name, &version);
  let mut file = File::open(&archive_path)
    .map_err(|e| AppError::PluginHashFileReadError(archive_path.clone(), e) )
    ?;
  let mut buffer = Vec::new();
  file.read_to_end(&mut buffer)
    .map_err(|e| AppError::PluginHashFileReadError(archive_path.clone(), e) )
    ?;
  archive_hash_bytes(&buffer.into())
}

fn archive_hash_bytes(
  buffer: &Bytes,
) -> Result<(String, String), AppError> {
  let mut hasher = Sha256::new();
  hasher.update(&buffer);
  let digest = hasher.finalize();
  Ok((format!("{:x}", digest), "sha256".to_string()))
}

fn archive_write(
  cache_dir: &String,
  name: &String,
  version: &JenkinsPluginVersion,
  bytes: &Bytes,
) -> Result<(), AppError> {
  let archive_path = archive_cache_path(&&cache_dir, name, &version);
  let mut file = File::create(&archive_path)
    .map_err(|e| {
      AppError::PluginArchiveWriteError(archive_path.clone(), e)
    })?;
  file.write_all(&bytes)
    .map_err(|e| {
      AppError::PluginArchiveWriteError(archive_path.clone(), e)
    })?;
  debug!("Wrote archive to: {}", archive_path);
  Ok(())
}

// TODO: Ugh I did all of this and only later found there's a DiskCache in
// cached.  Take a look!
pub fn dependency_http(
  cache_dir: String,
  name: String,
  version: JenkinsPluginVersion,
) -> Result<String, AppError> {
  let url = format!(
    "https://get.jenkins.io/plugins/{}/{}/{}.hpi",
    name,
    version,
    name,
  );
  debug!("Trying url: {}", url);
  let response = blocking::get(url)
    .map_err(|e| AppError::PackageGetCallError(
      e.to_string(),
      name.clone(),
      version.to_string(),
    ))
    ?;
  debug!("Response for {}: {}", name, response.status());
  let bytes = response.bytes()
    .map_err(|e| AppError::PackageGetReadError(
      e.to_string(),
      name.clone(),
      version.to_string(),
    ))
    ?;
  // Bytes::clone doesn't actually make a copy but clones a reference.  You want
  // to_vec for strict copies, unintuitively.
  archive_write(&cache_dir, &name, &version, &bytes.clone())?;
  let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
    .map_err(|e| AppError::PackageUnzipError(
      e,
      name.clone(),
      version.to_string(),
    ))
    ?;
  archive
    .by_name("META-INF/MANIFEST.MF")
    .map_err(|e| AppError::PackageManifestSeekError(e, name.clone(), version.to_string()))
    // .map(|zip_file| zip_file.bytes())
    .and_then(|mut zipped_file| {
      let mut buffer = Vec::new();
      zipped_file.read_to_end(&mut buffer)
        .map_err(|e| {
          AppError::FileReadError(
            e.to_string(),
            name.clone(),
            version.to_string(),
          )
        })
        ?;
      String::from_utf8(buffer)
        .map_err(|e| {
          AppError::FileDecodeError(e, name.clone(), version.to_string())
        })
        .map(|manifest| {
          // Fun fact: MANIFEST.MF files can wrap lines with values.  To do
          // this, prefix the next line with a single space.  There might be
          // more to it, but this is observed only.  We don't care about
          // preserving the formatting, so just strip the space and the prior
          // line ending.
          manifest
            // Unknown if these are optional.  Just strip them.  Not doing so
            // consistently fouls up the next replace.
            .replace("\r", "")
            .replace("\n ", "")
          // manifest
        })
        .inspect(|manifest| debug!("Manifest for {}:\n{}", name, manifest) )
        .inspect(|manifest| {
          let path = format!("{}/{}--{}.mf", cache_dir, name, version);
          let file_res = File::create(&path);
          match file_res {
            Ok(mut file) => {
              let write_res = write!(file, "{}", manifest);
              match write_res {
                Ok(_) => (),
                Err(e) => warn!(
                  "Error writing {}.  Non-panic error due to caching nature.  Error: {}",
                  path,
                  e,
                )
              }
            },
            Err(e) => warn!(
              "Error writing {}.  Non-panic error due to caching nature.  Error: {}",
              path,
              e,
            )
          }
        })
    })
}

pub fn cached_manifest(
  cache_dir: String,
  name: String,
  version: JenkinsPluginVersion,
) -> Result<String, AppError> {
  let manifest_path = format!("{}/{}--{}.mf", cache_dir, name, version);
  let archive_path = archive_cache_path(&cache_dir, &name, &version);
  if std::fs::exists(&manifest_path).unwrap() {
    if std::fs::exists(&archive_path).unwrap() {
      std::fs::read_to_string(&manifest_path)
        .inspect(|_| debug!("Found {} in cache.", manifest_path))
        .map_err(AppError::CachedManifestReadWarning)
    } else {
      warn!("Manifest is present, but {} archive is missing.", archive_path);
      Err(AppError::CachedArchiveMissingWarning())
    }
  } else {
    Err(AppError::CachedManifestMissingWarning())
  }
}

// Actually since I have manual disk caching implemented, I can just disable
// this.  This should be disabled until I can figure out how to handle fancy
// arguments and borrows.
// #[cached(
//   ty = "SizedCache<String, usize>",
//   create = "{ SizedCache::with_size(100) }",
//   convert = r#"{ format!("{}-{}", name, version) }"#,
//   result = true,
// )]
pub fn dependency(
  specified: &Vec<ResolvedPackage>,
  cache_dir: String,
  name: String,
  version: &JenkinsPluginVersion,
) -> Result<SatisfiedPackage, AppError> {
  let real_version = specified
    .into_iter()
    .find(|p| p.name == name)
    .map(|p| p.version.clone())
    .unwrap_or(version.clone());
  // This is said to "move" the variable, but I don't see its effect.
  let _ = version;
  let dependencies = cached_manifest(
    cache_dir.clone(),
    name.clone(),
    real_version.clone(),
  )
    .or_else(|_| { dependency_http(
      cache_dir.clone(),
      name.clone(),
      real_version.clone(),
    ) })
    .and_then(parse_dependencies)
    .and_then(|deps| {
      deps
        .into_iter()
        .map(|dep| {
          dependency(&specified, cache_dir.clone(), dep.name, &dep.version)
        })
        .collect()
    })?;
  let (digest_string, digest_type) = archive_hash_file(
    &cache_dir,
    &name,
    &real_version,
  )?;
  Ok(SatisfiedPackage {
    name,
    version: real_version,
    dependencies,
    digest_string,
    digest_type,
  })
}

fn _dependency_latest(name: String) {
  let _ = format!("https://updates.jenkins-ci.org/latest/{}.hpi", name);
}

/**
 * The MANIFEST.MF file is a line-break separated file, wit keys, colons, and
 * values.  You can see the format here:
 * https://wiki.jenkins.io/display/JENKINS/Plugin+Structure
 * We need to look for all lines that start with "Plugin-Dependencies: ", and
 * read the comma separated list of <name>:<version>.  It is not known if the
 * version can be a range.  There might be multiple Plugin-Dependencies lines.
 */
fn parse_dependencies(manifest: String) -> Result<Vec<ResolvedPackage>, AppError> {
  manifest
    .split("\n")
    .into_iter()
    .filter(|line| line.starts_with("Plugin-Dependencies:") )
    .map(|line| {
      let matches = Regex::new(r"Plugin-Dependencies: ?(.*)$")
        .unwrap()
        .captures(line);
      match matches {
        Some(m) => {
          match m.get(1) {
            Some(s) => s
              .as_str()
              .split(",")
              .map(from_name_version_string)
              .collect(),
            None => vec!(),
          }
        },
        None => vec!(),
      }
    })
    .flatten()
    .collect()
}

fn from_name_version_string(plugin_pair: &str) -> Result<ResolvedPackage, AppError> {
  let [ name, version_and_resolution ] = TryInto::<[String; 2]>::try_into(
    plugin_pair
      .split_once(":")
      .into_iter()
      .collect::<(String, String)>()
  )
    .map_err(|_| AppError::RemotePluginDeserializeError(plugin_pair.into()))
    ?;
  // Versions can be split further into a resolution type.  We don't care about
  // that right now.
  let rust_weakness = version_and_resolution
    .split(";")
    .collect::<Vec<&str>>()
    ;
  // I don't know how this unwrap could fail.  I don't feel like fighting you
  // today, Rust.
  let version = rust_weakness
    .get(0)
    .unwrap()
    ;
  Ok(ResolvedPackage {
    name: name.to_string(),
    version: JenkinsPluginVersion::parse(&version.to_string())?,
  })
}
