use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "oncmp",
    about = "Compare parameters and tests between old and new Oneil runs."
)]
pub struct Args {
    /// Path to config file (default: ./oncmp_config.toml)
    #[arg(long = "config")]
    pub config_loc: Option<String>,

    /// Show only test diffs
    #[arg(short = 't', long = "tests", conflicts_with = "params_only")]
    pub tests_only: bool,

    /// Show only parameter diffs
    #[arg(short = 'p', long = "params", conflicts_with = "tests_only")]
    pub params_only: bool,

    /// Include unchanged items in output
    #[arg(short, long)]
    pub include_unchanged: bool,

    /// Print source output on parse error
    #[arg(short = 'e', long)]
    pub print_source_on_parse_error: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    All,
    Params,
    Tests,
}

impl Args {
    pub fn mode(&self) -> Mode {
        match (self.tests_only, self.params_only) {
            (true, false) => Mode::Tests,
            (false, true) => Mode::Params,
            _ => Mode::All,
        }
    }
}
