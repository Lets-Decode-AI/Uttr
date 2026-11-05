//! Cloud providers: OpenAI's chat completions API (or any server compatible with it, such as
//! Ollama or LM Studio) and Anthropic's Messages API.

use serde_json::{Value, json};

use super::prompt::Prompt;

pub const OPENAI_URL: &str = "https://api.openai.com/v1";
pub const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1";
/// Sent to OpenRouter so requests are attributed to the app (its "app" leaderboard).
pub const OPENROUTER_HEADERS: &[(&str, &str)] = &[
    ("HTTP-Referer", "https://github.com/Lets-Decode-AI/Uttr"),
    ("X-Title", "uttr"),
];
pub const ANTHROPIC_URL: &str = "https://api.anthropic.com";
const ANTHROPIC_VERSION: &str = "2026-10-21";

/// Turns an HTTP error body into a short message.
fn api_error(status: reqwest::StatusCode, body: &str) -> String {
    let detail = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            v.pointer("/error/message")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| body.chars().take(200).collect());
    match status.as_u16() {
        401 | 403 => format!("The API key was rejected. {detail}"),
        404 => format!("The model was not found. Check the model name. {detail}"),
        429 => format!("The provider is rate limiting requests. {detail}"),
        _ => format!("The provider returned an error ({status}). {detail}"),
    }
}

async fn post(req: reqwest::RequestBuilder) -> Result<Value, String> {
    let resp = req.send().await.map_err(|e| {
        if e.is_timeout() {
            "The AI provider took too long to answer.".to_string()
        } else {
            format!("Could not reach the AI provider. {e}")
        }
    })?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(api_error(status, &body));
    }
    serde_json::from_str(&body).map_err(|e| format!("Unexpected reply from the provider: {e}"))
}

/// `base` is the API root including `/v1` (for example `http://localhost:11434/v1`).
pub async fn openai(
    http: &reqwest::Client,
    base: &str,
    key: Option<&str>,
    model: &str,
    p: &Prompt,
    headers: &[(&str, &str)],
) -> Result<String, String> {
    let url = format!("{}/chat/completions", base.trim_end_matches('/'));
    // No token limit or temperature: reasoning models reject or misuse both.
    let body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": p.system },
            { "role": "user", "content": p.user },
        ],
    });
    let mut req = http.post(url).json(&body);
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    if let Some(k) = key {
        req = req.bearer_auth(k);
    }
    let v = post(req).await?;
    v.pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "The provider sent no text.".into())
}

/// `base` is the API root without `/v1` (tests point it at a mock server).
pub async fn anthropic(
    http: &reqwest::Client,
    base: &str,
    key: &str,
    model: &str,
    p: &Prompt,
) -> Result<String, String> {
    let url = format!("{}/v1/messages", base.trim_end_matches('/'));
    let body = json!({
        "model": model,
        "max_tokens": p.max_tokens.max(1024),
        // Short rewrites: keep latency down.
        "output_config": { "effort": "low" },
        "system": p.system,
        "messages": [{ "role": "user", "content": p.user }],
    });
    let req = http
        .post(url)
        .header("x-api-key", key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .json(&body);
    let v = post(req).await?;
    if v.get("stop_reason").and_then(Value::as_str) == Some("refusal") {
        return Err("The model declined this request.".into());
    }
    let text: String = v
        .get("content")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    if text.trim().is_empty() {
        return Err("The provider sent no text.".into());
    }
    Ok(text)
}

/// One model in OpenRouter's catalog, for the model picker.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterModel {
    pub id: String,
    pub name: String,
    pub context_length: u64,
    /// US dollars per million input and output tokens; `None` when not listed.
    pub input_per_m: Option<f64>,
    pub output_per_m: Option<f64>,
}

