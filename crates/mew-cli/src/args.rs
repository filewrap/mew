use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "mew")]
#[command(version)]
#[command(about = "mew — a tiny coding cat with sharp claws")]
#[command(
    after_help = "Quick start:\n  mew setup       Connect a provider and choose a model\n  mew             Open Mew chat in the current project\n  mew doctor      Check terminal, configuration, and tools\n  mew --inline    Use scrollback-style chat output\n\nStyle:\n  mew style set <theme>           Choose a built-in theme\n  mew style motion reduced|full   Adjust mascot and UI motion\n\nInside chat:\n  /               Search commands and project actions\n  /model          Search and switch configured models\n  /sessions       Search and resume saved sessions\n  /skills         Search skills and read their instructions\n  /mcp            Inspect configured MCP servers\n  /exit           Save the session and leave\n\nAsk mode is read-only. Edit mode asks before applying each patch."
)]
pub struct Cli {
    /// Use the scrollback-friendly chat presentation instead of the full-screen shell.
    #[arg(long, global = true)]
    pub inline: bool,
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Diagnose terminal, config, and local tool availability.
    Doctor,
    /// Initialize and index the current workspace.
    Init {
        #[arg(long)]
        dry_run: bool,
    },
    Name(NameCommand),
    Style(StyleCommand),
    Config(ConfigCommand),
    /// List, validate, or add model providers.
    Provider(ProviderCommand),
    /// List available models or change the active model.
    Model(ModelCommand),
    /// List or resume saved chat sessions.
    Session(SessionCommand),
    /// Run first-time setup or change the configured provider/model.
    Setup,
    Ask {
        prompt: String,
        #[arg(short, long)]
        model: Option<String>,
    },
    /// Start the interactive, native-tool workspace chat.
    Chat {
        #[arg(short, long)]
        model: Option<String>,
    },
    /// Run the bounded native-tool agent loop.
    Agent(AgentCommand),
    /// Repair an issue using exact-match, diff-previewed edits.
    Fix {
        prompt: String,
        #[arg(short, long)]
        model: Option<String>,
        #[arg(long)]
        allow_checks: bool,
        #[arg(long)]
        allow_network: bool,
        #[arg(
            long,
            help = "Approve each proposed edit/check/network request without an interactive prompt"
        )]
        yes: bool,
        #[arg(long = "skill", action = clap::ArgAction::Append)]
        skills: Vec<String>,
    },
    /// Review the working tree with read-only tools.
    Review {
        #[arg(
            default_value = "Review the working tree for correctness, regressions, security issues, and missing tests."
        )]
        prompt: String,
        #[arg(short, long)]
        model: Option<String>,
    },
    /// Run planner, coder, and reviewer model passes and export a transcript.
    Council {
        prompt: String,
        #[arg(short, long)]
        model: Option<String>,
    },
    Skills(SkillsCommand),
    Mcp(McpCommand),
    /// Create, inspect, validate, and execute TOML-defined dependency workflows.
    Workflow(WorkflowCommand),
    /// Create, inspect, validate, and execute sequential agent pipelines.
    Pipeline(WorkflowCommand),
    /// Manage workspace-defined reusable subagents.
    Subagent(SubagentCommand),
    /// Create, inspect, or remove Git worktrees for isolated workspace work.
    Worktree(WorktreeCommand),
    /// Initialize, inspect, or validate project lifecycle hooks.
    Hooks(HooksCommand),
}

#[derive(Debug, Args)]
pub struct AgentCommand {
    pub prompt: String,
    #[arg(short, long)]
    pub model: Option<String>,
    #[arg(long, value_enum, default_value_t = AgentModeArg::Ask)]
    pub mode: AgentModeArg,
    #[arg(long)]
    pub allow_checks: bool,
    #[arg(long)]
    pub allow_network: bool,
    #[arg(
        long,
        help = "Approve each proposed edit/check/network request without an interactive prompt"
    )]
    pub yes: bool,
    #[arg(long = "skill", action = clap::ArgAction::Append)]
    pub skills: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AgentModeArg {
    Ask,
    Edit,
    Auto,
}

#[derive(Debug, Args)]
pub struct NameCommand {
    #[command(subcommand)]
    pub command: NameSubcommand,
}
#[derive(Debug, Subcommand)]
pub enum NameSubcommand {
    Show,
    Set { name: String },
    Random,
    Reset,
}

