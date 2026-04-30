use clap::{Args, Subcommand};

mod due_date;
mod read;
mod write;

#[derive(Args)]
pub struct MilestoneArgs {
    #[command(subcommand)]
    pub command: MilestoneCommand,
}

#[derive(Subcommand)]
pub enum MilestoneCommand {
    /// List milestones
    List {
        /// Filter by state (open, closed, all)
        #[arg(
            long,
            short,
            default_value = "open",
            value_parser = ["open", "closed", "all"],
            ignore_case = true
        )]
        state: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// View a milestone
    View {
        /// Milestone number
        number: u64,
    },
    /// Create a milestone
    Create {
        /// Milestone title (prompts when omitted)
        title: Option<String>,
        /// Optional milestone description
        #[arg(long, short)]
        description: Option<String>,
        /// Due date as YYYY-MM-DD or RFC3339
        #[arg(long = "due-on")]
        due_on: Option<String>,
    },
    /// Edit a milestone
    Edit {
        /// Milestone number
        number: u64,
        /// Updated title
        #[arg(long, short)]
        title: Option<String>,
        /// Updated description
        #[arg(long, short)]
        description: Option<String>,
        /// Updated due date as YYYY-MM-DD, RFC3339, or an empty string to clear
        #[arg(long = "due-on")]
        due_on: Option<String>,
        /// Updated state (open or closed)
        #[arg(long, short, value_parser = ["open", "closed"], ignore_case = true)]
        state: Option<String>,
    },
    /// Delete a milestone
    Delete {
        /// Milestone number
        number: u64,
        /// Skip confirmation
        #[arg(long)]
        yes: bool,
    },
}

pub async fn run(
    args: MilestoneArgs,
    cli_hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
) -> crate::error::Result<()> {
    match args.command {
        MilestoneCommand::List { state, json } => {
            read::list(cli_hostname, cli_repo, cli_profile, &state, json).await
        }
        MilestoneCommand::View { number } => {
            read::view(cli_hostname, cli_repo, cli_profile, number).await
        }
        MilestoneCommand::Create {
            title,
            description,
            due_on,
        } => {
            write::create(
                cli_hostname,
                cli_repo,
                cli_profile,
                title,
                description,
                due_on,
            )
            .await
        }
        MilestoneCommand::Edit {
            number,
            title,
            description,
            due_on,
            state,
        } => {
            write::edit(
                cli_hostname,
                cli_repo,
                cli_profile,
                write::EditRequest {
                    number,
                    title,
                    description,
                    due_on,
                    state,
                },
            )
            .await
        }
        MilestoneCommand::Delete { number, yes } => {
            write::delete(cli_hostname, cli_repo, cli_profile, number, yes).await
        }
    }
}
