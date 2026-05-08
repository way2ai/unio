use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use unio_core::UserPaths;
use unio_tools::{ToolCall, ToolDefinition};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRequest {
    pub model: Option<String>,
    pub messages: Vec<ModelMessage>,
    pub tools: Vec<ToolDefinition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub input_tokens: usize,
    pub output_tokens: usize,
}

#[async_trait]
pub trait ModelProvider: Send + Sync {
    async fn complete(&self, request: ModelRequest) -> anyhow::Result<ModelResponse>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSummary {
    pub provider: String,
    pub model: String,
    pub fallback_to_mock: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderKind {
    OpenAi,
    Anthropic,
    Mock,
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: String,
}

impl ProviderConfig {
    pub fn from_env() -> Self {
        let file_config = read_default_config_file().unwrap_or_default();
        Self::from_file_config_and_env(file_config, |key| std::env::var(key))
    }

    #[cfg(test)]
    fn from_config_text_and_env(
        config_text: &str,
        env: impl Fn(&str) -> Option<String>,
    ) -> anyhow::Result<Self> {
        let file_config = toml::from_str(config_text)?;
        Ok(Self::from_file_config_and_env(file_config, |key| {
            env(key).ok_or(std::env::VarError::NotPresent)
        }))
    }

    fn from_file_config_and_env(
        file_config: UnioConfigFile,
        env: impl Fn(&str) -> Result<String, std::env::VarError>,
    ) -> Self {
        let model_config = file_config.model.unwrap_or_default();
        let provider = env("UNIO_MODEL_PROVIDER")
            .ok()
            .or(model_config.provider)
            .unwrap_or_else(|| "mock".into());
        match provider.as_str() {
            "openai" | "openai-compatible" => Self {
                kind: ProviderKind::OpenAi,
                base_url: env("OPENAI_BASE_URL").ok().or(model_config.base_url),
                api_key: env("OPENAI_API_KEY").ok().or(model_config.api_key),
                model: env("OPENAI_MODEL")
                    .ok()
                    .or(model_config.model)
                    .unwrap_or_else(|| "gpt-4o-mini".into()),
            },
            "anthropic" => Self {
                kind: ProviderKind::Anthropic,
                base_url: env("ANTHROPIC_BASE_URL").ok().or(model_config.base_url),
                api_key: env("ANTHROPIC_API_KEY").ok().or(model_config.api_key),
                model: env("ANTHROPIC_MODEL")
                    .ok()
                    .or(model_config.model)
                    .unwrap_or_else(|| "claude-3-5-sonnet-latest".into()),
            },
            _ => Self {
                kind: ProviderKind::Mock,
                base_url: None,
                api_key: None,
                model: "mock".into(),
            },
        }
    }
}

pub enum ResolvedProvider {
    OpenAi(OpenAiCompatibleProvider, ProviderSummary),
    Anthropic(AnthropicProvider, ProviderSummary),
    Mock(MockModelProvider, ProviderSummary),
}

impl ResolvedProvider {
    pub fn from_env() -> Self {
        let config = ProviderConfig::from_env();
        match config.kind {
            ProviderKind::OpenAi if config.api_key.is_some() => {
                let summary = ProviderSummary {
                    provider: "openai-compatible".into(),
                    model: config.model.clone(),
                    fallback_to_mock: false,
                };
                Self::OpenAi(OpenAiCompatibleProvider::new(config), summary)
            }
            ProviderKind::Anthropic if config.api_key.is_some() => {
                let summary = ProviderSummary {
                    provider: "anthropic".into(),
                    model: config.model.clone(),
                    fallback_to_mock: false,
                };
                Self::Anthropic(AnthropicProvider::new(config), summary)
            }
            ProviderKind::OpenAi | ProviderKind::Anthropic | ProviderKind::Mock => {
                let requested = match config.kind {
                    ProviderKind::OpenAi => "openai-compatible",
                    ProviderKind::Anthropic => "anthropic",
                    ProviderKind::Mock => "mock",
                };
                Self::Mock(
                    MockModelProvider,
                    ProviderSummary {
                        provider: requested.into(),
                        model: "mock".into(),
                        fallback_to_mock: requested != "mock",
                    },
                )
            }
        }
    }

    pub fn summary(&self) -> &ProviderSummary {
        match self {
            Self::OpenAi(_, summary) => summary,
            Self::Anthropic(_, summary) => summary,
            Self::Mock(_, summary) => summary,
        }
    }
}

#[async_trait]
impl ModelProvider for ResolvedProvider {
    async fn complete(&self, request: ModelRequest) -> anyhow::Result<ModelResponse> {
        match self {
            Self::OpenAi(provider, _) => provider.complete(request).await,
            Self::Anthropic(provider, _) => provider.complete(request).await,
            Self::Mock(provider, _) => provider.complete(request).await,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MockModelProvider;

#[async_trait]
impl ModelProvider for MockModelProvider {
    async fn complete(&self, request: ModelRequest) -> anyhow::Result<ModelResponse> {
        let latest = request
            .messages
            .last()
            .map(|message| message.content.as_str())
            .unwrap_or("");
        if let Some(query) = latest.trim().strip_prefix("mock-context ") {
            let found = request
                .messages
                .iter()
                .rev()
                .skip(1)
                .any(|message| message.content.contains(query));
            let content = if found {
                format!("Mock context found: {query}")
            } else {
                format!("Mock context missing: {query}")
            };
            return Ok(ModelResponse {
                input_tokens: request
                    .messages
                    .iter()
                    .map(|message| message.content.split_whitespace().count())
                    .sum(),
                output_tokens: content.split_whitespace().count(),
                content,
                tool_calls: Vec::new(),
            });
        }
        if let Some(call) = parse_mock_tool_request(latest) {
            return Ok(ModelResponse {
                input_tokens: latest.split_whitespace().count(),
                output_tokens: 0,
                content: format!("Mock requested tool: {}", call.name),
                tool_calls: vec![call],
            });
        }
        if let Some((input_tokens, output_tokens)) = parse_mock_usage_request(latest)? {
            return Ok(ModelResponse {
                input_tokens,
                output_tokens,
                content: format!("Mock usage: input={input_tokens} output={output_tokens}"),
                tool_calls: Vec::new(),
            });
        }
        let content = format!("Mock root agent received: {latest}");
        Ok(ModelResponse {
            input_tokens: latest.split_whitespace().count(),
            output_tokens: content.split_whitespace().count(),
            content,
            tool_calls: Vec::new(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleProvider {
    client: reqwest::Client,
    config: ProviderConfig,
}

impl OpenAiCompatibleProvider {
    pub fn new(config: ProviderConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            config,
        }
    }
}

#[async_trait]
impl ModelProvider for OpenAiCompatibleProvider {
    async fn complete(&self, request: ModelRequest) -> anyhow::Result<ModelResponse> {
        let base_url = self
            .config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.openai.com/v1".into());
        let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
        let response = self
            .client
            .post(url)
            .bearer_auth(
                self.config
                    .api_key
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("missing OPENAI_API_KEY"))?,
            )
            .json(&json!({
                "model": self.config.model,
                "messages": request.messages,
                "tools": openai_tools(&request.tools),
                "stream": false
            }))
            .send()
            .await?
            .error_for_status()?;
        let payload: OpenAiChatCompletion = response.json().await?;
        let choice = payload
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("openai-compatible response missing choices"))?;
        Ok(ModelResponse {
            content: choice.message.content,
            tool_calls: choice
                .message
                .tool_calls
                .unwrap_or_default()
                .into_iter()
                .map(|call| ToolCall {
                    call_id: call.id,
                    name: call.function.name,
                    arguments: serde_json::from_str(&call.function.arguments)
                        .unwrap_or_else(|_| serde_json::Value::Object(serde_json::Map::new())),
                })
                .collect(),
            input_tokens: payload
                .usage
                .as_ref()
                .map(|usage| usage.prompt_tokens)
                .unwrap_or(0),
            output_tokens: payload
                .usage
                .as_ref()
                .map(|usage| usage.completion_tokens)
                .unwrap_or(0),
        })
    }
}

#[derive(Debug, Clone)]
pub struct AnthropicProvider {
    client: reqwest::Client,
    config: ProviderConfig,
}

impl AnthropicProvider {
    pub fn new(config: ProviderConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            config,
        }
    }
}

#[async_trait]
impl ModelProvider for AnthropicProvider {
    async fn complete(&self, request: ModelRequest) -> anyhow::Result<ModelResponse> {
        let base_url = self
            .config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.anthropic.com/v1".into());
        let url = format!("{}/messages", base_url.trim_end_matches('/'));
        let system = request
            .messages
            .iter()
            .filter(|message| message.role == "system")
            .map(|message| message.content.clone())
            .collect::<Vec<_>>()
            .join("\n\n");
        let messages = request
            .messages
            .into_iter()
            .filter(|message| message.role != "system")
            .map(|message| {
                json!({
                    "role": if message.role == "assistant" { "assistant" } else { "user" },
                    "content": message.content
                })
            })
            .collect::<Vec<_>>();

        let response = self
            .client
            .post(url)
            .header(
                "x-api-key",
                self.config
                    .api_key
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("missing ANTHROPIC_API_KEY"))?,
            )
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": self.config.model,
                "system": system,
                "messages": messages,
                "max_tokens": 4096
            }))
            .send()
            .await?
            .error_for_status()?;
        let payload: AnthropicMessageResponse = response.json().await?;
        let content = payload
            .content
            .iter()
            .filter(|block| block.r#type == "text")
            .filter_map(|block| block.text.clone())
            .collect::<Vec<_>>()
            .join("");
        let tool_calls = payload
            .content
            .into_iter()
            .filter(|block| block.r#type == "tool_use")
            .filter_map(|block| {
                Some(ToolCall {
                    call_id: block.id?,
                    name: block.name?,
                    arguments: block
                        .input
                        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new())),
                })
            })
            .collect();
        Ok(ModelResponse {
            content,
            tool_calls,
            input_tokens: payload.usage.input_tokens,
            output_tokens: payload.usage.output_tokens,
        })
    }
}

#[derive(Debug, Deserialize)]
struct OpenAiChatCompletion {
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessage {
    #[serde(default)]
    content: String,
    tool_calls: Option<Vec<OpenAiToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolCall {
    id: String,
    function: OpenAiToolFunction,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
}

#[derive(Debug, Deserialize)]
struct AnthropicMessageResponse {
    content: Vec<AnthropicContentBlock>,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    r#type: String,
    text: Option<String>,
    id: Option<String>,
    name: Option<String>,
    input: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: usize,
    output_tokens: usize,
}

fn openai_tools(tools: &[ToolDefinition]) -> Vec<serde_json::Value> {
    tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": {
                        "type": "object",
                        "additionalProperties": true
                    }
                }
            })
        })
        .collect()
}

