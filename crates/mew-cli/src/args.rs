use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "mew")]
#[command(version)]
#[command(about = "mew — a tiny coding cat with sharp claws")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Doctor,
    Init {
        #[arg(long)]
        dry_run: bool,
    },
    Name(NameCommand),
    Style(StyleCommand),
    Config(ConfigCommand),
    Provider(ProviderCommand),
    Model(ModelCommand),
    Session(SessionCommand),
    Ask {
        prompt: String,
        #[arg(short, long)]
        model: Option<String>,
    },
    Chat {
        #[arg(short, long)]
        model: Option<String>,
    },
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
    Set { theme: String },
    Preview,
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
    AddOpenai {
        id: String,
        base_url: String,
        api_key_env: String,
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
