use clap::Parser;

#[derive(Parser)]
#[command(
  name = "jenkins-plugin-graph",
  about = "Build a graph of Jenkins plugins.",
)]
pub struct Cli {
  #[command(flatten)]
  pub verbosity: clap_verbosity_flag::Verbosity,
  #[arg(
    env,
    short,
    long,
    default_value = "$HOME/.cache/jenkins-plugin-graph",
    help = "Cache directory to avoid HTTP trips.",
  )]
  pub cache_dir: String,
  // TODO: Document the structure somewhere.
  #[arg(
    env,
    short,
    long,
    help = "A YAML file containing dependencies.",
    )]
  pub dependency_file: String,
  #[arg(
    short,
    long,
    help = "Print packages that are not dependencies of another package.",
    default_value_t = false
  )]
  pub root_only: bool,
}
