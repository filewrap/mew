use crate::args::{WorkflowCommand, WorkflowKindArg, WorkflowSubcommand};
use crate::commands::definitions::{
    build_agent_profile, create_definition_file, load_agent_definitions, read_toml, validate_id,
    workspace_definition_dir, AgentDefinition,
};
use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use mew_common::MewConfig;
use mew_mcp::McpClient;
use mew_provider::{ProviderRegistry, ToolSpec};
use mew_runtime::{
    AgentRuntime, McpRuntimeTool, RuntimeBuilder, RuntimeLimits, RuntimeTool, ToolContext,
    ToolOutput, ToolRisk, WorkflowResult, WorkflowSpec, WorkflowStatus, WorkflowStep,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum DocumentKind {
    #[default]
    Workflow,
    Pipeline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkflowDocument {
    #[serde(default = "format_version")]
    version: u32,
    id: String,
    #[serde(default)]
    kind: DocumentKind,
    #[serde(default)]
    agents: Vec<AgentDefinition>,
    steps: Vec<WorkflowStep>,
    #[serde(default)]
    max_parallel_steps: Option<usize>,
    #[serde(default)]
    fail_fast: bool,
}

fn format_version() -> u32 {
    1
}

impl WorkflowDocument {
    fn scaffold(id: &str, kind: DocumentKind) -> Self {
        Self {
            version: 1,
            id: id.to_string(),
            kind,
            agents: vec![AgentDefinition {
                id: "main".into(),
                model: None,
                system_prompt: "You are a precise workflow agent. Follow the step instructions, use only available tools, and report the result clearly.".into(),
                skills: Vec::new(),
                max_steps: Some(8),
                context_tokens: None,
                response_reserve: None,
            }],
            steps: vec![
                WorkflowStep::new("collect", "main", "Analyze the request: {{input}}"),
                WorkflowStep::new(
                    "synthesize",
                    "main",
                    "Synthesize the previous result:\n{{steps.collect.output}}",
                )
                .depends_on(["collect"]),
            ],
            max_parallel_steps: None,
            fail_fast: true,
        }
    }

    fn validate_shape(&self, forced_pipeline: bool) -> Result<()> {
        if self.version != 1 {
            bail!(
                "unsupported workflow format version {}; expected 1",
                self.version
            );
        }
        validate_id(&self.id, "workflow")?;
        if self.steps.is_empty() {
            bail!("workflow {} must contain at least one step", self.id);
        }
        if self.max_parallel_steps == Some(0) {
            bail!("max_parallel_steps must be at least 1");
        }
        if self.max_parallel_steps.is_some_and(|value| value > 128) {
            bail!("max_parallel_steps may not exceed 128");
        }
        let mut agent_ids = std::collections::HashSet::new();
        for agent in &self.agents {
            agent.validate()?;
            if !agent_ids.insert(agent.id.as_str()) {
                bail!("duplicate workflow agent id: {}", agent.id);
            }
        }
        for step in &self.steps {
            if step.permissions.allow_write
                || step.permissions.allow_execute
                || step.permissions.allow_network
                || step.permissions.allow_external
                || step.permissions.auto_approve
            {
                bail!("workflow step {} must use read-only permissions; side-effect capabilities are not enabled by workflow files", step.id);
            }
        }
        if forced_pipeline && self.kind != DocumentKind::Pipeline {
            bail!("{} is a workflow, not a pipeline; use `mew workflow` or initialize it with --kind pipeline", self.id);
        }
        if self.kind == DocumentKind::Pipeline {
            for (index, step) in self.steps.iter().enumerate() {
                let expected = if index == 0 {
                    None
                } else {
                    Some(self.steps[index - 1].id.as_str())
                };
                if step.depends_on.len() > 1
                    || step.depends_on.first().map(String::as_str) != expected
                {
                    bail!("pipeline step {} must depend only on its immediate predecessor (or no step for the first stage)", step.id);
                }
            }
        }
        Ok(())
    }

    fn to_spec(&self) -> WorkflowSpec {
        let mut spec = match self.kind {
            DocumentKind::Workflow => WorkflowSpec::new(self.id.clone(), self.steps.clone()),
            DocumentKind::Pipeline => WorkflowSpec::pipeline(self.id.clone(), self.steps.clone()),
        };
        spec.max_parallel_steps = self.max_parallel_steps;
        spec.fail_fast = self.fail_fast;
        spec
    }
}

pub async fn run(
    root: &Path,
    cfg: &MewConfig,
    command: WorkflowCommand,
    pipeline_command: bool,
) -> Result<()> {
    match command.command {
        WorkflowSubcommand::List => list(root, pipeline_command),
        WorkflowSubcommand::Init { id, kind } => init(root, &id, kind, pipeline_command),
        WorkflowSubcommand::Show { id } => {
            let (_, document) = load_document(root, &id)?;
            document.validate_shape(pipeline_command)?;
            println!("{}", toml::to_string_pretty(&document)?);
            Ok(())
        }
        WorkflowSubcommand::Validate { id } => {
            let (_, document) = load_document(root, &id)?;
            let spec = validate_document(root, cfg, &document, pipeline_command)?;
            println!(
                "valid {}: {} step(s), {} agent profile(s)",
                kind_name(document.kind),
                spec.steps.len(),
                document.agents.len()
            );
            Ok(())
        }
        WorkflowSubcommand::Run {
            id,
            input,
            input_file,
        } => {
            let (_, document) = load_document(root, &id)?;
            execute(root, cfg, document, input, input_file, pipeline_command).await
        }
    }
}

fn list(root: &Path, pipeline_command: bool) -> Result<()> {
    let Some(directory) = workspace_definition_dir(root, "workflows", false)? else {
        println!("No workflow definitions found. Create one with `mew workflow init <id>`. ");
        return Ok(());
    };
    let mut documents = Vec::new();
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("toml") {
            continue;
        }
        if fs::symlink_metadata(&path)?.file_type().is_symlink() || !path.is_file() {
            bail!(
                "refusing to list a non-regular workflow definition {}",
                path.display()
            );
        }
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        validate_id(stem, "workflow filename")?;
        let document: WorkflowDocument = read_toml(&path)?;
        if document.id != stem {
            bail!(
                "workflow id {:?} must match filename {:?}",
                document.id,
                stem
            );
        }
        document
            .validate_shape(false)
            .with_context(|| format!("validate {}", path.display()))?;
        if !pipeline_command || document.kind == DocumentKind::Pipeline {
            documents.push(document);
        }
    }
    documents.sort_by(|left, right| left.id.cmp(&right.id));
    for document in documents {
        println!(
            "{:<24} {:<9} {:>3} step(s) · {} agent profile(s)",
            document.id,
            kind_name(document.kind),
            document.steps.len(),
            document.agents.len(),
        );
    }
    Ok(())
}

pub fn available_definitions(root: &Path, pipeline_command: bool) -> Result<Vec<String>> {
    let Some(directory) = workspace_definition_dir(root, "workflows", false)? else {
        return Ok(Vec::new());
    };
    let mut ids = Vec::new();
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("toml") {
            continue;
        }
        if fs::symlink_metadata(&path)?.file_type().is_symlink() || !path.is_file() {
            bail!(
                "refusing to inspect non-regular workflow definition {}",
                path.display()
            );
        }
        let id = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        validate_id(id, "workflow filename")?;
        let (_, document) = load_document(root, id)?;
        document.validate_shape(false)?;
        if !pipeline_command || document.kind == DocumentKind::Pipeline {
            ids.push(id.to_string());
        }
    }
    ids.sort();
    Ok(ids)
}

