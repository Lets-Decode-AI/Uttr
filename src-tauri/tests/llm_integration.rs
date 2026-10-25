//! Real-model tests of the on-device AI (llama.cpp). Ignored unless run with `--ignored`; they
//! need a GGUF chat model, for example the catalog's Qwen2.5 1.5B Instruct:
//!
//!   UTTR_LLM_MODEL="$HOME/Downloads/qwen2.5-1.5b-instruct-q4_k_m.gguf" \
//!     cargo test --release --test llm_integration -- --ignored --nocapture

use std::path::PathBuf;
use std::time::Instant;

use uttr_lib::llm::LocalLlm;
use uttr_lib::llm::prompt::{self, Prompt};
use uttr_lib::style::Style;

fn model() -> Option<PathBuf> {
    std::env::var_os("UTTR_LLM_MODEL").map(PathBuf::from)
}

fn run(llm: &LocalLlm, path: &PathBuf, p: Prompt) -> String {
    let started = Instant::now();
    let out = prompt::tidy(&llm.complete(path, p).expect("generation works"));
    eprintln!("{:?}: {out:?}", started.elapsed());
    out
}

#[test]
#[ignore = "needs UTTR_LLM_MODEL"]
fn cleanup_removes_fillers_and_keeps_the_correction() {
    let Some(path) = model() else { return };
    let llm = LocalLlm::spawn(None);
    let raw = "um so I think we should uh meet on Tuesday no wait Wednesday at ten";
    let out = run(&llm, &path, prompt::cleanup(raw, Style::Formal, &["Priya".into()]));
    assert!(prompt::plausible_cleanup(raw, &out), "{out}");
    let lower = out.to_lowercase();
    assert!(lower.contains("wednesday"), "{out}");
    assert!(!lower.split_whitespace().any(|w| w.trim_matches(',') == "um"), "{out}");

    // The model stays loaded: a second request reuses it.
    let raw = "can you send priya the notes from today";
    let out = run(&llm, &path, prompt::cleanup(raw, Style::Formal, &["Priya".into()]));
    assert!(out.contains("Priya"), "{out}");
    assert!(!out.to_lowercase().starts_with("sure"), "the model must not answer: {out}");
}

#[test]
#[ignore = "needs UTTR_LLM_MODEL"]
fn command_rewrites_the_selection() {
    let Some(path) = model() else { return };
    let llm = LocalLlm::spawn(None);
    let out = run(
        &llm,
        &path,
        prompt::command("translate this to French", Some("Good morning, my friend.")),
    );
    assert!(out.to_lowercase().contains("bonjour"), "{out}");
}
