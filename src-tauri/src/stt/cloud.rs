//! Bring-your-own-key speech-to-text: speech chunks are sent to an OpenAI-compatible
//! `/audio/transcriptions` endpoint (OpenAI, Groq, or a self-hosted server) with the user's key.

use std::sync::{Arc, Mutex};

use futures_util::future::BoxFuture;

use super::{Chunk, SttError, join_chunks};
use crate::app_state::Settings;
use crate::dictation::{ModelSource, SessionPort, SttPort};
use crate::llm::{KEYRING_STT, Keys};
use crate::models::InstalledModel;
use crate::models::catalog::{CatalogEntry, EngineKind};

/// Model ids of cloud dictations start with this, for example `cloud:whisper-1`.
pub const CLOUD_PREFIX: &str = "cloud:";
pub const OPENAI_STT_URL: &str = "https://api.openai.com/v1";
pub const GROQ_STT_URL: &str = "https://api.groq.com/openai/v1";
/// Shown when cloud speech is on but its key is missing.
pub const NEEDS_KEY: &str = "Add your speech API key on the Models page.";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SttProvider {
    /// sherpa-onnx on this computer.
    #[default]
    Local,
    /// A cloud service with the user's own API key.
    Cloud,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CloudConfig {
    /// API root including `/v1`.
    pub base_url: String,
    pub model: String,
    /// ISO code; `None` lets the service detect it.
    pub language: Option<String>,
    pub key: Option<String>,
}

/// 16 kHz mono 16-bit PCM WAV.
pub fn wav(samples: &[f32]) -> Vec<u8> {
    const RATE: u32 = crate::audio::SAMPLE_RATE;
    let data = (samples.len() * 2) as u32;
    let mut w = Vec::with_capacity(44 + data as usize);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&1u16.to_le_bytes()); // mono
    w.extend_from_slice(&RATE.to_le_bytes());
    w.extend_from_slice(&(RATE * 2).to_le_bytes()); // byte rate
    w.extend_from_slice(&2u16.to_le_bytes()); // block align
    w.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    w.extend_from_slice(b"data");
    w.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        w.extend_from_slice(&v.to_le_bytes());
    }
    w
}

/// Hosts that always need a key (a self-hosted server may not).
pub fn needs_key(base_url: &str) -> bool {
    let base = base_url.trim_end_matches('/');
    [OPENAI_STT_URL, GROQ_STT_URL].contains(&base)
}

/// Transcribes one chunk.
pub async fn transcribe(
    http: &reqwest::Client,
    cfg: &CloudConfig,
    samples: &[f32],
) -> Result<String, SttError> {
    let fail = |m: String| SttError::EngineFailed(m);
    let file = reqwest::multipart::Part::bytes(wav(samples))
        .file_name("speech.wav")
        .mime_str("audio/wav")
        .map_err(|e| fail(e.to_string()))?;
    let mut form = reqwest::multipart::Form::new()
        .part("file", file)
        .text("model", cfg.model.clone())
        .text("response_format", "json");
    if let Some(lang) = &cfg.language {
        form = form.text("language", lang.clone());
    }
    let url = format!(
        "{}/audio/transcriptions",
        cfg.base_url.trim_end_matches('/')
    );
    let mut req = http.post(url).multipart(form);
    if let Some(k) = &cfg.key {
        req = req.bearer_auth(k);
    }
    let resp = req.send().await.map_err(|e| {
        fail(if e.is_timeout() {
            "The speech service took too long to answer.".into()
        } else {
            format!("Could not reach the speech service. {e}")
        })
    })?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| fail(e.to_string()))?;
    match status.as_u16() {
        200..=299 => {}
        401 | 403 => return Err(fail("The speech API key was rejected.".into())),
        404 => return Err(fail("The speech service does not know this model.".into())),
        429 => return Err(fail("The speech service is rate limiting requests.".into())),
        _ => {
            let detail: String = body.chars().take(200).collect();
            return Err(fail(format!(
                "The speech service returned an error ({status}). {detail}"
            )));
        }
    }
    let v: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| fail(format!("Unexpected reply: {e}")))?;
    v.get("text")
        .and_then(serde_json::Value::as_str)
        .map(|t| t.trim().to_string())
        .ok_or_else(|| fail("The speech service sent no text.".into()))
}

/// The stand-in "installed model" of a cloud dictation.
pub fn virtual_model(model: &str) -> InstalledModel {
    InstalledModel {
        entry: CatalogEntry {
            id: format!("{CLOUD_PREFIX}{model}"),
            name: model.to_string(),
            hf_repo: String::new(),
            revision: String::new(),
            files: vec![],
            engine: EngineKind::Cloud,
            languages: vec![],
            min_ram_gb: 0,
            notes: String::new(),
        },
        dir: std::path::PathBuf::new(),
    }
}

/// A dictation transcribed in the cloud: each chunk is uploaded as soon as it is cut.
pub struct CloudSession {
    http: reqwest::Client,
    cfg: CloudConfig,
    rt: tokio::runtime::Handle,
    jobs: Mutex<Vec<tokio::task::JoinHandle<Result<String, SttError>>>>,
}