fn init(
    root: &Path,
    id: &str,
    requested_kind: Option<WorkflowKindArg>,
    pipeline_command: bool,
) -> Result<()> {
    validate_id(id, "workflow")?;
    let kind = if pipeline_command {
        if matches!(requested_kind, Some(WorkflowKindArg::Workflow)) {
            bail!("`mew pipeline init` cannot create kind=workflow");
        }
        DocumentKind::Pipeline
    } else {
        match requested_kind.unwrap_or(WorkflowKindArg::Workflow) {
            WorkflowKindArg::Workflow => DocumentKind::Workflow,
            WorkflowKindArg::Pipeline => DocumentKind::Pipeline,
        }
    };
    let directory = workspace_definition_dir(root, "workflows", true)?.expect("created directory");
    let path = create_definition_file(&directory, id, &WorkflowDocument::scaffold(id, kind))?;
    println!(
        "created {}",
        path.strip_prefix(root).unwrap_or(&path).display()
    );
    Ok(())
}

fn load_document(root: &Path, id: &str) -> Result<(PathBuf, WorkflowDocument)> {
    validate_id(id, "workflow")?;
    let directory = workspace_definition_dir(root, "workflows", false)?.ok_or_else(|| {
        anyhow!("no workflow definitions found; run `mew workflow init {id}` first")
    })?;
    let path = directory.join(format!("{id}.toml"));
    if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        bail!("refusing to load workflow through a symlink");
    }
    let document: WorkflowDocument = read_toml(&path)?;
    if document.id != id {
        bail!(
            "workflow id {:?} does not match filename {id:?}",
            document.id
        );
    }
    Ok((path, document))
}