#[derive(Debug, Args)]
pub struct StyleCommand {
    #[command(subcommand)]
    pub command: StyleSubcommand,
}
#[derive(Debug, Subcommand)]
pub enum StyleSubcommand {
    List,
    Set {
        theme: String,
    },
    Preview,
    Motion {
        #[arg(value_enum)]
        mode: MotionMode,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum MotionMode {
    Full,
    Reduced,
}

#[derive(Debug, Args)]
pub struct ConfigCommand {
    #[command(subcommand)]
    pub command: ConfigSubcommand,
}
#[derive(Debug, Subcommand)]
pub enum ConfigSubcommand {
    Path,
    Show,
}

#[derive(Debug, Args)]
pub struct ProviderCommand {
    #[command(subcommand)]
    pub command: ProviderSubcommand,
}
#[derive(Debug, Subcommand)]
pub enum ProviderSubcommand {
    List,
    Test {
        provider: String,
    },
    /// Open Google sign-in in the system browser for the built-in Gemini provider.
    Login {
        provider: String,
    },
    /// Remove Mew’s locally stored Google Gemini OAuth tokens.
    Logout {
        provider: String,
    },
    #[command(name = "add", alias = "add-openai")]
    AddOpenai {
        /// Short unique provider id (for example: local-llm)
        id: String,
        /// OpenAI-compatible HTTPS endpoint (HTTP is allowed only for loopback)
        base_url: String,
        /// Environment-variable name holding the API key; the key itself is not stored
        api_key_env: String,
        /// Model id accepted by this endpoint
        model: String,
    },
}

#[derive(Debug, Args)]
pub struct ModelCommand {
    #[command(subcommand)]
    pub command: ModelSubcommand,
}
#[derive(Debug, Subcommand)]
pub enum ModelSubcommand {
    List {
        provider: Option<String>,
        #[arg(long)]
        remote: bool,
        #[arg(long)]
        all: bool,
    },
    Use {
        model: String,
    },
    Show,
}

#[derive(Debug, Args)]
pub struct SessionCommand {
    #[command(subcommand)]
    pub command: SessionSubcommand,
}
#[derive(Debug, Subcommand)]
pub enum SessionSubcommand {
    List,
    Show { id: String },
    Resume { id: String },
}

#[derive(Debug, Args)]
pub struct SkillsCommand {
    #[command(subcommand)]
    pub command: SkillsSubcommand,
}
#[derive(Debug, Subcommand)]
pub enum SkillsSubcommand {
    List,
    Show { name: String },
}

#[derive(Debug, Args)]
pub struct McpCommand {
    #[command(subcommand)]
    pub command: McpSubcommand,
}
#[derive(Debug, Subcommand)]
pub enum McpSubcommand {
    /// Start mew's read-only native-tools MCP server on stdio.
    Serve,
    /// Start an MCP server process and list its advertised tools.
    Tools {
        #[arg(required = true, num_args = 1..)]
        server: Vec<String>,
    },
    /// Start an MCP server process and invoke one advertised tool.
    Call {
        tool: String,
        arguments_json: String,
        #[arg(last = true, required = true, num_args = 1..)]
        server: Vec<String>,
    },
}

#[derive(Debug, Args)]
pub struct WorkflowCommand {
    #[command(subcommand)]
    pub command: WorkflowSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum WorkflowSubcommand {
    List,
    Init {
        id: String,
        #[arg(long, value_enum)]
        kind: Option<WorkflowKindArg>,
    },
    Show {
        id: String,
    },
    Validate {
        id: String,
    },
    Run {
        id: String,
        #[arg(long, conflicts_with = "input_file")]
        input: Option<String>,
        #[arg(long)]
        input_file: Option<std::path::PathBuf>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum WorkflowKindArg {
    Workflow,
    Pipeline,
}

#[derive(Debug, Args)]
pub struct SubagentCommand {
    #[command(subcommand)]
    pub command: SubagentSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum SubagentSubcommand {
    List,
    Init {
        id: String,
    },
    Show {
        id: String,
    },
    Run {
        id: String,
        prompt: String,
        #[arg(short, long)]
        model: Option<String>,
    },
}

#[derive(Debug, Args)]
pub struct WorktreeCommand {
    #[command(subcommand)]
    pub command: WorktreeSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum WorktreeSubcommand {
    List,
    Create {
        branch: String,
        /// New worktree location, relative to the current project unless absolute.
        path: Option<std::path::PathBuf>,
    },
    Remove {
        /// Worktree directory to remove; only clean worktrees are eligible.
        path: std::path::PathBuf,
    },
}

#[derive(Debug, Args)]
pub struct HooksCommand {
    #[command(subcommand)]
    pub command: HooksSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum HooksSubcommand {
    Init,
    Status,
    Validate,
}