impl CloudSession {
    pub fn new(http: reqwest::Client, cfg: CloudConfig, rt: tokio::runtime::Handle) -> Self {
        Self {
            http,
            cfg,
            rt,
            jobs: Mutex::default(),
        }
    }
}

impl SessionPort for CloudSession {
    fn feed(&self, chunk: Chunk) {
        if !chunk.has_speech || chunk.samples.is_empty() {
            return;
        }
        let (http, cfg) = (self.http.clone(), self.cfg.clone());
        let job = self
            .rt
            .spawn(async move { transcribe(&http, &cfg, &chunk.samples).await });
        self.jobs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(job);
    }
    fn finish(
        self: Box<Self>,
        tail: Option<Chunk>,
    ) -> BoxFuture<'static, Result<String, SttError>> {
        if let Some(t) = tail {
            self.feed(t);
        }
        let jobs = std::mem::take(&mut *self.jobs.lock().unwrap_or_else(|p| p.into_inner()));
        Box::pin(async move {
            let mut texts = Vec::with_capacity(jobs.len());
            for job in jobs {
                let text = job
                    .await
                    .map_err(|_| SttError::EngineFailed("a speech upload crashed".into()))??;
                texts.push(text);
            }
            Ok(join_chunks(&texts))
        })
    }
    fn cancel(self: Box<Self>) {
        for job in self
            .jobs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .drain(..)
        {
            job.abort();
        }
    }
}

/// Sends each dictation to the local engine or the cloud, as the settings say.
pub struct SpeechRouter {
    local: Arc<dyn SttPort>,
    models: Arc<dyn ModelSource>,
    settings: Arc<Mutex<Settings>>,
    keys: Keys,
    http: reqwest::Client,
}

impl SpeechRouter {
    pub fn new(
        local: Arc<dyn SttPort>,
        models: Arc<dyn ModelSource>,
        settings: Arc<Mutex<Settings>>,
        keys: Keys,
    ) -> Self {
        Self {
            local,
            models,
            settings,
            keys,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap_or_default(),
        }
    }

    fn config(&self) -> CloudConfig {
        let s = self.settings.lock().unwrap_or_else(|p| p.into_inner());
        CloudConfig {
            base_url: s.stt_cloud_base_url.clone(),
            model: s.stt_cloud_model.clone(),
            language: Some(s.stt_cloud_language.trim().to_string()).filter(|l| !l.is_empty()),
            key: self.keys.get(KEYRING_STT),
        }
    }

    fn cloud(&self) -> bool {
        self.settings
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .stt_provider
            == SttProvider::Cloud
    }
}

impl SttPort for SpeechRouter {
    fn session(&self, model: InstalledModel) -> Box<dyn SessionPort> {
        if model.entry.id.starts_with(CLOUD_PREFIX) {
            Box::new(CloudSession::new(
                self.http.clone(),
                self.config(),
                tokio::runtime::Handle::current(),
            ))
        } else {
            self.local.session(model)
        }
    }
}

impl ModelSource for SpeechRouter {
    fn active(&self) -> Option<InstalledModel> {
        if !self.cloud() {
            return self.models.active();
        }
        let cfg = self.config();
        (self.unavailable().is_none()).then(|| virtual_model(&cfg.model))
    }
    fn unavailable(&self) -> Option<String> {
        if !self.cloud() {
            return self.models.unavailable();
        }
        let cfg = self.config();
        (cfg.key.is_none() && needs_key(&cfg.base_url)).then(|| NEEDS_KEY.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    fn cfg(base: String, key: Option<&str>) -> CloudConfig {
        CloudConfig {
            base_url: base,
            model: "whisper-1".into(),
            language: Some("en".into()),
            key: key.map(String::from),
        }
    }

    fn chunk(n: usize, speech: bool) -> Chunk {
        Chunk {
            samples: vec![0.25; n],
            has_speech: speech,
            start_ms: 0,
        }
    }

    #[test]
    fn wav_has_a_16k_mono_16_bit_header_and_clamps() {
        let w = wav(&[0.0, 1.0, -1.0, 2.0]);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..12], b"WAVE");
        assert_eq!(u16::from_le_bytes([w[22], w[23]]), 1, "mono");
        assert_eq!(u32::from_le_bytes(w[24..28].try_into().unwrap()), 16_000);
        assert_eq!(u16::from_le_bytes([w[34], w[35]]), 16, "bits");
        assert_eq!(
            u32::from_le_bytes(w[40..44].try_into().unwrap()),
            8,
            "data bytes"
        );
        let s = |i: usize| i16::from_le_bytes([w[44 + 2 * i], w[45 + 2 * i]]);
        assert_eq!((s(0), s(1), s(2), s(3)), (0, i16::MAX, -i16::MAX, i16::MAX));
        assert_eq!(w.len(), 44 + 8);
    }

    #[test]
    fn known_hosts_need_a_key() {
        assert!(needs_key(OPENAI_STT_URL));
        assert!(needs_key("https://api.groq.com/openai/v1/"));
        assert!(!needs_key("http://localhost:8000/v1"));
    }

