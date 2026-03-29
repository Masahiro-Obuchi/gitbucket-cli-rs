use std::io;

use clap::{Args, CommandFactory, ValueEnum};
use clap_complete::{
    generate,
    shells::{Bash, Fish, PowerShell, Zsh},
};

use crate::error::Result;

use super::Cli;

#[derive(Args)]
pub struct CompletionArgs {
    /// Shell to generate completion for
    #[arg(value_enum)]
    shell: CompletionShell,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    #[value(name = "powershell")]
    PowerShell,
}

pub async fn run(args: CompletionArgs) -> Result<()> {
    let mut command = Cli::command();
    let bin_name = command.get_name().to_string();
    let mut stdout = io::stdout();

    match args.shell {
        CompletionShell::Bash => generate(Bash, &mut command, &bin_name, &mut stdout),
        CompletionShell::Zsh => generate(Zsh, &mut command, &bin_name, &mut stdout),
        CompletionShell::Fish => generate(Fish, &mut command, &bin_name, &mut stdout),
        CompletionShell::PowerShell => {
            generate(PowerShell, &mut command, &bin_name, &mut stdout)
        }
    }

    Ok(())
}
