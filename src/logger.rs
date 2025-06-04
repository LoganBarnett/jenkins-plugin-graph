use crate::{error, AppError};
use clap_verbosity_flag::Verbosity;
use log::*;

pub fn logger_init(verbosity: &Verbosity) -> Result<(), error::AppError> {
  let mut logger = stderrlog::new();
  logger
    .verbosity(verbosity.log_level().unwrap_or(Level::Info))
    .init()
    .map_err(AppError::LoggingInitializationError)?;
  info!("Setup up logger with verbosity {}.", verbosity);
  Ok(())
}