    #[tokio::test]
    async fn uploads_a_wav_with_model_language_and_key() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/audio/transcriptions"))
            .and(header("authorization", "Bearer sk-stt"))
            .respond_with(|req: &Request| {
                let body = String::from_utf8_lossy(&req.body);
                let ok = body.contains("name=\"model\"\r\n\r\nwhisper-1")
                    && body.contains("name=\"language\"\r\n\r\nen")
                    && body.contains("filename=\"speech.wav\"")
                    && body.contains("RIFF");
                if ok {
                    ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!({"text": " Hello there. "}))
                } else {
                    ResponseTemplate::new(400).set_body_string(body.to_string())
                }
            })
            .mount(&server)
            .await;
        let c = cfg(format!("{}/v1", server.uri()), Some("sk-stt"));
        let text = transcribe(&reqwest::Client::new(), &c, &[0.1; 1600])
            .await
            .unwrap();
        assert_eq!(text, "Hello there.");
    }

    #[tokio::test]
    async fn a_rejected_key_is_reported_plainly() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(401).set_body_string("{}"))
            .mount(&server)
            .await;
        let c = cfg(format!("{}/v1", server.uri()), Some("bad"));
        let e = transcribe(&reqwest::Client::new(), &c, &[0.1; 160])
            .await
            .unwrap_err();
        assert!(
            e.to_string().contains("The speech API key was rejected."),
            "{e}"
        );
    }

    /// Answers with the chunk's length in samples, slower for the first request.
    struct ByLength;
    impl Respond for ByLength {
        fn respond(&self, req: &Request) -> ResponseTemplate {
            let body = &req.body;
            // The WAV's data size sits 40 bytes after "RIFF".
            let at = body.windows(4).position(|w| w == b"RIFF").unwrap() + 40;
            let bytes = u32::from_le_bytes(body[at..at + 4].try_into().unwrap());
            let samples = bytes / 2;
            let delay = if samples == 3200 { 300 } else { 0 };
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(delay))
                .set_body_json(serde_json::json!({ "text": format!("part{samples}") }))
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_session_joins_chunks_in_order_and_skips_silence() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ByLength)
            .expect(3)
            .mount(&server)
            .await;
        let c = cfg(format!("{}/v1", server.uri()), Some("k"));
        let s = Box::new(CloudSession::new(
            reqwest::Client::new(),
            c,
            tokio::runtime::Handle::current(),
        ));
        s.feed(chunk(3200, true)); // slow answer
        s.feed(chunk(1000, false)); // silence: never sent
        s.feed(chunk(1600, true));
        let text = s.finish(Some(chunk(800, true))).await.unwrap();
        assert_eq!(text, "part3200 part1600 part800");
    }

    #[test]
    fn virtual_models_are_marked_as_cloud() {
        let m = virtual_model("whisper-large-v3-turbo");
        assert_eq!(m.entry.id, "cloud:whisper-large-v3-turbo");
        assert_eq!(m.entry.engine, EngineKind::Cloud);
    }

    struct LocalOnly(Option<InstalledModel>);
    impl ModelSource for LocalOnly {
        fn active(&self) -> Option<InstalledModel> {
            self.0.clone()
        }
    }
    struct NoStt;
    impl SttPort for NoStt {
        fn session(&self, _m: InstalledModel) -> Box<dyn SessionPort> {
            unreachable!("the local engine is not used for cloud dictations")
        }
    }

    fn router(provider: SttProvider, base: &str, key: Option<&str>) -> SpeechRouter {
        let keys = Keys::default();
        keys.set(KEYRING_STT, key.map(String::from));
        let settings = Settings {
            stt_provider: provider,
            stt_cloud_base_url: base.into(),
            stt_cloud_model: "whisper-1".into(),
            ..Settings::default()
        };
        SpeechRouter::new(
            Arc::new(NoStt),
            Arc::new(LocalOnly(Some(virtual_model("x")).map(|mut m| {
                m.entry.id = "parakeet-v2".into();
                m
            }))),
            Arc::new(Mutex::new(settings)),
            keys,
        )
    }

    #[test]
    fn local_provider_uses_the_installed_model() {
        let r = router(SttProvider::Local, OPENAI_STT_URL, None);
        assert_eq!(r.active().unwrap().entry.id, "parakeet-v2");
        assert_eq!(r.unavailable(), None);
    }

    #[test]
    fn cloud_without_a_key_is_not_ready() {
        let r = router(SttProvider::Cloud, OPENAI_STT_URL, None);
        assert!(r.active().is_none());
        assert_eq!(r.unavailable().as_deref(), Some(NEEDS_KEY));
    }

    #[test]
    fn cloud_with_a_key_or_a_keyless_server_is_ready() {
        let r = router(SttProvider::Cloud, OPENAI_STT_URL, Some("k"));
        assert_eq!(r.active().unwrap().entry.id, "cloud:whisper-1");
        let r = router(SttProvider::Cloud, "http://localhost:8000/v1", None);
        assert_eq!(r.active().unwrap().entry.id, "cloud:whisper-1");
    }
}
