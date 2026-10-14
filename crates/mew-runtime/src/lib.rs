//! Reusable orchestration backend for mew.
//!
//! The CLI is only one possible frontend. This crate owns agent runs, typed
//! results, tool routing, subagent delegation, workflow DAGs, pipeline helpers,
//! token budgeting, lifecycle events, and MCP tool adapters.

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use futures_util::future::join_all;
use mew_markdown::{parse_response, ResponseDocument, MODEL_INSTRUCTIONS};
use mew_mcp::McpClient;
use mew_provider::{
    assistant_message, system_message, user_message, ChatMessage, ChatRequest, Provider, ToolCall,
    ToolSpec,
};
use mew_skills::Skill;
use mew_tokens::{compact_messages, Budget};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tokio::time::timeout;
use uuid::Uuid;

const DELEGATE_TOOL: &str = "delegate_to_agent";

/// A model-facing agent identity. Different agents may use different providers,
/// models, system instructions, skills, and tool registries.
#[derive(Clone)]
pub struct AgentProfile {
    pub id: String,
    pub provider: Arc<dyn Provider>,
    pub model: String,
    pub system_prompt: String,
    pub skills: Vec<Skill>,
    pub tools: ToolRegistry,
    pub limits: AgentLimits,
    pub delegated_permissions: RuntimePermissions,
}

impl AgentProfile {
    pub fn new(
        id: impl Into<String>,
        provider: Arc<dyn Provider>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            provider,
            model: model.into(),
            system_prompt: "You are an agent running inside mew. Be accurate, respect tool boundaries, and do not claim actions that did not succeed.".into(),
            skills: Vec::new(),
            tools: ToolRegistry::default(),
            limits: AgentLimits::default(),
            delegated_permissions: RuntimePermissions::read_only(),
        }
    }

    pub fn with_delegated_permissions(mut self, permissions: RuntimePermissions) -> Self {
        self.delegated_permissions = permissions;
        self
    }

    fn complete_system_prompt(&self) -> String {
        let mut prompt = self.system_prompt.clone();
        for skill in &self.skills {
            prompt.push_str("\n\n## Skill: ");
            prompt.push_str(&skill.name);
            prompt.push('\n');
            prompt.push_str(&skill.body);
        }
        prompt.push_str("\n\n");
        prompt.push_str(MODEL_INSTRUCTIONS);
        prompt
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AgentLimits {
    pub max_steps: usize,
    pub context_tokens: usize,
    pub response_reserve: usize,
}

impl Default for AgentLimits {
    fn default() -> Self {
        Self {
            max_steps: 16,
            context_tokens: 32_000,
            response_reserve: 2_000,
        }
    }
}

/// Permissions are capability grants, not automatic confirmation. Any
/// non-read-only tool still requires an approver unless `auto_approve` is set.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct RuntimePermissions {
    pub allow_write: bool,
    pub allow_execute: bool,
    pub allow_network: bool,
    pub allow_external: bool,
    pub auto_approve: bool,
}

impl RuntimePermissions {
    pub fn read_only() -> Self {
        Self::default()
    }

    fn permits(self, risk: ToolRisk) -> bool {
        match risk {
            ToolRisk::ReadOnly => true,
            ToolRisk::Write => self.allow_write,
            ToolRisk::Execute => self.allow_execute,
            ToolRisk::Network => self.allow_network,
            ToolRisk::External => self.allow_external,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolRisk {
    ReadOnly,
    Write,
    Execute,
    Network,
    External,
}

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub run_id: String,
    pub parent_run_id: Option<String>,
    pub agent_id: String,
    pub depth: usize,
}

#[derive(Debug, Clone)]
pub struct ToolOutput {
    pub content: Vec<ContentPart>,
    pub metadata: BTreeMap<String, Value>,
}

impl ToolOutput {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ContentPart::Text { text: text.into() }],
            metadata: BTreeMap::new(),
        }
    }

    pub fn json(value: Value) -> Self {
        Self {
            content: vec![ContentPart::Json { value }],
            metadata: BTreeMap::new(),
        }
    }

    fn observation(&self) -> String {
        self.content
            .iter()
            .map(ContentPart::as_text)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Extensible tool contract. Tool implementations own their domain-specific
/// validation and side-effect controls; the runtime adds capability gating,
/// optional approval, output caps, tracing, and provider schema publication.
#[async_trait]
pub trait RuntimeTool: Send + Sync {
    fn definition(&self) -> ToolSpec;
    fn risk(&self) -> ToolRisk {
        ToolRisk::ReadOnly
    }
    fn approval_summary(&self, arguments: &Value) -> String {
        format!("{}({})", self.definition().name, arguments)
    }
    async fn execute(&self, context: ToolContext, arguments: Value) -> Result<ToolOutput>;
}

#[derive(Clone, Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, Arc<dyn RuntimeTool>>,
}

impl ToolRegistry {
    pub fn register<T: RuntimeTool + 'static>(&mut self, tool: T) -> Result<()> {
        self.register_shared(Arc::new(tool))
    }

    pub fn register_shared(&mut self, tool: Arc<dyn RuntimeTool>) -> Result<()> {
        let definition = tool.definition();
        validate_identifier(&definition.name, "tool")?;
        if definition.name == DELEGATE_TOOL {
            bail!("{DELEGATE_TOOL} is reserved by the runtime");
        }
        if !definition.parameters.is_object() {
            bail!(
                "tool {} must provide an object JSON schema",
                definition.name
            );
        }
        if self.tools.contains_key(&definition.name) {
            bail!("duplicate tool name: {}", definition.name);
        }
        self.tools.insert(definition.name, tool);
        Ok(())
    }

    pub fn definitions(&self) -> Vec<ToolSpec> {
        self.tools.values().map(|tool| tool.definition()).collect()
    }

    fn get(&self, name: &str) -> Option<Arc<dyn RuntimeTool>> {
        self.tools.get(name).cloned()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

#[async_trait]
pub trait ToolApprover: Send + Sync {
    async fn approve(&self, request: ToolApprovalRequest) -> Result<bool>;
}

#[derive(Debug, Clone)]
pub struct ToolApprovalRequest {
    pub run_id: String,
    pub agent_id: String,
    pub tool_name: String,
    pub summary: String,
    pub arguments: Value,
    pub risk: ToolRisk,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeLimits {
    pub max_depth: usize,
    pub max_total_runs: usize,
    pub max_total_tool_calls: usize,
    pub max_workflow_steps: usize,
    pub max_parallel_workflow_steps: usize,
    pub max_tool_output_bytes: usize,
    pub model_timeout: Duration,
    pub tool_timeout: Duration,
}

impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            max_depth: 4,
            max_total_runs: 32,
            max_total_tool_calls: 256,
            max_workflow_steps: 128,
            max_parallel_workflow_steps: 8,
            max_tool_output_bytes: 48_000,
            model_timeout: Duration::from_secs(180),
            tool_timeout: Duration::from_secs(60),
        }
    }
}

#[derive(Default)]
pub struct RuntimeBuilder {
    agents: HashMap<String, AgentProfile>,
    limits: RuntimeLimits,
    approver: Option<Arc<dyn ToolApprover>>,
    events: Option<mpsc::UnboundedSender<RuntimeEvent>>,
}

impl RuntimeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn limits(mut self, limits: RuntimeLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn approver(mut self, approver: Arc<dyn ToolApprover>) -> Self {
        self.approver = Some(approver);
        self
    }

    pub fn events(mut self, sender: mpsc::UnboundedSender<RuntimeEvent>) -> Self {
        self.events = Some(sender);
        self
    }

    pub fn register_agent(&mut self, profile: AgentProfile) -> Result<()> {
        validate_identifier(&profile.id, "agent")?;
        if profile.model.trim().is_empty() {
            bail!("agent {} has an empty model id", profile.id);
        }
        if self.agents.contains_key(&profile.id) {
            bail!("duplicate agent id: {}", profile.id);
        }
        self.agents.insert(profile.id.clone(), profile);
        Ok(())
    }

    pub fn build(self) -> Result<AgentRuntime> {
        if self.agents.is_empty() {
            bail!("runtime requires at least one registered agent");
        }
        if self.limits.max_total_runs == 0 || self.limits.max_total_tool_calls == 0 {
            bail!("runtime execution limits must be non-zero");
        }
        Ok(AgentRuntime {
            agents: Arc::new(self.agents),
            limits: self.limits,
            approver: self.approver,
            events: self.events,
        })
    }
}

