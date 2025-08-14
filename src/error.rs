use strum_macros::Display;

#[derive(Debug, Display)]
pub enum AppError {
  CachedArchiveMissingWarning(),
  CachedManifestReadWarning(std::io::Error),
  CachedManifestMissingWarning(),
  CliParseError(clap::error::Error),
  InputFileOpenError(std::io::Error),
  InputFileDeserializeError(serde_yaml::Error),
  FileDecodeError(std::string::FromUtf8Error, String, String),
  FileReadError(String, String, String),
  LoggingInitializationError(log::SetLoggerError),
  PackageGetCallError(String, String, String),
  PackageGetReadError(String, String, String),
  PackageUnzipError(zip::result::ZipError, String, String),
  PackageManifestSeekError(zip::result::ZipError, String, String),
  PluginArchiveWriteError(String, std::io::Error),
  PluginHashFileReadError(String, std::io::Error),
  RemotePluginDeserializeError(String),
  VersionParseError(),
  YamlSerializationError(serde_yaml::Error),
}
