use std::env::temp_dir;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, exit};

use clap::{Parser, Subcommand};
use compiler::CompilerProcess;
use diagnostic::ChsResult;

const BIN_NAME: &str = env!("CARGO_BIN_NAME");
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ChsResult<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Build {
            paths,
            verbose,
            output,
            search_path,
            optimization_level,
            features,
        } => {
            let mut cp = CompilerProcess::default();
            cp.add_default_search_paths()?;
            for path in paths {
                cp.add_source(path)?;
            }
            for sp in search_path {
                cp.add_search_path(sp)?;
            }
            for f in features {
                cp.add_feature(f);
            }
            cp.set_verbose(verbose);
            cp.set_target_name(output);
            match optimization_level {
                '1' => cp.set_optimization_level(compiler::OptimizationLevel::O1),
                '2' => cp.set_optimization_level(compiler::OptimizationLevel::O2),
                '3' => cp.set_optimization_level(compiler::OptimizationLevel::O3),
                _ => {}
            }

            cp.compile()?;
        }
        Commands::Run {
            paths,
            search_path,
            features,
        } => {
            let mut cp = CompilerProcess::default();
            cp.add_default_search_paths()?;
            let target_name = temp_dir().join(format!("chs_{}", rand::random::<u32>()));
            cp.set_target_name(target_name);
            for path in paths {
                cp.add_source(path)?;
            }
            for sp in search_path {
                cp.add_search_path(sp)?;
            }
            for f in features {
                cp.add_feature(f);
            }
            cp.compile()?;
            let status = Command::new(cp.target_name()).status()?;
            if status.success() {
                exit(0);
            } else {
                exit(1);
            }
        }
        Commands::Clear => {
            _ = fs::remove_dir_all(".build");
        }
        Commands::Version => {
            println!("version: {BIN_NAME}-{PKG_VERSION}",);
        }
    }
    Ok(())
}

#[derive(Parser)]
#[command(name = "chs")]
#[command(about = "Chs managing tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile mdoule and dependencies
    #[command(visible_alias = "b")]
    Build {
        paths: Vec<PathBuf>,
        #[arg(short)]
        verbose: bool,
        #[arg(short, long, default_value = "out")]
        output: PathBuf,
        #[arg(short = 'S', long)]
        search_path: Vec<PathBuf>,
        #[arg(short = 'O', default_value = "0")]
        optimization_level: char,
        #[arg(short = 'F', long)]
        features: Vec<String>,
    },
    /// Compile and run program
    #[command(visible_alias = "r")]
    Run {
        paths: Vec<PathBuf>,
        #[arg(short = 'S', long)]
        search_path: Vec<PathBuf>,
        #[arg(short = 'F', long)]
        features: Vec<String>,
    },
    /// Clear the project artifacts
    Clear,
    /// Print version
    #[command(version)]
    Version,
}