#[derive(Clone)]
pub struct AgentRuntime {
    agents: Arc<HashMap<String, AgentProfile>>,
    limits: RuntimeLimits,
    approver: Option<Arc<dyn ToolApprover>>,
    events: Option<mpsc::UnboundedSender<RuntimeEvent>>,
}

impl AgentRuntime {
    pub fn agent_ids(&self) -> Vec<String> {
        let mut ids = self.agents.keys().cloned().collect::<Vec<_>>();
        ids.sort();
        ids
    }

    pub async fn run(&self, request: AgentRunRequest) -> Result<AgentRunResult> {
        let budget = Arc::new(ExecutionBudget::default());
        self.run_internal(request, None, 0, budget).await
    }

    async fn run_internal(
        &self,
        request: AgentRunRequest,
        parent_run_id: Option<String>,
        depth: usize,
        budget: Arc<ExecutionBudget>,
    ) -> Result<AgentRunResult> {
        if depth > self.limits.max_depth {
            bail!("subagent depth limit ({}) exceeded", self.limits.max_depth);
        }
        if budget.runs.fetch_add(1, Ordering::Relaxed) >= self.limits.max_total_runs {
            bail!(
                "runtime total-run limit ({}) exceeded",
                self.limits.max_total_runs
            );
        }
        let profile = self
            .agents
            .get(&request.agent_id)
            .cloned()
            .ok_or_else(|| anyhow!("unknown agent: {}", request.agent_id))?;
        if request.prompt.trim().is_empty() && request.history.is_empty() {
            bail!("agent prompt and history are both empty");
        }
        let run_id = Uuid::new_v4().to_string();
        self.emit(RuntimeEvent::RunStarted {
            run_id: run_id.clone(),
            parent_run_id: parent_run_id.clone(),
            agent_id: profile.id.clone(),
            depth,
        });
        let result = self
            .run_agent_body(
                &profile,
                request,
                run_id.clone(),
                parent_run_id.clone(),
                depth,
                budget,
            )
            .await;
        match result {
            Ok(result) => {
                self.emit(RuntimeEvent::RunFinished {
                    run_id,
                    agent_id: profile.id,
                    status: result.status,
                    iterations: result.iterations,
                });
                Ok(result)
            }
            Err(error) => {
                self.emit(RuntimeEvent::RunFailed {
                    run_id,
                    agent_id: profile.id,
                    error: format!("{error:#}"),
                });
                Err(error)
            }
        }
    }