fn validate_document(
    root: &Path,
    cfg: &MewConfig,
    document: &WorkflowDocument,
    pipeline_command: bool,
) -> Result<WorkflowSpec> {
    document.validate_shape(pipeline_command)?;
    if pipeline_command && document.kind != DocumentKind::Pipeline {
        bail!("{} is not a sequential pipeline", document.id);
    }
    let spec = document.to_spec();
    let profiles = make_profiles(root, cfg, document)?;
    let mut builder = RuntimeBuilder::new();
    for profile in profiles {
        builder.register_agent(profile)?;
    }
    let runtime = builder.build()?;
    runtime.validate_workflow_spec(&spec)?;
    Ok(spec)
}

fn make_profiles(
    root: &Path,
    cfg: &MewConfig,
    document: &WorkflowDocument,
) -> Result<Vec<mew_runtime::AgentProfile>> {
    let providers = ProviderRegistry::from_config(cfg);
    let mut definitions = load_agent_definitions(root)?;
    for inline in &document.agents {
        if definitions.iter().any(|existing| existing.id == inline.id) {
            bail!("workflow {} redefines workspace agent {:?}; remove the duplicate or use a different id", document.id, inline.id);
        }
        definitions.push(inline.clone());
    }
    let referenced = document
        .steps
        .iter()
        .map(|step| step.agent_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    if referenced.iter().any(|id| *id == "main")
        && !definitions.iter().any(|definition| definition.id == "main")
    {
        definitions.push(AgentDefinition {
            id: "main".into(),
            model: None,
            system_prompt: "You are a precise workflow agent. Follow the workflow step instructions and report evidence and limitations.".into(),
            skills: Vec::new(),
            max_steps: None,
            context_tokens: None,
            response_reserve: None,
        });
    }
    let mut profiles = Vec::new();
    for definition in definitions {
        profiles.push(build_agent_profile(
            &definition,
            cfg,
            &providers,
            root,
            None,
        )?);
    }
    Ok(profiles)
}

async fn execute(
    root: &Path,
    cfg: &MewConfig,
    document: WorkflowDocument,
    input: Option<String>,
    input_file: Option<PathBuf>,
    pipeline_command: bool,
) -> Result<()> {
    let input = read_input(root, input, input_file)?;
    let result = execute_document(root, cfg, document, input, pipeline_command).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    if result.status != WorkflowStatus::Succeeded {
        bail!(
            "{} {} with status {:?}",
            kind_name(if pipeline_command {
                DocumentKind::Pipeline
            } else {
                DocumentKind::Workflow
            }),
            result.name,
            result.status
        );
    }
    Ok(())
}

async fn execute_document(
    root: &Path,
    cfg: &MewConfig,
    document: WorkflowDocument,
    input: Value,
    pipeline_command: bool,
) -> Result<WorkflowResult> {
    let spec = validate_document(root, cfg, &document, pipeline_command)?;
    let profiles = make_profiles(root, cfg, &document)?;
    for profile in &profiles {
        if spec.steps.iter().any(|step| step.agent_id == profile.id)
            && !profile.provider.is_authorized()
        {
            bail!(
                "provider {} for agent {} is not authorized",
                profile.provider.id(),
                profile.id
            );
        }
    }
    let mut builder = RuntimeBuilder::new().limits(RuntimeLimits {
        max_workflow_steps: 128,
        max_total_runs: 256,
        max_total_tool_calls: 512,
        ..RuntimeLimits::default()
    });
    for profile in profiles {
        builder.register_agent(profile)?;
    }
    let runtime: AgentRuntime = builder.build()?;
    runtime.run_workflow(spec, input).await
}

/// Run a saved read-only workflow or pipeline from an interactive agent turn.
/// The returned summary omits provider transcripts while retaining step output,
/// status, and errors for the calling agent.
pub async fn run_from_chat(
    root: &Path,
    cfg: &MewConfig,
    id: &str,
    input: Value,
    pipeline: bool,
) -> Result<Value> {
    validate_id(id, "workflow")?;
    let (_, document) = load_document(root, id)?;
    if pipeline && document.kind != DocumentKind::Pipeline {
        bail!("{id} is not a pipeline; use /workflow {id} or change its kind");
    }
    let kind = document.kind;
    let result = execute_document(root, cfg, document, input, pipeline).await?;
    let steps = result
        .steps
        .iter()
        .map(|step| {
            json!({
                "step_id": step.step_id,
                "status": format!("{:?}", step.status).to_ascii_lowercase(),
                "response": step.result.as_ref().map(|run| run.response.as_text()),
                "error": step.error,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "id": result.name,
        "kind": kind_name(kind),
        "status": format!("{:?}", result.status).to_ascii_lowercase(),
        "steps": steps,
    }))
}

/// Native workflow and configured MCP tools exposed to interactive chat.
/// MCP tools are always registered when configured; each external invocation
/// still passes through the runtime's explicit approval callback.
pub async fn chat_runtime_tools(root: &Path, cfg: &MewConfig) -> Result<Vec<Arc<dyn RuntimeTool>>> {
    let root = root.canonicalize()?;
    let mut tools: Vec<Arc<dyn RuntimeTool>> = vec![
        Arc::new(WorkflowRuntimeTool {
            root: root.clone(),
            config: cfg.clone(),
            pipeline: false,
        }),
        Arc::new(WorkflowRuntimeTool {
            root: root.clone(),
            config: cfg.clone(),
            pipeline: true,
        }),
    ];
    let configured_servers = match configured_mcp_servers(&root) {
        Ok(servers) => servers,
        Err(error) => {
            eprintln!("mew: MCP configuration could not be loaded: {error:#}");
            Vec::new()
        }
    };
    for server in configured_servers {
        if server.kind.as_deref().is_some_and(|kind| kind != "local") {
            eprintln!(
                "mew: MCP server {:?} is configured as remote; this build supports local stdio MCP servers",
                server.name
            );
            continue;
        }
        let Some((program, args)) = server.command.split_first() else {
            eprintln!(
                "mew: MCP server {:?} has an empty command and was skipped",
                server.name
            );
            continue;
        };
        let client = match tokio::time::timeout(
            Duration::from_secs(30),
            McpClient::start_with_env(program, args, Some(&root), &server.environment),
        )
        .await
        {
            Ok(Ok(client)) => client,
            Ok(Err(error)) => {
                eprintln!(
                    "mew: could not start MCP server {:?}: {error:#}",
                    server.name
                );
                continue;
            }
            Err(_) => {
                eprintln!(
                    "mew: MCP server {:?} did not initialize within 30 seconds",
                    server.name
                );
                continue;
            }
        };
        let client = Arc::new(tokio::sync::Mutex::new(client));
        let advertisements = match tokio::time::timeout(Duration::from_secs(30), async {
            client.lock().await.list_tools().await
        })
        .await
        {
            Ok(Ok(advertisements)) => advertisements,
            Ok(Err(error)) => {
                eprintln!(
                    "mew: could not list tools from MCP server {:?}: {error:#}",
                    server.name
                );
                continue;
            }
            Err(_) => {
                eprintln!(
                    "mew: listing tools from MCP server {:?} timed out",
                    server.name
                );
                continue;
            }
        };
        for advertisement in advertisements {
            let remote_name = advertisement
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let exposed_name = qualified_mcp_tool_name(&server.name, remote_name);
            match McpRuntimeTool::from_advertisement_as(
                client.clone(),
                server.name.clone(),
                &advertisement,
                exposed_name,
            ) {
                Ok(tool) => tools.push(Arc::new(tool)),
                Err(error) => eprintln!(
                    "mew: skipping invalid tool from MCP server {:?}: {error:#}",
                    server.name
                ),
            }
        }
    }
    Ok(tools)
}

#[derive(Debug, Clone)]
struct ChatMcpServer {
    name: String,
    kind: Option<String>,
    command: Vec<String>,
    environment: BTreeMap<String, String>,
}

/// Names configured globally or through OpenCode-compatible project config
/// or Mew's `.mew/mcp.json`; a `enabled: false` field is deliberately ignored.
pub fn configured_mcp_names(root: &Path) -> Result<Vec<String>> {
    let mut names = configured_mcp_servers(root)?
        .into_iter()
        .map(|server| server.name)
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    Ok(names)
}

fn configured_mcp_servers(root: &Path) -> Result<Vec<ChatMcpServer>> {
    let root = root.canonicalize()?;
    let mut paths = Vec::new();
    if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
        paths.push((
            PathBuf::from(config_home).join("opencode/opencode.json"),
            false,
        ));
    } else if let Some(home) = std::env::var_os("HOME") {
        paths.push((
            PathBuf::from(home).join(".config/opencode/opencode.json"),
            false,
        ));
    }
    paths.extend([
        (root.join("opencode.json"), true),
        (root.join(".opencode/opencode.json"), true),
        (root.join(".mew/mcp.json"), true),
    ]);
    let mut servers = BTreeMap::new();
    for (path, workspace_scoped) in paths {
        if !path.exists() {
            continue;
        }
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            bail!(
                "refusing to read MCP config through a symlink or non-file: {}",
                path.display()
            );
        }
        let canonical = path.canonicalize()?;
        if (workspace_scoped && !canonical.starts_with(&root)) || metadata.len() > 1024 * 1024 {
            bail!("MCP config must be in its expected scope and no larger than 1 MiB");
        }
        let config: Value = serde_json::from_slice(&fs::read(&canonical)?)
            .with_context(|| format!("parse MCP config {}", canonical.display()))?;
        let Some(entries) = config.get("mcp").and_then(Value::as_object) else {
            continue;
        };
        for (name, entry) in entries {
            if !valid_mcp_name(name) {
                eprintln!("mew: skipping MCP server with invalid name {name:?}");
                continue;
            }
            let Some(object) = entry.as_object() else {
                eprintln!("mew: skipping MCP server {name:?}: configuration must be an object");
                continue;
            };
            // The user's preference is that configured MCP servers stay on;
            // OpenCode's optional enabled=false flag is not used to disable them.
            let kind = object
                .get("type")
                .and_then(Value::as_str)
                .map(str::to_string);
            let command = match object.get("command").and_then(Value::as_array) {
                Some(parts) => parts
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>(),
                None => Vec::new(),
            };
            let mut environment = BTreeMap::new();
            let mut invalid_environment = false;
            if let Some(values) = object.get("environment").and_then(Value::as_object) {
                for (key, value) in values {
                    let Some(value) = value.as_str() else {
                        continue;
                    };
                    match expand_env_reference(value) {
                        Ok(value) => {
                            environment.insert(key.clone(), value);
                        }
                        Err(error) => {
                            eprintln!("mew: skipping MCP server {name:?}: {error:#}");
                            invalid_environment = true;
                            break;
                        }
                    }
                }
            }
            if invalid_environment {
                continue;
            }
            servers.insert(
                name.clone(),
                ChatMcpServer {
                    name: name.clone(),
                    kind,
                    command,
                    environment,
                },
            );
        }
    }
    Ok(servers.into_values().collect())
}

