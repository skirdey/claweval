mod backend;
mod checks;
mod judge;
mod jsonschema;
mod printer;
mod report;
mod runner;
mod spec;
mod stats;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "claweval", version, about = "ClawEval: composable evals for long-running agentic assistants")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run a suite JSON and write a report JSON
    Run {
        /// Path to suite JSON
        suite: PathBuf,

        /// Output report JSON file
        #[arg(long)]
        out: Option<PathBuf>,

        /// Override global repeats (multiplies episode repeats)
        #[arg(long)]
        repeats: Option<u32>,

        /// Override backend type in suite (openclaw|command|http|openai)
        #[arg(long)]
        backend: Option<String>,

        /// Override OpenClaw binary path
        #[arg(long)]
        openclaw: Option<String>,

        /// Force OpenClaw --local
        #[arg(long)]
        local: bool,

        /// OpenClaw profile name (isolates state under ~/.openclaw-<profile>)
        #[arg(long)]
        profile: Option<String>,

        /// Enable LLM-judge checks (otherwise they are treated as pass)
        #[arg(long, default_value_t = false)]
        enable_llm_judge: bool,

        /// Only run episodes whose id matches this glob pattern (e.g. "*memory*")
        #[arg(long)]
        filter: Option<String>,

        /// Number of parallel episode workers (default: 1)
        #[arg(long, default_value_t = 1)]
        jobs: u32,

        /// Verbose logging
        #[arg(long, default_value_t = false)]
        verbose: bool,
    },

    /// Parse and validate a suite JSON, then exit 0 (valid) or 1 (invalid)
    Validate {
        /// Path to suite JSON
        suite: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            suite,
            out,
            repeats,
            backend,
            openclaw,
            local,
            profile,
            enable_llm_judge,
            filter,
            jobs,
            verbose,
        } => {
            let mut suite_spec = spec::SuiteSpec::from_path(&suite)?;

            // CLI overrides
            if let Some(r) = repeats {
                suite_spec.global_repeats = Some(r);
            }
            if let Some(bt) = backend {
                suite_spec.backend.backend_type = bt;
            }
            if let Some(p) = profile {
                suite_spec.backend.profile = Some(p);
            }
            if local {
                suite_spec.backend.local = Some(true);
            }
            if let Some(bin) = openclaw {
                suite_spec.backend.openclaw_bin = Some(bin);
            }

            let report = runner::run_suite(
                &suite_spec,
                runner::RunOptions {
                    enable_llm_judge,
                    verbose,
                    filter,
                    jobs,
                },
            )?;

            if let Some(out_path) = out {
                std::fs::write(&out_path, serde_json::to_string_pretty(&report)?)?;
                eprintln!("Wrote report to {}", out_path.display());
            } else {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }

        Commands::Validate { suite } => {
            match spec::SuiteSpec::from_path(&suite) {
                Ok(s) => {
                    eprintln!(
                        "OK: \"{}\" — {} episode(s)",
                        s.name,
                        s.episodes.len()
                    );
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("ERROR: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}