    async fn run_agent_body(
        &self,
        profile: &AgentProfile,
        request: AgentRunRequest,
        run_id: String,
        parent_run_id: Option<String>,
        depth: usize,
        budget: Arc<ExecutionBudget>,
    ) -> Result<AgentRunResult> {
        let mut messages = vec![system_message(profile.complete_system_prompt())];
        messages.extend(
            request
                .history
                .into_iter()
                .filter(|message| message.role != "system"),
        );
        if !request.prompt.trim().is_empty() {
            messages.push(user_message(request.prompt));
        }

        let mut delegate_targets = self
            .agents
            .keys()
            .filter(|id| *id != &profile.id)
            .cloned()
            .collect::<Vec<_>>();
        delegate_targets.sort();
        let can_delegate = depth < self.limits.max_depth
            && !delegate_targets.is_empty()
            && profile.provider.supports_tool_calls(&profile.model);
        let mut tool_specs = profile.tools.definitions();
        if can_delegate {
            tool_specs.push(delegate_tool_spec(&delegate_targets));
        }
        if !tool_specs.is_empty() && !profile.provider.supports_tool_calls(&profile.model) {
            bail!(
                "agent {}/{} has tools but its provider/model is not marked as tool-capable",
                profile.id,
                profile.model
            );
        }

        let token_budget = Budget {
            context_tokens: profile.limits.context_tokens.max(2_048),
            response_reserve: profile.limits.response_reserve.max(256),
        };
        let mut input_tokens = 0u64;
        let mut output_tokens = 0u64;
        let mut has_input_usage = false;
        let mut has_output_usage = false;
        let mut estimated_input_tokens = 0usize;
        let mut transcript = messages.clone();
        let mut tool_calls = Vec::new();
        let mut children = Vec::new();
        let mut final_response = AgentResponse::default();
        let mut iterations = 0usize;

        for iteration in 1..=profile.limits.max_steps.max(1) {
            iterations = iteration;
            let (compacted, estimate) = compact_messages(&messages, token_budget);
            estimated_input_tokens = estimated_input_tokens.saturating_add(estimate);
            messages = compacted;
            self.emit(RuntimeEvent::ModelRequestStarted {
                run_id: run_id.clone(),
                agent_id: profile.id.clone(),
                iteration,
                estimated_input_tokens: estimate,
            });
            let response = timeout(
                self.limits.model_timeout,
                profile.provider.chat(ChatRequest {
                    model: profile.model.clone(),
                    messages: messages.clone(),
                    temperature: Some(0.2),
                    max_tokens: Some(token_budget.response_reserve.min(u32::MAX as usize) as u32),
                    tools: tool_specs.clone(),
                }),
            )
            .await
            .context("model request timed out")??;
            if let Some(value) = response.input_tokens {
                input_tokens = input_tokens.saturating_add(value);
                has_input_usage = true;
            }
            if let Some(value) = response.output_tokens {
                output_tokens = output_tokens.saturating_add(value);
                has_output_usage = true;
            }
            self.emit(RuntimeEvent::ModelResponseReceived {
                run_id: run_id.clone(),
                agent_id: profile.id.clone(),
                iteration,
                tool_calls: response.tool_calls.len(),
                input_tokens: response.input_tokens,
                output_tokens: response.output_tokens,
            });
            if response.tool_calls.is_empty() {
                let presentation = parse_response(&response.text);
                let clean_text = presentation.markdown.clone();
                final_response = AgentResponse {
                    content: if clean_text.is_empty() {
                        Vec::new()
                    } else {
                        vec![ContentPart::Text {
                            text: clean_text.clone(),
                        }]
                    },
                    presentation,
                    finish_reason: FinishReason::Stop,
                    metadata: BTreeMap::from([
                        ("provider".into(), json!(response.provider)),
                        ("model".into(), json!(response.model)),
                    ]),
                };
                messages.push(assistant_message(clean_text));
                transcript = messages.clone();
                break;
            }

            messages.push(ChatMessage {
                role: "assistant".into(),
                content: response.text,
                tool_calls: response.tool_calls.clone(),
                tool_call_id: None,
                name: None,
            });
            for call in response.tool_calls {
                self.emit(RuntimeEvent::ToolStarted {
                    run_id: run_id.clone(),
                    agent_id: profile.id.clone(),
                    tool_call_id: call.id.clone(),
                    tool_name: call.name.clone(),
                });
                if !budget.take_tool_call(self.limits.max_total_tool_calls) {
                    let output = "Denied: runtime total tool-call limit reached.".to_string();
                    self.record_tool_result(&run_id, &profile.id, &call, false, output.clone());
                    messages.push(tool_message(&call, output));
                    tool_calls.push(ToolCallRecord::failure(
                        call,
                        "total tool-call limit reached",
                    ));
                    continue;
                }
                let parsed = serde_json::from_str::<Value>(&call.arguments);
                let (observation, succeeded, record, child) = match parsed {
                    Err(error) => {
                        let output = format!("Invalid JSON arguments: {error}");
                        (
                            output.clone(),
                            false,
                            ToolCallRecord::failure(call.clone(), output),
                            None,
                        )
                    }
                    Ok(arguments) if !arguments.is_object() => {
                        let output = "Invalid arguments: expected a JSON object.".to_string();
                        (
                            output.clone(),
                            false,
                            ToolCallRecord::failure(call.clone(), output),
                            None,
                        )
                    }
                    Ok(arguments) if call.name == DELEGATE_TOOL => {
                        match self
                            .delegate(profile, &run_id, depth, arguments, budget.clone())
                            .await
                        {
                            Ok((child_result, observation)) => (
                                observation,
                                true,
                                ToolCallRecord::success(call.clone(), Vec::new()),
                                Some(child_result),
                            ),
                            Err(error) => {
                                let output = format!("Subagent failed: {error:#}");
                                (
                                    output.clone(),
                                    false,
                                    ToolCallRecord::failure(call.clone(), output),
                                    None,
                                )
                            }
                        }
                    }
                    Ok(arguments) => match profile.tools.get(&call.name) {
                        None => {
                            let output = format!("Unknown tool: {}", call.name);
                            (
                                output.clone(),
                                false,
                                ToolCallRecord::failure(call.clone(), output),
                                None,
                            )
                        }
                        Some(tool) => {
                            let risk = tool.risk();
                            if !request.permissions.permits(risk) {
                                let output = format!(
                                    "Denied: the run has not granted the {risk:?} capability."
                                );
                                (
                                    output.clone(),
                                    false,
                                    ToolCallRecord::failure(call.clone(), output),
                                    None,
                                )
                            } else if risk != ToolRisk::ReadOnly
                                && !request.permissions.auto_approve
                                && !self
                                    .approve_tool(
                                        &tool,
                                        &run_id,
                                        &profile.id,
                                        risk,
                                        arguments.clone(),
                                    )
                                    .await?
                            {
                                let output =
                                    "Denied by approver; no operation was performed.".to_string();
                                (
                                    output.clone(),
                                    false,
                                    ToolCallRecord::failure(call.clone(), output),
                                    None,
                                )
                            } else {
                                let context = ToolContext {
                                    run_id: run_id.clone(),
                                    parent_run_id: parent_run_id.clone(),
                                    agent_id: profile.id.clone(),
                                    depth,
                                };
                                match timeout(
                                    self.limits.tool_timeout,
                                    tool.execute(context, arguments.clone()),
                                )
                                .await
                                {
                                    Ok(Ok(mut output)) => {
                                        for part in &mut output.content {
                                            part.cap_text(self.limits.max_tool_output_bytes);
                                        }
                                        let observation = cap_text(
                                            &output.observation(),
                                            self.limits.max_tool_output_bytes,
                                        );
                                        let record = ToolCallRecord {
                                            id: call.id.clone(),
                                            name: call.name.clone(),
                                            arguments,
                                            succeeded: true,
                                            output: output.content,
                                            metadata: output.metadata,
                                            error: None,
                                        };
                                        (observation, true, record, None)
                                    }
                                    Ok(Err(error)) => {
                                        let output = format!("Tool error: {error:#}");
                                        (
                                            output.clone(),
                                            false,
                                            ToolCallRecord::failure(call.clone(), output),
                                            None,
                                        )
                                    }
                                    Err(_) => {
                                        let output = format!(
                                            "Tool timed out after {} seconds.",
                                            self.limits.tool_timeout.as_secs()
                                        );
                                        (
                                            output.clone(),
                                            false,
                                            ToolCallRecord::failure(call.clone(), output),
                                            None,
                                        )
                                    }
                                }
                            }
                        }
                    },
                };
                self.record_tool_result(
                    &run_id,
                    &profile.id,
                    &call,
                    succeeded,
                    observation.clone(),
                );
                if let Some(child) = child {
                    children.push(child);
                }
                tool_calls.push(record);
                messages.push(tool_message(&call, observation));
            }
            transcript = messages.clone();
            final_response = AgentResponse {
                content: Vec::new(),
                presentation: ResponseDocument::default(),
                finish_reason: FinishReason::ToolCalls,
                metadata: BTreeMap::new(),
            };
        }

        let status = if final_response.finish_reason == FinishReason::Stop {
            RunStatus::Completed
        } else {
            final_response.content = vec![ContentPart::Text {
                text:
                    "The agent reached its configured step limit before producing a final response."
                        .into(),
            }];
            final_response.finish_reason = FinishReason::MaxSteps;
            RunStatus::MaxSteps
        };
        Ok(AgentRunResult {
            run_id,
            parent_run_id,
            agent_id: profile.id.clone(),
            status,
            response: final_response,
            usage: TokenUsage {
                input_tokens: has_input_usage.then_some(input_tokens),
                output_tokens: has_output_usage.then_some(output_tokens),
                estimated_input_tokens,
            },
            iterations,
            transcript,
            tool_calls,
            children,
            metadata: request.metadata,
        })
    }

    async fn approve_tool(
        &self,
        tool: &Arc<dyn RuntimeTool>,
        run_id: &str,
        agent_id: &str,
        risk: ToolRisk,
        arguments: Value,
    ) -> Result<bool> {
        let Some(approver) = &self.approver else {
            return Ok(false);
        };
        approver
            .approve(ToolApprovalRequest {
                run_id: run_id.into(),
                agent_id: agent_id.into(),
                tool_name: tool.definition().name,
                summary: tool.approval_summary(&arguments),
                arguments,
                risk,
            })
            .await
    }

    async fn delegate(
        &self,
        parent: &AgentProfile,
        parent_run_id: &str,
        depth: usize,
        arguments: Value,
        budget: Arc<ExecutionBudget>,
    ) -> Result<(AgentRunResult, String)> {
        let agent_id = arguments
            .get("agent_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("delegate_to_agent requires agent_id"))?;
        let task = arguments
            .get("task")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow!("delegate_to_agent requires a non-empty task"))?;
        if agent_id == parent.id {
            bail!("an agent cannot delegate to itself");
        }
        if !self.agents.contains_key(agent_id) {
            bail!("unknown subagent: {agent_id}");
        }
        let child = Box::pin(self.run_internal(
            AgentRunRequest::new(agent_id, task).with_permissions(parent.delegated_permissions),
            Some(parent_run_id.into()),
            depth + 1,
            budget,
        ))
        .await?;
        let observation = format!(
            "Subagent {} ({}) returned:\n{}",
            child.agent_id,
            child.status.as_str(),
            child.response.as_text()
        );
        Ok((child, observation))
    }

    fn record_tool_result(
        &self,
        run_id: &str,
        agent_id: &str,
        call: &ToolCall,
        success: bool,
        output: String,
    ) {
        self.emit(RuntimeEvent::ToolFinished {
            run_id: run_id.into(),
            agent_id: agent_id.into(),
            tool_call_id: call.id.clone(),
            tool_name: call.name.clone(),
            succeeded: success,
            output_preview: cap_text(&output, 512),
        });
    }

    fn emit(&self, event: RuntimeEvent) {
        if let Some(sender) = &self.events {
            let _ = sender.send(event);
        }
    }

    /// Run a validated dependency graph. Ready independent steps execute in
    /// bounded parallel batches; dependent steps receive completed outputs.
    pub fn validate_workflow_spec(&self, spec: &WorkflowSpec) -> Result<()> {
        validate_workflow(spec, &self.agents, self.limits.max_workflow_steps)
    }

    /// Run a sequence of stages, automatically passing each stage's result to the next.
    pub async fn run_pipeline(
        &self,
        id: impl Into<String>,
        steps: Vec<WorkflowStep>,
        input: Value,
    ) -> Result<WorkflowResult> {
        self.run_workflow(WorkflowSpec::pipeline(id, steps), input)
            .await
    }