fn parse_mock_tool_request(input: &str) -> Option<ToolCall> {
    let rest = input.trim().strip_prefix("mock-tool ")?;
    let (name, args) = rest
        .split_once(' ')
        .map(|(name, args)| (name, args))
        .unwrap_or((rest, ""));
    Some(ToolCall {
        call_id: format!("tool_{}", uuid::Uuid::new_v4()),
        name: name.trim().to_string(),
        arguments: parse_key_value_args(args).ok()?,
    })
}

fn parse_mock_usage_request(input: &str) -> anyhow::Result<Option<(usize, usize)>> {
    let Some(rest) = input.trim().strip_prefix("mock-usage ") else {
        return Ok(None);
    };
    let args = parse_key_value_args(rest)?;
    let input_tokens = args
        .get("input")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("0")
        .parse()?;
    let output_tokens = args
        .get("output")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("0")
        .parse()?;
    Ok(Some((input_tokens, output_tokens)))
}

fn parse_key_value_args(value: &str) -> anyhow::Result<serde_json::Value> {
    if value.trim().is_empty() {
        return Ok(serde_json::Value::Object(serde_json::Map::new()));
    }
    match serde_json::from_str(value) {
        Ok(json) => Ok(json),
        Err(error) => {
            let mut object = serde_json::Map::new();
            for pair in value.split(',').filter(|part| !part.trim().is_empty()) {
                let Some((key, raw_value)) = pair.split_once('=') else {
                    return Err(error.into());
                };
                object.insert(
                    key.trim().to_string(),
                    serde_json::Value::String(raw_value.trim().to_string()),
                );
            }
            Ok(serde_json::Value::Object(object))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_mock_tool_request, parse_mock_usage_request, ProviderConfig, ProviderKind};

    #[test]
    fn parses_mock_skill_tool_request() {
        let call =
            parse_mock_tool_request("mock-tool skill-tool name=repo,request=inspect-modules")
                .unwrap();

        assert_eq!(call.name, "skill-tool");
        assert_eq!(call.arguments["name"], "repo");
        assert_eq!(call.arguments["request"], "inspect-modules");
    }

    #[test]
    fn parses_mock_usage_request() {
        let usage = parse_mock_usage_request("mock-usage input=90000,output=1000")
            .unwrap()
            .unwrap();

        assert_eq!(usage, (90000, 1000));
    }

    #[test]
    fn loads_openai_compatible_provider_from_config_toml() {
        let config = ProviderConfig::from_config_text_and_env(
            r#"
            [model]
            provider = "openai-compatible"
            model = "gpt-4.1"
            base_url = "https://example.test/v1"
            api_key = "file-key"
            "#,
            |_| None,
        )
        .unwrap();

        assert_eq!(config.kind, ProviderKind::OpenAi);
        assert_eq!(config.model, "gpt-4.1");
        assert_eq!(config.base_url.as_deref(), Some("https://example.test/v1"));
        assert_eq!(config.api_key.as_deref(), Some("file-key"));
    }

    #[test]
    fn environment_variables_override_config_toml() {
        let config = ProviderConfig::from_config_text_and_env(
            r#"
            [model]
            provider = "openai-compatible"
            model = "gpt-4.1"
            base_url = "https://example.test/v1"
            api_key = "file-key"
            "#,
            |key| match key {
                "UNIO_MODEL_PROVIDER" => Some("anthropic".into()),
                "ANTHROPIC_MODEL" => Some("claude-3-5-haiku-latest".into()),
                "ANTHROPIC_BASE_URL" => Some("https://anthropic.example.test/v1".into()),
                "ANTHROPIC_API_KEY" => Some("env-key".into()),
                _ => None,
            },
        )
        .unwrap();

        assert_eq!(config.kind, ProviderKind::Anthropic);
        assert_eq!(config.model, "claude-3-5-haiku-latest");
        assert_eq!(
            config.base_url.as_deref(),
            Some("https://anthropic.example.test/v1")
        );
        assert_eq!(config.api_key.as_deref(), Some("env-key"));
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
struct UnioConfigFile {
    model: Option<ModelConfigFile>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ModelConfigFile {
    provider: Option<String>,
    base_url: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
}

fn read_default_config_file() -> anyhow::Result<UnioConfigFile> {
    let path = UserPaths::current()?.root.join("config.toml");
    if !path.exists() {
        return Ok(UnioConfigFile::default());
    }
    let config_text = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&config_text)?)
}
