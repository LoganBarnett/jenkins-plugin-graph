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
  // pub jenkins_version: String,
}