fn valid_mcp_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 48
        && name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
}

fn qualified_mcp_tool_name(server: &str, tool: &str) -> String {
    let qualified = format!("{server}__{tool}");
    if qualified.len() <= 64 {
        return qualified;
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    server.hash(&mut hasher);
    tool.hash(&mut hasher);
    let hash = hasher.finish();
    let server_part = server.chars().take(16).collect::<String>();
    let suffix = format!("_{hash:016x}");
    let room = 64usize
        .saturating_sub(server_part.len() + suffix.len() + 2)
        .max(1);
    let tool_part = tool.chars().take(room).collect::<String>();
    format!("{server_part}__{tool_part}{suffix}")
}

fn expand_env_reference(value: &str) -> Result<String> {
    let mut output = String::new();
    let mut remaining = value;
    while let Some(start) = remaining.find("{env:") {
        output.push_str(&remaining[..start]);
        let after = &remaining[start + 5..];
        let end = after
            .find('}')
            .ok_or_else(|| anyhow!("unterminated {{env:NAME}} MCP environment reference"))?;
        let name = &after[..end];
        let resolved = std::env::var(name)
            .with_context(|| format!("MCP environment variable {name} is not set"))?;
        output.push_str(&resolved);
        remaining = &after[end + 1..];
    }
    output.push_str(remaining);
    Ok(output)
}

struct WorkflowRuntimeTool {
    root: PathBuf,
    config: MewConfig,
    pipeline: bool,
}

#[async_trait]
impl RuntimeTool for WorkflowRuntimeTool {
    fn definition(&self) -> ToolSpec {
        let name = if self.pipeline {
            "run_pipeline"
        } else {
            "run_workflow"
        };
        ToolSpec {
            name: name.into(),
            description: if self.pipeline {
                "Run a named saved sequential pipeline from .mew/workflows. Use only when the user explicitly asks to run that pipeline. Pipelines are validated and read-only."
            } else {
                "Run a named saved dependency workflow from .mew/workflows. Use only when the user explicitly asks to run that workflow. Workflows are validated and read-only."
            }.into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "Saved workflow or pipeline id"},
                    "input": {"type": "object", "description": "JSON input object for the definition", "additionalProperties": true}
                },
                "required": ["id"],
                "additionalProperties": false
            }),
        }
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::ReadOnly
    }

    async fn execute(&self, _context: ToolContext, arguments: Value) -> Result<ToolOutput> {
        let id = arguments
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("workflow tool requires a string id"))?;
        let input = arguments.get("input").cloned().unwrap_or_else(|| json!({}));
        if !input.is_object() {
            bail!("workflow input must be a JSON object");
        }
        let summary = run_from_chat(&self.root, &self.config, id, input, self.pipeline).await?;
        Ok(ToolOutput::text(serde_json::to_string_pretty(&summary)?))
    }
}