    pub async fn run_workflow(&self, spec: WorkflowSpec, input: Value) -> Result<WorkflowResult> {
        validate_workflow(&spec, &self.agents, self.limits.max_workflow_steps)?;
        let workflow_id = Uuid::new_v4().to_string();
        self.emit(RuntimeEvent::WorkflowStarted {
            workflow_id: workflow_id.clone(),
            name: spec.id.clone(),
            steps: spec.steps.len(),
        });
        let budget = Arc::new(ExecutionBudget::default());
        let by_id = spec
            .steps
            .iter()
            .map(|step| (step.id.clone(), step))
            .collect::<HashMap<_, _>>();
        let mut pending = spec
            .steps
            .iter()
            .map(|step| step.id.clone())
            .collect::<BTreeSet<_>>();
        let mut completed: HashMap<String, WorkflowStepResult> = HashMap::new();
        let parallelism = spec
            .max_parallel_steps
            .unwrap_or(self.limits.max_parallel_workflow_steps)
            .clamp(1, self.limits.max_parallel_workflow_steps.max(1));

        while !pending.is_empty() {
            let ready = pending
                .iter()
                .filter_map(|id| by_id.get(id).copied())
                .filter(|step| {
                    step.depends_on
                        .iter()
                        .all(|dependency| completed.contains_key(dependency))
                })
                .cloned()
                .collect::<Vec<_>>();
            if ready.is_empty() {
                bail!("workflow has unresolved dependencies or a cycle");
            }

            let failed_dependencies = ready
                .iter()
                .filter(|step| {
                    step.depends_on.iter().any(|dependency| {
                        completed
                            .get(dependency)
                            .is_some_and(|result| result.status != StepStatus::Succeeded)
                    })
                })
                .map(|step| step.id.clone())
                .collect::<Vec<_>>();
            for id in failed_dependencies {
                pending.remove(&id);
                let result =
                    WorkflowStepResult::skipped(id.clone(), "a dependency did not succeed");
                completed.insert(id.clone(), result.clone());
                self.emit(RuntimeEvent::WorkflowStepFinished {
                    workflow_id: workflow_id.clone(),
                    step_id: id,
                    status: result.status,
                });
            }

            let mut runnable = ready
                .into_iter()
                .filter(|step| pending.contains(&step.id))
                .collect::<Vec<_>>();
            if spec.fail_fast
                && completed
                    .values()
                    .any(|step| step.status == StepStatus::Failed)
            {
                runnable.clear();
            }
            if runnable.is_empty() {
                if spec.fail_fast
                    && completed
                        .values()
                        .any(|step| step.status == StepStatus::Failed)
                {
                    for id in std::mem::take(&mut pending) {
                        let result = WorkflowStepResult::skipped(
                            id.clone(),
                            "workflow stopped after a failed step",
                        );
                        completed.insert(id.clone(), result.clone());
                        self.emit(RuntimeEvent::WorkflowStepFinished {
                            workflow_id: workflow_id.clone(),
                            step_id: id,
                            status: result.status,
                        });
                    }
                    break;
                }
                continue;
            }

            for batch in runnable.chunks(parallelism) {
                let mut prepared = Vec::with_capacity(batch.len());
                for step in batch {
                    match render_template(&step.prompt, &input, &completed, &step.depends_on) {
                        Ok(prompt) => prepared.push((step.clone(), prompt)),
                        Err(error) => {
                            pending.remove(&step.id);
                            let step_result = WorkflowStepResult {
                                step_id: step.id.clone(),
                                status: StepStatus::Failed,
                                result: None,
                                error: Some(format!("render workflow step {}: {error:#}", step.id)),
                            };
                            self.emit(RuntimeEvent::WorkflowStepFinished {
                                workflow_id: workflow_id.clone(),
                                step_id: step.id.clone(),
                                status: step_result.status,
                            });
                            completed.insert(step.id.clone(), step_result);
                        }
                    }
                }
                if prepared.is_empty() {
                    if spec.fail_fast
                        && completed
                            .values()
                            .any(|step| step.status == StepStatus::Failed)
                    {
                        break;
                    }
                    continue;
                }
                let results = join_all(prepared.into_iter().map(|(step, prompt)| async {
                    let run_request = AgentRunRequest::new(step.agent_id.clone(), prompt)
                        .with_permissions(step.permissions)
                        .with_metadata(
                            json!({"workflow_id": workflow_id, "workflow_step_id": step.id}),
                        );
                    let result = self
                        .run_internal(run_request, None, 0, budget.clone())
                        .await;
                    (step, result)
                }))
                .await;
                for (step, run_result) in results {
                    pending.remove(&step.id);
                    let step_result = match run_result {
                        Ok(result) => WorkflowStepResult {
                            step_id: step.id.clone(),
                            status: StepStatus::Succeeded,
                            result: Some(result),
                            error: None,
                        },
                        Err(error) => WorkflowStepResult {
                            step_id: step.id.clone(),
                            status: StepStatus::Failed,
                            result: None,
                            error: Some(format!("{error:#}")),
                        },
                    };
                    self.emit(RuntimeEvent::WorkflowStepFinished {
                        workflow_id: workflow_id.clone(),
                        step_id: step.id.clone(),
                        status: step_result.status,
                    });
                    completed.insert(step.id, step_result);
                }
                if spec.fail_fast
                    && completed
                        .values()
                        .any(|step| step.status == StepStatus::Failed)
                {
                    break;
                }
            }
        }

        let steps = spec
            .steps
            .iter()
            .filter_map(|step| completed.remove(&step.id))
            .collect::<Vec<_>>();
        let status = if steps
            .iter()
            .all(|step| step.status == StepStatus::Succeeded)
        {
            WorkflowStatus::Succeeded
        } else if steps
            .iter()
            .any(|step| step.status == StepStatus::Succeeded)
        {
            WorkflowStatus::Partial
        } else {
            WorkflowStatus::Failed
        };
        self.emit(RuntimeEvent::WorkflowFinished {
            workflow_id: workflow_id.clone(),
            name: spec.id.clone(),
            status,
        });
        Ok(WorkflowResult {
            workflow_id,
            name: spec.id,
            status,
            steps,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AgentRunRequest {
    pub agent_id: String,
    pub prompt: String,
    pub history: Vec<ChatMessage>,
    pub permissions: RuntimePermissions,
    pub metadata: Value,
}

impl AgentRunRequest {
    pub fn new(agent_id: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            prompt: prompt.into(),
            history: Vec::new(),
            permissions: RuntimePermissions::read_only(),
            metadata: Value::Null,
        }
    }

    pub fn with_history(mut self, history: Vec<ChatMessage>) -> Self {
        self.history = history;
        self
    }

    pub fn with_permissions(mut self, permissions: RuntimePermissions) -> Self {
        self.permissions = permissions;
        self
    }

    pub fn with_metadata(mut self, metadata: Value) -> Self {
        self.metadata = metadata;
        self
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Completed,
    MaxSteps,
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::MaxSteps => "max_steps",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    #[default]
    Stop,
    ToolCalls,
    MaxSteps,
}

/// Extensible multimodal response envelope. Current built-in model adapters
/// populate text; tools and future provider adapters can return structured JSON,
/// image/audio/video references, artifacts, or forward-compatible opaque parts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text {
        text: String,
    },
    Json {
        value: Value,
    },
    Image {
        uri: String,
        mime_type: Option<String>,
        alt: Option<String>,
    },
    Audio {
        uri: String,
        mime_type: Option<String>,
        transcript: Option<String>,
    },
    Video {
        uri: String,
        mime_type: Option<String>,
        description: Option<String>,
    },
    Artifact {
        name: String,
        uri: String,
        mime_type: Option<String>,
    },
    Opaque {
        media_type: String,
        value: Value,
    },
}

impl ContentPart {
    fn as_text(&self) -> String {
        match self {
            Self::Text { text } => text.clone(),
            Self::Json { value } => {
                serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
            }
            Self::Image { uri, alt, .. } => format!(
                "[image: {}{}]",
                alt.as_deref().unwrap_or("image"),
                uri_suffix(uri)
            ),
            Self::Audio {
                uri, transcript, ..
            } => transcript
                .clone()
                .unwrap_or_else(|| format!("[audio{}]", uri_suffix(uri))),
            Self::Video {
                uri, description, ..
            } => format!(
                "[video: {}{}]",
                description.as_deref().unwrap_or("video"),
                uri_suffix(uri)
            ),
            Self::Artifact { name, uri, .. } => format!("[artifact: {name}{}]", uri_suffix(uri)),
            Self::Opaque { media_type, value } => format!("[{media_type}: {value}]"),
        }
    }

    fn cap_text(&mut self, max_bytes: usize) {
        if let Self::Text { text } = self {
            *text = cap_text(text, max_bytes);
        }
    }
}

fn uri_suffix(uri: &str) -> String {
    if uri.is_empty() {
        String::new()
    } else {
        format!("; {uri}")
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AgentResponse {
    pub content: Vec<ContentPart>,
    #[serde(default)]
    pub presentation: ResponseDocument,
    #[serde(default)]
    pub finish_reason: FinishReason,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

impl AgentResponse {
    pub fn as_text(&self) -> String {
        self.content
            .iter()
            .map(ContentPart::as_text)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub estimated_input_tokens: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCallRecord {
    pub id: String,
    pub name: String,
    pub arguments: Value,
    pub succeeded: bool,
    pub output: Vec<ContentPart>,
    pub metadata: BTreeMap<String, Value>,
    pub error: Option<String>,
}

impl ToolCallRecord {
    fn success(call: ToolCall, output: Vec<ContentPart>) -> Self {
        let arguments = serde_json::from_str(&call.arguments).unwrap_or(Value::Null);
        Self {
            id: call.id,
            name: call.name,
            arguments,
            succeeded: true,
            output,
            metadata: BTreeMap::new(),
            error: None,
        }
    }

    fn failure(call: ToolCall, error: impl Into<String>) -> Self {
        let arguments = serde_json::from_str(&call.arguments).unwrap_or(Value::Null);
        Self {
            id: call.id,
            name: call.name,
            arguments,
            succeeded: false,
            output: Vec::new(),
            metadata: BTreeMap::new(),
            error: Some(error.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRunResult {
    pub run_id: String,
    pub parent_run_id: Option<String>,
    pub agent_id: String,
    pub status: RunStatus,
    pub response: AgentResponse,
    pub usage: TokenUsage,
    pub iterations: usize,
    pub transcript: Vec<ChatMessage>,
    pub tool_calls: Vec<ToolCallRecord>,
    pub children: Vec<AgentRunResult>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum RuntimeEvent {
    RunStarted {
        run_id: String,
        parent_run_id: Option<String>,
        agent_id: String,
        depth: usize,
    },
    ModelRequestStarted {
        run_id: String,
        agent_id: String,
        iteration: usize,
        estimated_input_tokens: usize,
    },
    ModelResponseReceived {
        run_id: String,
        agent_id: String,
        iteration: usize,
        tool_calls: usize,
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
    },
    ToolStarted {
        run_id: String,
        agent_id: String,
        tool_call_id: String,
        tool_name: String,
    },
    ToolFinished {
        run_id: String,
        agent_id: String,
        tool_call_id: String,
        tool_name: String,
        succeeded: bool,
        output_preview: String,
    },
    RunFinished {
        run_id: String,
        agent_id: String,
        status: RunStatus,
        iterations: usize,
    },
    RunFailed {
        run_id: String,
        agent_id: String,
        error: String,
    },
    WorkflowStarted {
        workflow_id: String,
        name: String,
        steps: usize,
    },
    WorkflowStepFinished {
        workflow_id: String,
        step_id: String,
        status: StepStatus,
    },
    WorkflowFinished {
        workflow_id: String,
        name: String,
        status: WorkflowStatus,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    pub agent_id: String,
    pub prompt: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub permissions: RuntimePermissions,
}

impl WorkflowStep {
    pub fn new(
        id: impl Into<String>,
        agent_id: impl Into<String>,
        prompt: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            agent_id: agent_id.into(),
            prompt: prompt.into(),
            depends_on: Vec::new(),
            permissions: RuntimePermissions::read_only(),
        }
    }

    pub fn depends_on(mut self, dependencies: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.depends_on = dependencies.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_permissions(mut self, permissions: RuntimePermissions) -> Self {
        self.permissions = permissions;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSpec {
    pub id: String,
    pub steps: Vec<WorkflowStep>,
    #[serde(default)]
    pub max_parallel_steps: Option<usize>,
    #[serde(default)]
    pub fail_fast: bool,
}

impl WorkflowSpec {
    pub fn new(id: impl Into<String>, steps: Vec<WorkflowStep>) -> Self {
        Self {
            id: id.into(),
            steps,
            max_parallel_steps: None,
            fail_fast: false,
        }
    }

    /// Convert a list of steps into a sequential pipeline by chaining each step
    /// to its immediate predecessor. Each step prompt may reference prior output.
    pub fn pipeline(id: impl Into<String>, mut steps: Vec<WorkflowStep>) -> Self {
        for index in 1..steps.len() {
            steps[index].depends_on = vec![steps[index - 1].id.clone()];
        }
        Self::new(id, steps)
    }

    pub fn with_parallelism(mut self, max_parallel_steps: usize) -> Self {
        self.max_parallel_steps = Some(max_parallel_steps);
        self
    }

    pub fn fail_fast(mut self, enabled: bool) -> Self {
        self.fail_fast = enabled;
        self
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Succeeded,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStatus {
    Succeeded,
    Partial,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStepResult {
    pub step_id: String,
    pub status: StepStatus,
    pub result: Option<AgentRunResult>,
    pub error: Option<String>,
}

impl WorkflowStepResult {
    fn skipped(step_id: String, reason: &str) -> Self {
        Self {
            step_id,
            status: StepStatus::Skipped,
            result: None,
            error: Some(reason.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResult {
    pub workflow_id: String,
    pub name: String,
    pub status: WorkflowStatus,
    pub steps: Vec<WorkflowStepResult>,
}

/// Adapter for one tool advertised by an already-started MCP stdio server.
/// Marked External so runs require `allow_external` plus an approver (unless
/// deliberately auto-approved by the embedding application).
pub struct McpRuntimeTool {
    client: Arc<Mutex<McpClient>>,
    server: String,
    name: String,
    remote_name: String,
    description: String,
    input_schema: Value,
}

impl McpRuntimeTool {
    pub fn from_advertisement(
        client: Arc<Mutex<McpClient>>,
        server: impl Into<String>,
        advertisement: &Value,
    ) -> Result<Self> {
        let remote_name = advertisement
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("MCP tool advertisement is missing its name"))?
            .to_string();
        let server = server.into();
        let name = format!("{server}__{remote_name}");
        Self::from_advertisement_as(client, server, advertisement, name)
    }

    pub fn from_advertisement_as(
        client: Arc<Mutex<McpClient>>,
        server: impl Into<String>,
        advertisement: &Value,
        exposed_name: impl Into<String>,
    ) -> Result<Self> {
        let remote_name = advertisement
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("MCP tool advertisement is missing its name"))?
            .to_string();
        validate_identifier(&remote_name, "MCP tool")?;
        let name = exposed_name.into();
        validate_identifier(&name, "exposed MCP tool")?;
        let input_schema = advertisement.get("inputSchema").cloned().unwrap_or_else(
            || json!({"type":"object","properties":{},"additionalProperties":true}),
        );
        if !input_schema.is_object() {
            bail!("MCP tool {name} inputSchema must be an object");
        }
        Ok(Self {
            client,
            server: server.into(),
            name,
            remote_name,
            description: advertisement
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("MCP tool")
                .to_string(),
            input_schema,
        })
    }
}

#[async_trait]
impl RuntimeTool for McpRuntimeTool {
    fn definition(&self) -> ToolSpec {
        ToolSpec {
            name: self.name.clone(),
            description: self.description.clone(),
            parameters: self.input_schema.clone(),
        }
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::External
    }

    fn approval_summary(&self, arguments: &Value) -> String {
        format!(
            "Call MCP server {} tool {} with {}",
            self.server, self.name, arguments
        )
    }

    async fn execute(&self, _context: ToolContext, arguments: Value) -> Result<ToolOutput> {
        let response = self
            .client
            .lock()
            .await
            .call_tool(&self.remote_name, arguments)
            .await?;
        if response.get("isError").and_then(Value::as_bool) == Some(true) {
            bail!(
                "MCP server {} reported a tool error: {}",
                self.server,
                response
            );
        }
        let text = response
            .get("content")
            .and_then(Value::as_array)
            .map(|content| {
                content
                    .iter()
                    .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
                    .map(|part| {
                        part.get("text")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string()
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| response.to_string());
        let mut output = ToolOutput::text(text);
        output
            .metadata
            .insert("mcp_server".into(), json!(self.server));
        Ok(output)
    }
}

#[derive(Default)]
struct ExecutionBudget {
    runs: AtomicUsize,
    tool_calls: AtomicUsize,
}

impl ExecutionBudget {
    fn take_tool_call(&self, limit: usize) -> bool {
        self.tool_calls.fetch_add(1, Ordering::Relaxed) < limit
    }
}

fn delegate_tool_spec(targets: &[String]) -> ToolSpec {
    ToolSpec {
        name: DELEGATE_TOOL.into(),
        description: "Delegate a bounded subtask to another registered agent. Subagents run with read-only tool permissions by default.".into(),
        parameters: json!({
            "type":"object",
            "properties":{
                "agent_id":{"type":"string","enum":targets,"description":"Registered agent id; must be different from the current agent."},
                "task":{"type":"string","description":"A focused, self-contained task for the subagent."}
            },
            "required":["agent_id","task"],
            "additionalProperties":false
        }),
    }
}

fn tool_message(call: &ToolCall, content: String) -> ChatMessage {
    ChatMessage {
        role: "tool".into(),
        content,
        tool_calls: Vec::new(),
        tool_call_id: Some(call.id.clone()),
        name: Some(call.name.clone()),
    }
}

fn validate_identifier(value: &str, kind: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    {
        bail!(
            "invalid {kind} identifier {value:?}; use 1–64 ASCII letters, digits, '.', '_' or '-' "
        );
    }
    Ok(())
}

fn validate_workflow(
    spec: &WorkflowSpec,
    agents: &HashMap<String, AgentProfile>,
    max_steps: usize,
) -> Result<()> {
    validate_identifier(&spec.id, "workflow")?;
    if spec.steps.is_empty() {
        bail!("workflow must contain at least one step");
    }
    if spec.steps.len() > max_steps {
        bail!(
            "workflow has {} steps; limit is {max_steps}",
            spec.steps.len()
        );
    }
    let mut step_ids = HashSet::new();
    for step in &spec.steps {
        validate_identifier(&step.id, "workflow step")?;
        if !step_ids.insert(step.id.as_str()) {
            bail!("duplicate workflow step id: {}", step.id);
        }
        if !agents.contains_key(&step.agent_id) {
            bail!(
                "workflow step {} references unknown agent {}",
                step.id,
                step.agent_id
            );
        }
        if step.prompt.trim().is_empty() {
            bail!("workflow step {} has an empty prompt", step.id);
        }
    }
    let ids = spec
        .steps
        .iter()
        .map(|step| step.id.as_str())
        .collect::<HashSet<_>>();
    for step in &spec.steps {
        let mut unique = HashSet::new();
        for dependency in &step.depends_on {
            if !ids.contains(dependency.as_str()) {
                bail!(
                    "workflow step {} depends on unknown step {}",
                    step.id,
                    dependency
                );
            }
            if dependency == &step.id {
                bail!("workflow step {} cannot depend on itself", step.id);
            }
            if !unique.insert(dependency) {
                bail!(
                    "workflow step {} lists dependency {} more than once",
                    step.id,
                    dependency
                );
            }
        }
    }
    let mut visited = HashSet::new();
    let mut pending = spec
        .steps
        .iter()
        .map(|step| step.id.as_str())
        .collect::<HashSet<_>>();
    while !pending.is_empty() {
        let ready = spec
            .steps
            .iter()
            .filter(|step| pending.contains(step.id.as_str()))
            .filter(|step| {
                step.depends_on
                    .iter()
                    .all(|dependency| visited.contains(dependency.as_str()))
            })
            .map(|step| step.id.as_str())
            .collect::<Vec<_>>();
        if ready.is_empty() {
            bail!("workflow contains a dependency cycle");
        }
        for id in ready {
            pending.remove(id);
            visited.insert(id);
        }
    }
    validate_workflow_templates(spec)?;
    Ok(())
}

fn validate_workflow_templates(spec: &WorkflowSpec) -> Result<()> {
    for step in &spec.steps {
        let mut remaining = step.prompt.as_str();
        while let Some(start) = remaining.find("{{") {
            if remaining[..start].contains("}}") {
                bail!(
                    "workflow step {} contains an unmatched template terminator",
                    step.id
                );
            }
            let after_start = &remaining[start + 2..];
            let end = after_start.find("}}").ok_or_else(|| {
                anyhow!(
                    "workflow step {} has an unclosed template expression",
                    step.id
                )
            })?;
            let key = after_start[..end].trim();
            if key.is_empty() || key.contains("{{") {
                bail!(
                    "workflow step {} has an invalid template expression",
                    step.id
                );
            }
            if key == "input" {
                // The complete JSON input is always a valid reference.
            } else if let Some(path) = key.strip_prefix("input.") {
                if path.is_empty() || path.split('.').any(str::is_empty) {
                    bail!(
                        "workflow step {} has an invalid input path {path:?}",
                        step.id
                    );
                }
            } else if let Some(reference) = key.strip_prefix("steps.") {
                let (step_id, field) = reference
                    .split_once('.')
                    .ok_or_else(|| anyhow!("invalid workflow step reference {key:?}"))?;
                if !step
                    .depends_on
                    .iter()
                    .any(|dependency| dependency == step_id)
                {
                    bail!("workflow step reference {step_id:?} in step {} is not declared as a dependency", step.id);
                }
                if !matches!(field, "output" | "response_json") {
                    bail!("unsupported workflow reference field {field:?}");
                }
            } else {
                bail!("unknown workflow template expression {key:?}");
            }
            remaining = &after_start[end + 2..];
        }
        if remaining.contains("}}") {
            bail!(
                "workflow step {} contains an unmatched template terminator",
                step.id
            );
        }
    }
    Ok(())
}

fn render_template(
    template: &str,
    input: &Value,
    completed: &HashMap<String, WorkflowStepResult>,
    declared_dependencies: &[String],
) -> Result<String> {
    let mut rendered = String::with_capacity(template.len());
    let mut remaining = template;
    while let Some(start) = remaining.find("{{") {
        rendered.push_str(&remaining[..start]);
        let after_start = &remaining[start + 2..];
        let end = after_start
            .find("}}")
            .ok_or_else(|| anyhow!("unclosed workflow template expression"))?;
        let key = after_start[..end].trim();
        let replacement = if key == "input" {
            input.to_string()
        } else if let Some(path) = key.strip_prefix("input.") {
            let mut value = input;
            for component in path.split('.') {
                value = value
                    .get(component)
                    .ok_or_else(|| anyhow!("missing workflow input field {path:?}"))?;
            }
            value
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| value.to_string())
        } else if let Some(reference) = key.strip_prefix("steps.") {
            let (step_id, field) = reference
                .split_once('.')
                .ok_or_else(|| anyhow!("invalid workflow step reference {key:?}"))?;
            if !declared_dependencies
                .iter()
                .any(|dependency| dependency == step_id)
            {
                bail!("workflow step reference {step_id:?} is not declared as a dependency");
            }
            let step = completed
                .get(step_id)
                .ok_or_else(|| anyhow!("workflow dependency {step_id:?} is not complete"))?;
            if step.status != StepStatus::Succeeded {
                bail!("workflow dependency {step_id:?} did not succeed");
            }
            let result = step
                .result
                .as_ref()
                .ok_or_else(|| anyhow!("workflow dependency {step_id:?} has no result"))?;
            match field {
                "output" => result.response.as_text(),
                "response_json" => serde_json::to_string(&result.response)?,
                _ => bail!("unsupported workflow reference field {field:?}"),
            }
        } else {
            bail!("unknown workflow template variable {key:?}");
        };
        rendered.push_str(&replacement);
        remaining = &after_start[end + 2..];
    }
    rendered.push_str(remaining);
    Ok(rendered)
}

fn cap_text(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes.min(value.len());
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[output truncated]", &value[..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use async_trait::async_trait;
    use mew_markdown::ResponseBlock;
    use mew_provider::{ChatResponse, ProviderModel};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MockProvider {
        id: String,
        calls: AtomicUsize,
        behavior: MockBehavior,
    }

    #[derive(Clone, Copy)]
    enum MockBehavior {
        Echo,
        RichResponse,
        CallEchoTool,
        CallWriteTool,
        Delegate,
        Fail,
    }

    impl MockProvider {
        fn new(id: &str, behavior: MockBehavior) -> Self {
            Self {
                id: id.into(),
                calls: AtomicUsize::new(0),
                behavior,
            }
        }
    }

    #[async_trait]
    impl Provider for MockProvider {
        fn id(&self) -> &str {
            &self.id
        }
        fn api_key_env(&self) -> &str {
            "MOCK_KEY"
        }
        fn default_model(&self) -> &str {
            "mock-model"
        }
        fn models(&self) -> Vec<ProviderModel> {
            vec![ProviderModel {
                provider: self.id.clone(),
                id: "mock-model".into(),
                context: 8192,
                supports_tools: true,
                supports_vision: false,
                notes: String::new(),
            }]
        }
        fn is_authorized(&self) -> bool {
            true
        }
        async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            if matches!(self.behavior, MockBehavior::Fail) {
                bail!("fixture provider failure");
            }
            let (text, tool_calls) = match self.behavior {
                MockBehavior::Echo => {
                    let text = request
                        .messages
                        .iter()
                        .rev()
                        .find(|message| message.role == "user")
                        .map(|message| message.content.clone())
                        .unwrap_or_default();
                    (text, Vec::new())
                }
                MockBehavior::RichResponse => (
                    concat!(
                        "Answer.\n\n",
                        "> A quoted passage.\n\n",
                        "<!-- mew:v4:button {\"label\":\"Show example\",\"prompt\":\"Give a concrete example.\"} -->"
                    )
                    .into(),
                    Vec::new(),
                ),
                MockBehavior::CallEchoTool if call == 0 => (
                    String::new(),
                    vec![ToolCall {
                        id: "tc-1".into(),
                        name: "echo".into(),
                        arguments: r#"{"text":"tool-result"}"#.into(),
                    }],
                ),
                MockBehavior::CallWriteTool if call == 0 => (
                    String::new(),
                    vec![ToolCall {
                        id: "tc-write".into(),
                        name: "danger".into(),
                        arguments: "{}".into(),
                    }],
                ),
                MockBehavior::Delegate if call == 0 => (
                    String::new(),
                    vec![ToolCall {
                        id: "tc-child".into(),
                        name: DELEGATE_TOOL.into(),
                        arguments: r#"{"agent_id":"child","task":"child-task"}"#.into(),
                    }],
                ),
                MockBehavior::CallEchoTool
                | MockBehavior::CallWriteTool
                | MockBehavior::Delegate => {
                    let observation = request
                        .messages
                        .iter()
                        .rev()
                        .find(|message| message.role == "tool")
                        .map(|message| message.content.clone())
                        .unwrap_or_else(|| "done".into());
                    (observation, Vec::new())
                }
                MockBehavior::Fail => unreachable!("failure returned before response construction"),
            };
            Ok(ChatResponse {
                text,
                provider: self.id.clone(),
                model: request.model,
                input_tokens: Some(7),
                output_tokens: Some(3),
                tool_calls,
            })
        }
        async fn test(&self) -> Result<()> {
            Ok(())
        }
    }

    struct EchoTool;
    #[async_trait]
    impl RuntimeTool for EchoTool {
        fn definition(&self) -> ToolSpec {
            ToolSpec {
                name: "echo".into(),
                description: "Return supplied text".into(),
                parameters: json!({"type":"object","properties":{"text":{"type":"string"}},"required":["text"],"additionalProperties":false}),
            }
        }
        async fn execute(&self, _context: ToolContext, arguments: Value) -> Result<ToolOutput> {
            Ok(ToolOutput::text(
                arguments
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            ))
        }
    }

    struct SideEffectTool(Arc<AtomicUsize>);
    #[async_trait]
    impl RuntimeTool for SideEffectTool {
        fn definition(&self) -> ToolSpec {
            ToolSpec {
                name: "danger".into(),
                description: "Test-only side effect".into(),
                parameters: json!({"type":"object","properties":{},"additionalProperties":false}),
            }
        }
        fn risk(&self) -> ToolRisk {
            ToolRisk::Write
        }
        async fn execute(&self, _context: ToolContext, _arguments: Value) -> Result<ToolOutput> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(ToolOutput::text("side effect executed"))
        }
    }

    struct AllowApprover;
    #[async_trait]
    impl ToolApprover for AllowApprover {
        async fn approve(&self, _request: ToolApprovalRequest) -> Result<bool> {
            Ok(true)
        }
    }

    fn profile(id: &str, provider: Arc<dyn Provider>) -> AgentProfile {
        AgentProfile::new(id, provider, "mock-model")
    }

    #[tokio::test]
    async fn runtime_injects_v4_guidance_and_returns_parsed_presentation() {
        let agent = profile(
            "main",
            Arc::new(MockProvider::new("mock", MockBehavior::RichResponse)),
        );
        assert!(agent
            .complete_system_prompt()
            .contains("Mew response format (Markdown v4 layer)"));

        let mut builder = RuntimeBuilder::new();
        builder.register_agent(agent).unwrap();
        let runtime = builder.build().unwrap();
        let result = runtime
            .run(AgentRunRequest::new("main", "answer"))
            .await
            .unwrap();

        let text = result.response.as_text();
        assert!(text.contains("Answer."));
        assert!(text.contains("> A quoted passage."));
        assert!(!text.contains("mew:v4:button"));
        assert_eq!(
            result.response.presentation.buttons().next().unwrap().label,
            "Show example"
        );
        assert!(result
            .response
            .presentation
            .blocks
            .iter()
            .any(|block| matches!(
                block,
                ResponseBlock::Quote { text } if text == "A quoted passage."
            )));
    }

    #[tokio::test]
    async fn dispatches_tools_and_returns_typed_trace_and_usage() {
        let provider = Arc::new(MockProvider::new("mock", MockBehavior::CallEchoTool));
        let mut agent = profile("main", provider);
        agent.tools.register(EchoTool).unwrap();
        let (events_tx, mut events_rx) = mpsc::unbounded_channel();
        let mut builder = RuntimeBuilder::new().events(events_tx);
        builder.register_agent(agent).unwrap();
        let runtime = builder.build().unwrap();
        let result = runtime
            .run(AgentRunRequest::new("main", "use echo"))
            .await
            .unwrap();
        assert_eq!(result.status, RunStatus::Completed);
        assert_eq!(result.response.as_text(), "tool-result");
        assert_eq!(result.tool_calls.len(), 1);
        assert!(result.tool_calls[0].succeeded);
        assert_eq!(result.usage.input_tokens, Some(14));
        assert!(result
            .transcript
            .iter()
            .any(|message| message.role == "tool"));
        let events = std::iter::from_fn(|| events_rx.try_recv().ok()).collect::<Vec<_>>();
        assert!(events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::ToolStarted { .. })));
        assert!(events.iter().any(|event| matches!(
            event,
            RuntimeEvent::ToolFinished {
                succeeded: true,
                ..
            }
        )));
    }

    #[tokio::test]
    async fn write_tool_fails_closed_without_an_approver() {
        let executed = Arc::new(AtomicUsize::new(0));
        let mut agent = profile(
            "main",
            Arc::new(MockProvider::new("mock", MockBehavior::CallWriteTool)),
        );
        agent
            .tools
            .register(SideEffectTool(executed.clone()))
            .unwrap();
        let mut builder = RuntimeBuilder::new();
        builder.register_agent(agent).unwrap();
        let runtime = builder.build().unwrap();
        let permissions = RuntimePermissions {
            allow_write: true,
            ..RuntimePermissions::default()
        };
        let result = runtime
            .run(AgentRunRequest::new("main", "try it").with_permissions(permissions))
            .await
            .unwrap();
        assert_eq!(executed.load(Ordering::SeqCst), 0);
        assert!(!result.tool_calls[0].succeeded);
        assert!(result.tool_calls[0]
            .error
            .as_deref()
            .unwrap()
            .contains("approver"));
    }

    #[tokio::test]
    async fn write_tool_runs_only_after_capability_and_approval() {
        let executed = Arc::new(AtomicUsize::new(0));
        let mut agent = profile(
            "main",
            Arc::new(MockProvider::new("mock", MockBehavior::CallWriteTool)),
        );
        agent
            .tools
            .register(SideEffectTool(executed.clone()))
            .unwrap();
        let mut builder = RuntimeBuilder::new().approver(Arc::new(AllowApprover));
        builder.register_agent(agent).unwrap();
        let runtime = builder.build().unwrap();
        let permissions = RuntimePermissions {
            allow_write: true,
            ..RuntimePermissions::default()
        };
        let result = runtime
            .run(AgentRunRequest::new("main", "try it").with_permissions(permissions))
            .await
            .unwrap();
        assert_eq!(executed.load(Ordering::SeqCst), 1);
        assert!(result.tool_calls[0].succeeded);
    }

    #[tokio::test]
    async fn delegates_to_a_read_only_subagent() {
        let parent_provider = Arc::new(MockProvider::new("parent-model", MockBehavior::Delegate));
        let child_provider = Arc::new(MockProvider::new("child-model", MockBehavior::Echo));
        let mut builder = RuntimeBuilder::new();
        builder
            .register_agent(profile("parent", parent_provider))
            .unwrap();
        builder
            .register_agent(profile("child", child_provider))
            .unwrap();
        let runtime = builder.build().unwrap();
        let result = runtime
            .run(AgentRunRequest::new("parent", "delegate this"))
            .await
            .unwrap();
        assert_eq!(result.children.len(), 1);
        assert_eq!(result.children[0].agent_id, "child");
        assert_eq!(result.children[0].response.as_text(), "child-task");
        assert!(result.response.as_text().contains("child-task"));
    }

    #[tokio::test]
    async fn pipeline_passes_outputs_between_steps() {
        let provider = Arc::new(MockProvider::new("echo-model", MockBehavior::Echo));
        let mut builder = RuntimeBuilder::new();
        builder.register_agent(profile("worker", provider)).unwrap();
        let runtime = builder.build().unwrap();
        let result = runtime
            .run_pipeline(
                "simple-pipeline",
                vec![
                    WorkflowStep::new("first", "worker", "first={{input.name}}"),
                    WorkflowStep::new("second", "worker", "second={{steps.first.output}}"),
                ],
                json!({"name":"mew"}),
            )
            .await
            .unwrap();
        assert_eq!(result.status, WorkflowStatus::Succeeded);
        assert_eq!(result.steps.len(), 2);
        assert_eq!(
            result.steps[1].result.as_ref().unwrap().response.as_text(),
            "second=first=mew"
        );
    }

    #[tokio::test]
    async fn workflow_cycles_are_rejected_before_execution() {
        let provider = Arc::new(MockProvider::new("echo-model", MockBehavior::Echo));
        let mut builder = RuntimeBuilder::new();
        builder
            .register_agent(profile("worker", provider.clone()))
            .unwrap();
        let runtime = builder.build().unwrap();
        let spec = WorkflowSpec::new(
            "cycle",
            vec![
                WorkflowStep::new("a", "worker", "a").depends_on(["b"]),
                WorkflowStep::new("b", "worker", "b").depends_on(["a"]),
            ],
        );
        assert!(runtime.run_workflow(spec, Value::Null).await.is_err());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn workflow_templates_are_validated_before_provider_calls() {
        let provider = Arc::new(MockProvider::new("echo-model", MockBehavior::Echo));
        let mut builder = RuntimeBuilder::new();
        builder
            .register_agent(profile("worker", provider.clone()))
            .unwrap();
        let runtime = builder.build().unwrap();
        let invalid = WorkflowSpec::new(
            "invalid-template",
            vec![
                WorkflowStep::new("first", "worker", "hello"),
                WorkflowStep::new("second", "worker", "{{steps.first.output}}"),
            ],
        );
        assert!(runtime.validate_workflow_spec(&invalid).is_err());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn failed_dependencies_skip_children_but_allow_independent_branches() {
        let mut builder = RuntimeBuilder::new();
        builder
            .register_agent(profile(
                "bad",
                Arc::new(MockProvider::new("bad", MockBehavior::Fail)),
            ))
            .unwrap();
        builder
            .register_agent(profile(
                "good",
                Arc::new(MockProvider::new("good", MockBehavior::Echo)),
            ))
            .unwrap();
        let runtime = builder.build().unwrap();
        let spec = WorkflowSpec::new(
            "partial",
            vec![
                WorkflowStep::new("broken", "bad", "fail"),
                WorkflowStep::new("blocked", "good", "never run").depends_on(["broken"]),
                WorkflowStep::new("independent", "good", "still run"),
            ],
        );
        let result = runtime.run_workflow(spec, Value::Null).await.unwrap();
        assert_eq!(result.status, WorkflowStatus::Partial);
        assert_eq!(result.steps[0].status, StepStatus::Failed);
        assert_eq!(result.steps[1].status, StepStatus::Skipped);
        assert_eq!(result.steps[2].status, StepStatus::Succeeded);
    }

    #[tokio::test]
    async fn bad_template_fails_its_step_without_aborting_independent_steps() {
        let provider = Arc::new(MockProvider::new("echo", MockBehavior::Echo));
        let mut builder = RuntimeBuilder::new();
        builder.register_agent(profile("worker", provider)).unwrap();
        let runtime = builder.build().unwrap();
        let spec = WorkflowSpec::new(
            "template-error",
            vec![
                WorkflowStep::new("bad", "worker", "{{input.missing}}"),
                WorkflowStep::new("good", "worker", "still runs"),
            ],
        );
        let result = runtime.run_workflow(spec, json!({})).await.unwrap();
        assert_eq!(result.status, WorkflowStatus::Partial);
        assert_eq!(result.steps[0].status, StepStatus::Failed);
        assert_eq!(result.steps[1].status, StepStatus::Succeeded);
    }

    #[test]
    fn output_cap_preserves_utf8_boundaries() {
        let result = cap_text(&"λ".repeat(20), 7);
        assert!(result.is_char_boundary(result.find('\n').unwrap_or(result.len())));
    }
}
