//! The embedded model: llama.cpp through llama-cpp-2, greedy decoding, stops at <|end|>.

use std::num::NonZeroU32;
use std::path::Path;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::token::LlamaToken;

const END_ID: i32 = 5; // <|end|> in the training tokenizer
const EOS_ID: i32 = 2; // </s>
const MAX_TOKENS: usize = 64;
const N_CTX: u32 = 1024;

pub struct Model {
    model: LlamaModel, // declared first: dropped before the backend
    backend: LlamaBackend,
}

impl Model {
    pub fn load(path: &Path) -> Result<Model, String> {
        if !path.is_file() {
            return Err(format!("model file not found: {}", path.display()));
        }
        let mut backend = LlamaBackend::init().map_err(|e| format!("llama backend: {e}"))?;
        backend.void_logs();
        let params = LlamaModelParams::default();
        let model = LlamaModel::load_from_file(&backend, path, &params)
            .map_err(|e| format!("cannot load {}: {e}", path.display()))?;
        Ok(Model { model, backend })
    }

    /// The greedy answer to one question: the text up to <|end|>, trimmed.
    pub fn answer(&self, question: &str) -> Result<String, String> {
        let prompt = format!("<|q|>{question}<|a|>");
        let vocab = self.model.vocab();
        let tokens = vocab.tokenize(prompt.as_bytes(), true, true);
        if tokens.is_empty() || tokens.len() + MAX_TOKENS > N_CTX as usize {
            return Err("the question is too long".into());
        }
        let params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(N_CTX))
            .with_n_batch(N_CTX);
        let mut ctx = self
            .model
            .new_context(&self.backend, params)
            .map_err(|e| format!("context: {e}"))?;
        let mut batch = LlamaBatch::new(N_CTX as usize, 1);
        let last = tokens.len() - 1;
        for (i, token) in tokens.iter().enumerate() {
            batch
                .add(*token, i as i32, &[0], i == last)
                .map_err(|e| format!("batch: {e}"))?;
        }
        ctx.decode(&mut batch).map_err(|e| format!("decode: {e}"))?;

        let mut sampler = LlamaSampler::greedy();
        let mut out: Vec<LlamaToken> = Vec::new();
        let mut pos = tokens.len() as i32;
        for _ in 0..MAX_TOKENS {
            let token = sampler.sample(&ctx, batch.n_tokens() - 1);
            if token.0 == END_ID || token.0 == EOS_ID || vocab.is_eog(token) {
                break;
            }
            out.push(token);
            batch.clear();
            batch
                .add(token, pos, &[0], true)
                .map_err(|e| format!("batch: {e}"))?;
            pos += 1;
            ctx.decode(&mut batch).map_err(|e| format!("decode: {e}"))?;
        }
        let bytes = vocab.detokenize(&out, false, false);
        Ok(String::from_utf8_lossy(&bytes).trim().to_string())
    }
}