/// OpenRouter's public model list (no key needed): text-output models only, sorted by name.
pub async fn openrouter_models(
    http: &reqwest::Client,
    base: &str,
) -> Result<Vec<RouterModel>, String> {
    let url = format!("{}/models", base.trim_end_matches('/'));
    let v = post(http.get(url)).await?;
    let per_m = |m: &Value, k: &str| {
        m.pointer(&format!("/pricing/{k}"))
            .and_then(Value::as_str)
            .and_then(|p| p.parse::<f64>().ok())
            .map(|p| (p * 1_000_000.0 * 100.0).round() / 100.0)
    };
    let mut out: Vec<RouterModel> = v
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|m| {
            m.pointer("/architecture/output_modalities")
                .and_then(Value::as_array)
                .is_none_or(|o| o.iter().any(|x| x == "text"))
        })
        .filter_map(|m| {
            let id = m.get("id")?.as_str()?.to_string();
            Some(RouterModel {
                name: m
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or(&id)
                    .to_string(),
                context_length: m.get("context_length").and_then(Value::as_u64).unwrap_or(0),
                input_per_m: per_m(m, "prompt"),
                output_per_m: per_m(m, "completion"),
                id,
            })
        })
        .collect();
    out.sort_by_key(|m| m.name.to_lowercase());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn prompt() -> Prompt {
        Prompt {
            system: "sys".into(),
            user: "usr".into(),
            max_tokens: 100,
        }
    }

    #[tokio::test]
    async fn openai_sends_chat_messages_and_reads_the_reply() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("authorization", "Bearer sk-test"))
            .and(body_partial_json(json!({
                "model": "m",
                "messages": [{"role": "system", "content": "sys"}, {"role": "user", "content": "usr"}],
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{"message": {"role": "assistant", "content": "Clean."}}]
            })))
            .mount(&server)
            .await;
        let base = format!("{}/v1", server.uri());
        let out = openai(
            &reqwest::Client::new(),
            &base,
            Some("sk-test"),
            "m",
            &prompt(),
            &[],
        )
        .await;
        assert_eq!(out.unwrap(), "Clean.");
    }

    #[tokio::test]
    async fn openai_compatible_server_needs_no_key() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{"message": {"content": "ok"}}]
            })))
            .mount(&server)
            .await;
        let base = format!("{}/v1/", server.uri());
        let out = openai(
            &reqwest::Client::new(),
            &base,
            None,
            "llama3",
            &prompt(),
            &[],
        )
        .await;
        assert_eq!(out.unwrap(), "ok");
    }

    #[tokio::test]
    async fn openrouter_gets_attribution_headers() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/chat/completions"))
            .and(header("authorization", "Bearer or-key"))
            .and(header("x-title", "uttr"))
            .and(header(
                "http-referer",
                "https://github.com/Lets-Decode-AI/Uttr",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{"message": {"content": "routed"}}]
            })))
            .mount(&server)
            .await;
        let base = format!("{}/api/v1", server.uri());
        let out = openai(
            &reqwest::Client::new(),
            &base,
            Some("or-key"),
            "anthropic/claude-opus-5.5",
            &prompt(),
            OPENROUTER_HEADERS,
        )
        .await;
        assert_eq!(out.unwrap(), "routed");
    }

    #[tokio::test]
    async fn openrouter_model_list_keeps_text_models_with_prices() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": [
                {"id": "z/img", "name": "Zeta Image", "architecture": {"output_modalities": ["image"]}},
                {"id": "b/chat", "name": "beta chat", "context_length": 8192,
                 "pricing": {"prompt": "0.000003", "completion": "0.000015"},
                 "architecture": {"output_modalities": ["text"]}},
                {"id": "a/free", "name": "Alpha", "pricing": {"prompt": "0", "completion": "0"}}
            ]})))
            .mount(&server)
            .await;
        let base = format!("{}/api/v1", server.uri());
        let list = openrouter_models(&reqwest::Client::new(), &base)
            .await
            .unwrap();
        let ids: Vec<&str> = list.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["a/free", "b/chat"]);
        assert_eq!(
            (list[1].input_per_m, list[1].output_per_m),
            (Some(3.0), Some(15.0))
        );
        assert_eq!(list[1].context_length, 8192);
    }

    #[tokio::test]
    async fn anthropic_sends_headers_and_joins_text_blocks() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("x-api-key", "key"))
            .and(header("anthropic-version", ANTHROPIC_VERSION))
            .and(body_partial_json(json!({
                "model": "claude-opus-5-5",
                "system": "sys",
                "messages": [{"role": "user", "content": "usr"}],
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "stop_reason": "end_turn",
                "content": [
                    {"type": "thinking", "thinking": ""},
                    {"type": "text", "text": "Hello "},
                    {"type": "text", "text": "there."}
                ]
            })))
            .mount(&server)
            .await;
        let out = anthropic(
            &reqwest::Client::new(),
            &server.uri(),
            "key",
            "claude-opus-5-5",
            &prompt(),
        )
        .await;
        assert_eq!(out.unwrap(), "Hello there.");
    }

    #[tokio::test]
    async fn anthropic_refusal_is_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "stop_reason": "refusal", "content": []
            })))
            .mount(&server)
            .await;
        let out = anthropic(&reqwest::Client::new(), &server.uri(), "k", "m", &prompt()).await;
        assert!(out.unwrap_err().contains("declined"));
    }

    #[tokio::test]
    async fn bad_key_reads_as_a_key_problem() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "error": {"message": "invalid x-api-key"}
            })))
            .mount(&server)
            .await;
        let out = anthropic(&reqwest::Client::new(), &server.uri(), "k", "m", &prompt()).await;
        let e = out.unwrap_err();
        assert!(
            e.contains("API key was rejected") && e.contains("invalid x-api-key"),
            "{e}"
        );
    }
}