fn read_input(root: &Path, inline: Option<String>, input_file: Option<PathBuf>) -> Result<Value> {
    match (inline, input_file) {
        (Some(raw), None) => serde_json::from_str(&raw).context("parse --input as JSON"),
        (None, Some(path)) => {
            let path = if path.is_absolute() {
                path
            } else {
                root.join(path)
            };
            let canonical_root = root.canonicalize()?;
            let canonical_path = path
                .canonicalize()
                .with_context(|| format!("read input file {}", path.display()))?;
            if !canonical_path.starts_with(&canonical_root) {
                bail!("workflow input file must be inside the workspace");
            }
            let metadata = fs::metadata(&canonical_path)?;
            if metadata.len() > 1024 * 1024 {
                bail!("workflow input file exceeds 1 MiB");
            }
            serde_json::from_slice(&fs::read(canonical_path)?)
                .context("parse workflow input file as JSON")
        }
        (None, None) => Ok(json!({})),
        (Some(_), Some(_)) => unreachable!("clap prevents conflicting input options"),
    }
}

fn kind_name(kind: DocumentKind) -> &'static str {
    match kind {
        DocumentKind::Workflow => "workflow",
        DocumentKind::Pipeline => "pipeline",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_definition_chains_stages_in_order() {
        let document = WorkflowDocument::scaffold("example", DocumentKind::Pipeline);
        document.validate_shape(true).unwrap();
        let spec = document.to_spec();
        assert_eq!(spec.steps[0].depends_on, Vec::<String>::new());
        assert_eq!(spec.steps[1].depends_on, vec!["collect"]);
    }

    #[test]
    fn workflow_document_rejects_non_read_only_permissions() {
        let mut document = WorkflowDocument::scaffold("unsafe", DocumentKind::Workflow);
        document.steps[0].permissions.allow_write = true;
        assert!(document.validate_shape(false).is_err());
    }

    #[test]
    fn configured_mcp_is_loaded_even_when_opencode_marks_it_disabled() {
        let root = std::env::temp_dir().join(format!("mew-mcp-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("opencode.json"),
            r#"{"mcp":{"always-on":{"type":"local","enabled":false,"command":["echo","mcp"]}}}"#,
        )
        .unwrap();
        let names = configured_mcp_names(&root).unwrap();
        assert!(names.iter().any(|name| name == "always-on"));
        let _ = fs::remove_dir_all(root);
    }
}
