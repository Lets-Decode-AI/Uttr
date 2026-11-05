//! End-to-end check of bring-your-own-key speech: uttr's real cloud session uploads real
//! speech, chunk by chunk, to a real OpenAI-compatible `/v1/audio/transcriptions` server on
//! localhost, which transcribes it with Parakeet. Ignored unless run with `--ignored`; it
//! needs the model directory used by `stt_integration`:
//!
//!   UTTR_MODEL_DIR="$HOME/Library/Caches/uttr Test é/parakeet-v2" \
//!     cargo test --release --test byok_integration -- --ignored --nocapture

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use uttr_lib::audio::{FRAME_LEN, SAMPLE_RATE};
use uttr_lib::dictation::SessionPort;
use uttr_lib::models::InstalledModel;
use uttr_lib::models::catalog::CatalogEntry;
use uttr_lib::stt::cloud::{CloudConfig, CloudSession};
use uttr_lib::stt::{ChunkPlanner, ChunkerConfig, SherpaEngine, SttEngine};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

fn model() -> Option<InstalledModel> {
    let dir = PathBuf::from(std::env::var_os("UTTR_MODEL_DIR")?);
    let entry: CatalogEntry =
        serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).expect("manifest.json"))
            .expect("manifest.json is a catalog entry");
    Some(InstalledModel { entry, dir })
}

fn fixture(name: &str) -> (Vec<f32>, String) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let expected: std::collections::BTreeMap<String, String> =
        serde_json::from_slice(&std::fs::read(dir.join("expected.json")).unwrap()).unwrap();
    let mut r = hound::WavReader::open(dir.join(name)).unwrap();
    assert_eq!(r.spec().sample_rate, SAMPLE_RATE);
    let samples = r
        .samples::<i16>()
        .map(|s| s.unwrap() as f32 / 32768.0)
        .collect();
    (samples, expected[name].clone())
}

fn words(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric() || *c == '\'')
                .collect::<String>()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

fn wer(reference: &str, hypothesis: &str) -> f32 {
    let (r, h) = (words(reference), words(hypothesis));
    let mut prev: Vec<usize> = (0..=h.len()).collect();
    for i in 1..=r.len() {
        let mut cur = vec![i];
        for j in 1..=h.len() {
            let sub = prev[j - 1] + usize::from(r[i - 1] != h[j - 1]);
            cur.push(sub.min(prev[j] + 1).min(cur[j - 1] + 1));
        }
        prev = cur;
    }
    prev[h.len()] as f32 / r.len() as f32
}

/// A transcription server: reads the uploaded WAV, checks the form, runs Parakeet on it.
struct Whisperish {
    engine: Mutex<SherpaEngine>,
    requests: Arc<AtomicUsize>,
}

impl Respond for Whisperish {
    fn respond(&self, req: &Request) -> ResponseTemplate {
        let body = &req.body;
        let text = String::from_utf8_lossy(body);
        if !text.contains("name=\"model\"\r\n\r\nwhisper-1")
            || !text.contains("filename=\"speech.wav\"")
        {
            return ResponseTemplate::new(400).set_body_string("bad form");
        }
        let riff = body
            .windows(4)
            .position(|w| w == b"RIFF")
            .expect("a WAV part");
        let len = u32::from_le_bytes(body[riff + 40..riff + 44].try_into().unwrap()) as usize;
        let pcm = &body[riff + 44..riff + 44 + len];
        let samples: Vec<f32> = pcm
            .chunks_exact(2)
            .map(|b| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32768.0)
            .collect();
        self.requests.fetch_add(1, Ordering::SeqCst);
        let out = self
            .engine
            .lock()
            .unwrap()
            .transcribe(&samples)
            .expect("transcribe");
        ResponseTemplate::new(200).set_body_json(serde_json::json!({ "text": out }))
    }
}

/// Streams `samples` through the chunk planner into a cloud session, as a dictation does.
async fn dictate(server: &MockServer, samples: &[f32]) -> String {
    let cfg = CloudConfig {
        base_url: format!("{}/v1", server.uri()),
        model: "whisper-1".into(),
        language: None,
        key: Some("sk-local-test".into()),
    };
    let session = Box::new(CloudSession::new(
        reqwest::Client::new(),
        cfg,
        tokio::runtime::Handle::current(),
    ));
    let mut planner = ChunkPlanner::new(ChunkerConfig::default());
    for frame in samples.chunks(FRAME_LEN) {
        for chunk in planner.push(frame) {
            session.feed(chunk);
        }
    }
    session
        .finish(planner.finish())
        .await
        .expect("cloud transcription")
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs UTTR_MODEL_DIR (see scripts/fetch_test_model.sh)"]
async fn byok_session_transcribes_real_speech_through_a_real_server() {
    let Some(m) = model() else {
        return eprintln!("UTTR_MODEL_DIR not set; skipping");
    };
    let mut engine = SherpaEngine::new();
    engine.load(&m).expect("load model");
    let requests = Arc::new(AtomicUsize::new(0));
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/audio/transcriptions"))
        .and(header("authorization", "Bearer sk-local-test"))
        .respond_with(Whisperish {
            engine: Mutex::new(engine),
            requests: requests.clone(),
        })
        .mount(&server)
        .await;

    for name in ["s0.wav", "s1.wav", "s2.wav"] {
        let (samples, expected) = fixture(name);
        let text = dictate(&server, &samples).await;
        let w = wer(&expected, &text);
        eprintln!("{name}: WER {w:.3}\n  -> {text}");
        assert!(w < 0.08, "{name}: WER {w}");
    }

    // A long dictation (two recordings with a pause) is cut into several chunks; they must
    // come back in order.
    let (a, ta) = fixture("s3.wav");
    let (b, tb) = fixture("s4.wav");
    let before = requests.load(Ordering::SeqCst);
    let mut long = a;
    long.extend(std::iter::repeat_n(0.0, SAMPLE_RATE as usize));
    long.extend(b);
    let text = dictate(&server, &long).await;
    let chunks = requests.load(Ordering::SeqCst) - before;
    let w = wer(&format!("{ta} {tb}"), &text);
    eprintln!("long ({chunks} chunks): WER {w:.3}\n  -> {text}");
    assert!(chunks >= 2, "a long dictation is uploaded in pieces");
    assert!(w < 0.1, "long: WER {w}");
}
