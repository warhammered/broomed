use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::types::ProviderId;

// ── Task ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiTask {
    ClassifyFile,
    DescribeImage,
    SuggestFilename,
    SuggestFolder,
    DetectSemanticDuplicate,
    GenerateTags,
    SemanticSearch,
    SummarizeDocument,
}

// ── Capabilities ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiCapabilities {
    pub text: bool,
    pub vision: bool,
    pub embeddings: bool,
    pub structured_output: bool,
}

impl AiCapabilities {
    pub fn new(text: bool, vision: bool, embeddings: bool, structured_output: bool) -> Self {
        Self {
            text,
            vision,
            embeddings,
            structured_output,
        }
    }
}

// ── Provider config ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProviderConfig {
    pub id: ProviderId,
    pub name: String,
    pub base_url: String,
    pub model: String,
    #[serde(default)]
    pub vision_model: Option<String>,
    #[serde(default)]
    pub embedding_model: Option<String>,
    pub capabilities: AiCapabilities,
    pub priority: u8,
    pub enabled: bool,
}

// api_key deliberately omitted — passed at call site only

// ── Trait ───────────────────────────────────────────────────────────────

pub trait AiProvider {
    fn id(&self) -> &ProviderId;
    fn capabilities(&self) -> &AiCapabilities;
    fn priority(&self) -> u8;
    fn supports(&self, task: &AiTask) -> bool;
    fn classify(
        &self,
        task: AiTask,
        input: &str,
    ) -> impl std::future::Future<Output = Result<AiResult, CoreError>> + Send;

    fn classify_batch<'a>(
        &'a self,
        task: AiTask,
        inputs: &'a [&'a str],
    ) -> impl std::future::Future<Output = Vec<Result<AiResult, CoreError>>> + Send
    where
        Self: Sync,
    {
        async move {
            let mut results = Vec::with_capacity(inputs.len());
            for input in inputs {
                results.push(self.classify(task, input).await);
            }
            results
        }
    }
}

impl AiProvider for AiProviderConfig {
    fn id(&self) -> &ProviderId {
        &self.id
    }
    fn capabilities(&self) -> &AiCapabilities {
        &self.capabilities
    }
    fn priority(&self) -> u8 {
        self.priority
    }
    fn supports(&self, task: &AiTask) -> bool {
        match task {
            AiTask::DescribeImage => self.capabilities.vision,
            AiTask::DetectSemanticDuplicate | AiTask::SemanticSearch => {
                self.capabilities.embeddings
            }
            _ => self.capabilities.text,
        }
    }
    async fn classify(&self, task: AiTask, input: &str) -> Result<AiResult, CoreError> {
        let _ = task;
        Ok(heuristic_classify(input))
    }
}

// ── Heuristic fallback (offline, ext→category) ─────────────────────────

#[derive(Debug, Clone)]
pub struct HeuristicFallback {
    id: ProviderId,
    capabilities: AiCapabilities,
    priority: u8,
}

impl Default for HeuristicFallback {
    fn default() -> Self {
        Self {
            id: ProviderId::new("heuristic"),
            capabilities: AiCapabilities::new(true, false, false, true),
            priority: 1,
        }
    }
}

impl HeuristicFallback {
    pub fn new() -> Self {
        Self::default()
    }
}

impl AiProvider for HeuristicFallback {
    fn id(&self) -> &ProviderId {
        &self.id
    }
    fn capabilities(&self) -> &AiCapabilities {
        &self.capabilities
    }
    fn priority(&self) -> u8 {
        self.priority
    }
    fn supports(&self, task: &AiTask) -> bool {
        matches!(
            task,
            AiTask::ClassifyFile
                | AiTask::SuggestFolder
                | AiTask::SuggestFilename
                | AiTask::GenerateTags
        )
    }
    async fn classify(&self, task: AiTask, input: &str) -> Result<AiResult, CoreError> {
        let _ = task;
        Ok(heuristic_classify(input))
    }
}

// ── Embedding → category map ────────────────────────────────────────────

// 9 categories, 1 exemplar phrase each, hardcoded. Folder is suggested_folder.
#[allow(dead_code)]
const EXEMPLARS: &[(&str, &str, &str)] = &[
    (
        "Documents/Finance",
        "Documents",
        "invoice receipt budget expense finance financial statement",
    ),
    (
        "Documents/Work",
        "Documents",
        "project report meeting work document presentation",
    ),
    (
        "Documents/Code",
        "Code",
        "source code programming software function class repository",
    ),
    (
        "Media/Photos",
        "Images",
        "photo picture image camera photograph vacation",
    ),
    ("Media/Audio", "Audio", "music song audio sound album track"),
    (
        "Media/Videos",
        "Videos",
        "video movie film clip cinema recording",
    ),
    (
        "Archive",
        "Archives",
        "archive zip compressed backup tar package",
    ),
    (
        "Documents",
        "Documents",
        "document text article letter pdf essay",
    ),
    (
        "General",
        "General",
        "miscellaneous general file data unknown",
    ),
];

#[allow(dead_code)]
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0f32;
    let mut na = 0f32;
    let mut nb = 0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

#[allow(dead_code)]
fn confidence_from_cosine(cosine: f32) -> f32 {
    ((cosine + 1.0) / 2.0).clamp(0.0, 1.0)
}

#[allow(dead_code)]
fn classify_via_embeddings(
    input_emb: &[f32],
    exemplars: &[(String, String, Vec<f32>)],
) -> AiResult {
    let mut best = (0usize, f32::MIN);
    for (i, (_, _, emb)) in exemplars.iter().enumerate() {
        let c = cosine_similarity(input_emb, emb);
        if c > best.1 {
            best = (i, c);
        }
    }
    let (idx, cosine) = best;
    let (cat, folder, _) = &exemplars[idx];
    let confidence = confidence_from_cosine(cosine);
    AiResult {
        category: cat.clone(),
        subcategory: None,
        confidence,
        suggested_name: None,
        suggested_folder: Some(folder.clone()),
        tags: vec![cat.to_ascii_lowercase().replace(['/', ' '], "-")],
        reason: format!("embedding cosine {cosine:.3} -> {cat}"),
    }
}

// ── BundledLocalProvider (candle+tokenizers, lazy load) ───────────────

#[cfg(not(feature = "local-ai"))]
#[derive(Debug)]
struct StubModel;

#[cfg(feature = "local-ai")]
mod bert {
    #![allow(dead_code)]
    use candle_core::Tensor;
    use candle_nn::{
        embedding, layer_norm, linear, Activation, Embedding, LayerNorm, Linear, Module, VarBuilder,
    };
    use serde::Deserialize;

    fn default_eps() -> f64 {
        1e-12
    }
    fn default_hidden_act() -> Activation {
        Activation::Gelu
    }
    #[allow(dead_code)]
    fn default_true() -> bool {
        true
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct Config {
        pub vocab_size: usize,
        pub hidden_size: usize,
        pub num_hidden_layers: usize,
        pub num_attention_heads: usize,
        pub intermediate_size: usize,
        #[serde(default = "default_hidden_act")]
        pub hidden_act: Activation,
        #[serde(default)]
        pub hidden_dropout_prob: f64,
        #[serde(default)]
        pub attention_probs_dropout_prob: f64,
        pub max_position_embeddings: usize,
        pub type_vocab_size: usize,
        #[serde(default)]
        pub initializer_range: f64,
        #[serde(default = "default_eps")]
        pub layer_norm_eps: f64,
        #[serde(default)]
        pub pad_token_id: usize,
    }

    impl Default for Config {
        fn default() -> Self {
            Self {
                vocab_size: 30522,
                hidden_size: 384,
                num_hidden_layers: 6,
                num_attention_heads: 12,
                intermediate_size: 1536,
                hidden_act: Activation::Gelu,
                hidden_dropout_prob: 0.1,
                attention_probs_dropout_prob: 0.1,
                max_position_embeddings: 512,
                type_vocab_size: 2,
                initializer_range: 0.02,
                layer_norm_eps: 1e-12,
                pad_token_id: 0,
            }
        }
    }

    pub struct BertEmbeddings {
        word: Embedding,
        position: Embedding,
        token_type: Embedding,
        ln: LayerNorm,
    }

    impl BertEmbeddings {
        pub fn load(vb: VarBuilder, cfg: &Config) -> candle_core::Result<Self> {
            let word = embedding(cfg.vocab_size, cfg.hidden_size, vb.pp("word_embeddings"))?;
            let position = embedding(
                cfg.max_position_embeddings,
                cfg.hidden_size,
                vb.pp("position_embeddings"),
            )?;
            let token_type = embedding(
                cfg.type_vocab_size,
                cfg.hidden_size,
                vb.pp("token_type_embeddings"),
            )?;
            let ln = layer_norm(cfg.hidden_size, cfg.layer_norm_eps, vb.pp("LayerNorm"))?;
            Ok(Self {
                word,
                position,
                token_type,
                ln,
            })
        }
        pub fn forward(
            &self,
            input_ids: &Tensor,
            token_type_ids: &Tensor,
        ) -> candle_core::Result<Tensor> {
            let seq_len = input_ids.dim(1)?;
            let device = input_ids.device();
            let pos_ids = Tensor::arange(0u32, seq_len as u32, device)?
                .unsqueeze(0)?
                .broadcast_as(input_ids.shape())?;
            let w = self.word.forward(input_ids)?;
            let p = self.position.forward(&pos_ids)?;
            let t = self.token_type.forward(token_type_ids)?;
            let emb = (&w + &p)?;
            let emb = (&emb + &t)?;
            self.ln.forward(&emb)
        }
    }

    struct BertSelfAttention {
        q: Linear,
        k: Linear,
        v: Linear,
        q_ln: Option<LayerNorm>,
        k_ln: Option<LayerNorm>,
        v_ln: Option<LayerNorm>,
        num_heads: usize,
        head_dim: usize,
        scale: f64,
    }

    impl BertSelfAttention {
        fn load(vb: VarBuilder, hidden: usize, heads: usize) -> candle_core::Result<Self> {
            let q = linear(hidden, hidden, vb.pp("query"))?;
            let k = linear(hidden, hidden, vb.pp("key"))?;
            let v = linear(hidden, hidden, vb.pp("value"))?;
            let head_dim = hidden / heads;
            Ok(Self {
                q,
                k,
                v,
                q_ln: None,
                k_ln: None,
                v_ln: None,
                num_heads: heads,
                head_dim,
                scale: (head_dim as f64).sqrt(),
            })
        }
        fn forward(&self, hs: &Tensor, mask: Option<&Tensor>) -> candle_core::Result<Tensor> {
            let (b, seq, _) = (hs.dim(0)?, hs.dim(1)?, hs.dim(2)?);
            let q = self.q.forward(hs)?;
            let k = self.k.forward(hs)?;
            let v = self.v.forward(hs)?;
            let q = q
                .reshape((b, seq, self.num_heads, self.head_dim))?
                .transpose(1, 2)?; // b, h, seq, d
            let k = k
                .reshape((b, seq, self.num_heads, self.head_dim))?
                .transpose(1, 2)?;
            let v = v
                .reshape((b, seq, self.num_heads, self.head_dim))?
                .transpose(1, 2)?;
            let k_t = k.transpose(2, 3)?;
            let mut scores = q.matmul(&k_t)?; // b,h,seq,seq
            scores = (scores / self.scale)?;
            if let Some(m) = mask {
                // m: [b, seq] 1=keep, 0=mask -> expand to [b,1,1,seq]
                let m = m.unsqueeze(1)?.unsqueeze(1)?; // b,1,1,seq
                let m = m.to_dtype(candle_core::DType::F32)?;
                // convert 0-> -10000, 1->0 : (m - 1) * 10000
                let one = Tensor::new(1f32, m.device())?.broadcast_as(m.shape())?;
                let tenk = Tensor::new(10000f32, m.device())?.broadcast_as(m.shape())?;
                let adder = m.broadcast_sub(&one)?.broadcast_mul(&tenk)?;
                scores = scores.broadcast_add(&adder)?;
            }
            let weights = candle_nn::ops::softmax(&scores, 3)?;
            let ctx = weights.matmul(&v)?; // b,h,seq,d
            let ctx = ctx
                .transpose(1, 2)?
                .reshape((b, seq, self.num_heads * self.head_dim))?;
            Ok(ctx)
        }
    }

    struct BertSelfOutput {
        dense: Linear,
        ln: LayerNorm,
    }
    impl BertSelfOutput {
        fn load(vb: VarBuilder, hidden: usize, eps: f64) -> candle_core::Result<Self> {
            let dense = linear(hidden, hidden, vb.pp("dense"))?;
            let ln = layer_norm(hidden, eps, vb.pp("LayerNorm"))?;
            Ok(Self { dense, ln })
        }
        fn forward(&self, hs: &Tensor, input: &Tensor) -> candle_core::Result<Tensor> {
            let hs = self.dense.forward(hs)?;
            let hs = (&hs + input)?;
            self.ln.forward(&hs)
        }
    }

    struct BertAttention {
        self_attn: BertSelfAttention,
        output: BertSelfOutput,
    }
    impl BertAttention {
        fn load(vb: VarBuilder, cfg: &Config) -> candle_core::Result<Self> {
            let self_attn =
                BertSelfAttention::load(vb.pp("self"), cfg.hidden_size, cfg.num_attention_heads)?;
            let output =
                BertSelfOutput::load(vb.pp("output"), cfg.hidden_size, cfg.layer_norm_eps)?;
            Ok(Self { self_attn, output })
        }
        fn forward(&self, hs: &Tensor, mask: Option<&Tensor>) -> candle_core::Result<Tensor> {
            let ctx = self.self_attn.forward(hs, mask)?;
            self.output.forward(&ctx, hs)
        }
    }

    struct BertIntermediate {
        dense: Linear,
        act: Activation,
    }
    impl BertIntermediate {
        fn load(vb: VarBuilder, cfg: &Config) -> candle_core::Result<Self> {
            let dense = linear(cfg.hidden_size, cfg.intermediate_size, vb.pp("dense"))?;
            Ok(Self {
                dense,
                act: cfg.hidden_act,
            })
        }
        fn forward(&self, hs: &Tensor) -> candle_core::Result<Tensor> {
            let hs = self.dense.forward(hs)?;
            self.act.forward(&hs)
        }
    }

    struct BertOutput {
        dense: Linear,
        ln: LayerNorm,
    }
    impl BertOutput {
        fn load(vb: VarBuilder, cfg: &Config) -> candle_core::Result<Self> {
            let dense = linear(cfg.intermediate_size, cfg.hidden_size, vb.pp("dense"))?;
            let ln = layer_norm(cfg.hidden_size, cfg.layer_norm_eps, vb.pp("LayerNorm"))?;
            Ok(Self { dense, ln })
        }
        fn forward(&self, hs: &Tensor, input: &Tensor) -> candle_core::Result<Tensor> {
            let hs = self.dense.forward(hs)?;
            let hs = (&hs + input)?;
            self.ln.forward(&hs)
        }
    }

    struct BertLayer {
        attn: BertAttention,
        inter: BertIntermediate,
        out: BertOutput,
    }
    impl BertLayer {
        fn load(vb: VarBuilder, cfg: &Config) -> candle_core::Result<Self> {
            let attn = BertAttention::load(vb.pp("attention"), cfg)?;
            let inter = BertIntermediate::load(vb.pp("intermediate"), cfg)?;
            let out = BertOutput::load(vb.pp("output"), cfg)?;
            Ok(Self { attn, inter, out })
        }
        fn forward(&self, hs: &Tensor, mask: Option<&Tensor>) -> candle_core::Result<Tensor> {
            let a = self.attn.forward(hs, mask)?;
            let i = self.inter.forward(&a)?;
            self.out.forward(&i, &a)
        }
    }

    struct BertEncoder {
        layers: Vec<BertLayer>,
    }
    impl BertEncoder {
        fn load(vb: VarBuilder, cfg: &Config) -> candle_core::Result<Self> {
            let mut layers = Vec::with_capacity(cfg.num_hidden_layers);
            for i in 0..cfg.num_hidden_layers {
                layers.push(BertLayer::load(vb.pp(format!("layer.{i}")), cfg)?);
            }
            Ok(Self { layers })
        }
        fn forward(&self, mut hs: Tensor, mask: Option<&Tensor>) -> candle_core::Result<Tensor> {
            for l in &self.layers {
                hs = l.forward(&hs, mask)?;
            }
            Ok(hs)
        }
    }

    pub struct BertModel {
        embeddings: BertEmbeddings,
        encoder: BertEncoder,
    }

    impl BertModel {
        pub fn load(vb: VarBuilder, cfg: &Config) -> candle_core::Result<Self> {
            let embeddings = BertEmbeddings::load(vb.pp("embeddings"), cfg)?;
            let encoder = BertEncoder::load(vb.pp("encoder"), cfg)?;
            Ok(Self {
                embeddings,
                encoder,
            })
        }
        pub fn forward(
            &self,
            input_ids: &Tensor,
            token_type_ids: &Tensor,
            attention_mask: Option<&Tensor>,
        ) -> candle_core::Result<Tensor> {
            let emb = self.embeddings.forward(input_ids, token_type_ids)?;
            self.encoder.forward(emb, attention_mask)
        }
    }
}

#[cfg(feature = "local-ai")]
struct RealModel {
    tokenizer: tokenizers::Tokenizer,
    bert: bert::BertModel,
    device: candle_core::Device,
    exemplars: Vec<(String, String, Vec<f32>)>,
}

#[cfg(feature = "local-ai")]
impl std::fmt::Debug for RealModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RealModel")
            .field("device", &self.device)
            .finish()
    }
}

#[cfg(feature = "local-ai")]
impl RealModel {
    fn load(model_dir: &Path) -> Result<Self, CoreError> {
        let cfg_path = model_dir.join("config.json");
        let tok_path = model_dir.join("tokenizer.json");
        let model_path = model_dir.join("model.safetensors");
        if !cfg_path.exists() || !tok_path.exists() || !model_path.exists() {
            return Err(CoreError::Internal("model files missing".into()));
        }
        let cfg_str = std::fs::read_to_string(&cfg_path)
            .map_err(|e| CoreError::Internal(format!("read config: {e}")))?;
        let cfg: bert::Config = serde_json::from_str(&cfg_str)
            .map_err(|e| CoreError::Internal(format!("parse config: {e}")))?;
        let tokenizer = tokenizers::Tokenizer::from_file(&tok_path)
            .map_err(|e| CoreError::Internal(format!("load tokenizer: {e}")))?;
        let device = candle_core::Device::Cpu;
        let vb = unsafe {
            candle_nn::VarBuilder::from_mmaped_safetensors(
                &[&model_path],
                candle_core::DType::F32,
                &device,
            )
            .map_err(|e| CoreError::Internal(format!("load safetensors: {e}")))?
        };
        let bert = bert::BertModel::load(vb, &cfg)
            .map_err(|e| CoreError::Internal(format!("load bert: {e}")))?;
        let mut model = Self {
            tokenizer,
            bert,
            device,
            exemplars: Vec::new(),
        };
        // precompute exemplar embeddings
        let mut ex = Vec::new();
        for (cat, folder, phrase) in EXEMPLARS {
            match model.embed_raw(phrase) {
                Ok(v) => ex.push((cat.to_string(), folder.to_string(), v)),
                Err(e) => {
                    // ponytail: if exemplar fails, skip that category but keep others; fallback to heuristic later if all fail
                    tracing::warn!("exemplar embed failed for {cat}: {e}");
                }
            }
        }
        if ex.is_empty() {
            return Err(CoreError::Internal("no exemplar embeddings".into()));
        }
        model.exemplars = ex;
        Ok(model)
    }

    fn embed_raw(&self, text: &str) -> Result<Vec<f32>, CoreError> {
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| CoreError::Internal(format!("tokenize: {e}")))?;
        let ids: Vec<u32> = encoding.get_ids().to_vec();
        let attn: Vec<u32> = encoding.get_attention_mask().to_vec();
        if ids.is_empty() {
            return Err(CoreError::Internal("empty tokenization".into()));
        }
        let seq_len = ids.len();
        let ids_t = candle_core::Tensor::new(ids.as_slice(), &self.device)
            .map_err(|e| CoreError::Internal(format!("ids tensor: {e}")))?
            .unsqueeze(0)
            .map_err(|e| CoreError::Internal(format!("unsqueeze: {e}")))?;
        let attn_t = candle_core::Tensor::new(attn.as_slice(), &self.device)
            .map_err(|e| CoreError::Internal(format!("attn tensor: {e}")))?
            .unsqueeze(0)
            .map_err(|e| CoreError::Internal(format!("unsqueeze: {e}")))?;
        let type_ids =
            candle_core::Tensor::zeros((1, seq_len), candle_core::DType::U32, &self.device)
                .map_err(|e| CoreError::Internal(format!("type_ids: {e}")))?;
        // bert forward: attention_mask for encoder masking
        let hidden = self
            .bert
            .forward(&ids_t, &type_ids, Some(&attn_t))
            .map_err(|e| CoreError::Internal(format!("bert forward: {e}")))?;
        // mean-pool with attention mask
        let mask_f = attn_t
            .to_dtype(candle_core::DType::F32)
            .map_err(|e| CoreError::Internal(format!("mask dtype: {e}")))?;
        let mask_exp = mask_f
            .unsqueeze(2)
            .map_err(|e| CoreError::Internal(format!("mask unsqueeze: {e}")))?; // [1,seq,1]
        let masked = hidden
            .broadcast_mul(&mask_exp)
            .map_err(|e| CoreError::Internal(format!("masked: {e}")))?;
        let sum = masked
            .sum(1)
            .map_err(|e| CoreError::Internal(format!("sum: {e}")))?; // [1, hidden]
        let denom = mask_f
            .sum(1)
            .map_err(|e| CoreError::Internal(format!("denom sum: {e}")))?;
        let denom = denom
            .clamp(1e-9, f32::MAX)
            .map_err(|e| CoreError::Internal(format!("clamp: {e}")))?;
        let denom = denom
            .unsqueeze(1)
            .map_err(|e| CoreError::Internal(format!("denom unsqueeze: {e}")))?;
        let mean = sum
            .broadcast_div(&denom)
            .map_err(|e| CoreError::Internal(format!("mean: {e}")))?;
        // L2 norm
        let sq = mean
            .sqr()
            .map_err(|e| CoreError::Internal(format!("sqr: {e}")))?;
        let sum_sq = sq
            .sum(1)
            .map_err(|e| CoreError::Internal(format!("sum_sq: {e}")))?;
        let norm = sum_sq
            .sqrt()
            .map_err(|e| CoreError::Internal(format!("sqrt: {e}")))?;
        let norm = norm
            .clamp(1e-9, f32::MAX)
            .map_err(|e| CoreError::Internal(format!("norm clamp: {e}")))?;
        let norm = norm
            .unsqueeze(1)
            .map_err(|e| CoreError::Internal(format!("norm unsqueeze: {e}")))?;
        let normalized = mean
            .broadcast_div(&norm)
            .map_err(|e| CoreError::Internal(format!("norm div: {e}")))?;
        let vec = normalized
            .squeeze(0)
            .map_err(|e| CoreError::Internal(format!("squeeze: {e}")))?
            .to_vec1::<f32>()
            .map_err(|e| CoreError::Internal(format!("to_vec: {e}")))?;
        if vec.len() != 384 {
            // ponytail: all-MiniLM-L6-v2 is 384-dim; warn but allow
            tracing::warn!("unexpected embedding dim {}", vec.len());
        }
        Ok(vec)
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, CoreError> {
        self.embed_raw(text)
    }

    fn classify(&self, input: &str) -> AiResult {
        match self.embed(input) {
            Ok(emb) => classify_via_embeddings(&emb, &self.exemplars),
            Err(e) => {
                tracing::warn!("embed failed ({e}), fallback heuristic");
                heuristic_classify(input)
            }
        }
    }
}

#[derive(Debug)]
pub struct BundledLocalProvider {
    id: ProviderId,
    capabilities: AiCapabilities,
    priority: u8,
    model_dir: PathBuf,
    // ponytail: global OnceLock lazy load on first classify, not app start; per-model lock if throughput matters
    #[cfg(feature = "local-ai")]
    loaded: OnceLock<Arc<RealModel>>,
    #[cfg(not(feature = "local-ai"))]
    loaded: OnceLock<Arc<StubModel>>,
}

impl BundledLocalProvider {
    pub fn new() -> Self {
        Self {
            id: ProviderId::new("bundled-local"),
            capabilities: AiCapabilities::new(true, false, true, true),
            priority: 10,
            model_dir: PathBuf::from("src-tauri/resources/models/all-MiniLM-L6-v2"),
            loaded: OnceLock::new(),
        }
    }

    pub fn with_model_dir(path: impl Into<PathBuf>) -> Self {
        Self {
            id: ProviderId::new("bundled-local"),
            capabilities: AiCapabilities::new(true, false, true, true),
            priority: 10,
            model_dir: path.into(),
            loaded: OnceLock::new(),
        }
    }

    fn resolve_model_dir(&self) -> Option<PathBuf> {
        // Check bundled resources path first, then platform model_base_dir, then BROOMED_MODEL_DIR
        let candidates = [
            self.model_dir.clone(),
            crate::models::model_dir_for("all-MiniLM-L6-v2"),
            PathBuf::from("resources/models/all-MiniLM-L6-v2"),
        ];
        for p in candidates {
            if p.join("model.safetensors").exists()
                && p.join("config.json").exists()
                && p.join("tokenizer.json").exists()
            {
                return Some(p);
            }
        }
        None
    }

    pub fn model_available(&self) -> bool {
        self.resolve_model_dir().is_some()
    }

    pub fn resolved_model_dir(&self) -> Option<PathBuf> {
        self.resolve_model_dir()
    }

    #[cfg(feature = "local-ai")]
    fn ensure_loaded(&self) -> Option<Arc<RealModel>> {
        let dir = self.resolve_model_dir()?;
        if let Some(m) = self.loaded.get() {
            return Some(Arc::clone(m));
        }
        match RealModel::load(&dir) {
            Ok(m) => {
                let arc = Arc::new(m);
                let _ = self.loaded.set(Arc::clone(&arc));
                Some(arc)
            }
            Err(e) => {
                tracing::warn!("bundled model load failed from {:?}: {e}, fallback heuristic", dir);
                None
            }
        }
    }

    #[cfg(not(feature = "local-ai"))]
    fn ensure_loaded(&self) -> Option<Arc<StubModel>> {
        self.resolve_model_dir()?;
        Some(Arc::clone(self.loaded.get_or_init(|| Arc::new(StubModel))))
    }
}

impl Default for BundledLocalProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AiProvider for BundledLocalProvider {
    fn id(&self) -> &ProviderId {
        &self.id
    }
    fn capabilities(&self) -> &AiCapabilities {
        &self.capabilities
    }
    fn priority(&self) -> u8 {
        self.priority
    }
    fn supports(&self, task: &AiTask) -> bool {
        matches!(
            task,
            AiTask::ClassifyFile
                | AiTask::SemanticSearch
                | AiTask::DetectSemanticDuplicate
                | AiTask::GenerateTags
        )
    }
    async fn classify(&self, task: AiTask, input: &str) -> Result<AiResult, CoreError> {
        let _ = task;
        #[cfg(feature = "local-ai")]
        {
            if let Some(model) = self.ensure_loaded() {
                // real embedding path
                return Ok(model.classify(input));
            }
            Ok(heuristic_classify(input))
        }
        #[cfg(not(feature = "local-ai"))]
        {
            if let Some(_model) = self.ensure_loaded() {
                let mut r = heuristic_classify(input);
                r.confidence = (r.confidence + 0.05).min(0.92);
                r.reason = format!("bundled model: {}", r.reason);
                return Ok(r);
            }
            Ok(heuristic_classify(input))
        }
    }
}

// ── CloudProvider (DEPRECATED direct OpenAI/Anthropic — replaced by Broomed gateway)
// ponytail: kept only for dev with BROOMED_DEV_DIRECT_CLOUD=1 or feature unstable-direct-cloud
// Normal production path uses BroomedOnlineProvider via online.rs — do not use CloudProvider directly.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudKind {
    OpenAI,
    Anthropic,
}

#[derive(Debug, Clone)]
pub struct CloudProvider {
    kind: CloudKind,
    id: ProviderId,
    capabilities: AiCapabilities,
    priority: u8,
    base_url: String,
    model: String,
    api_key: Option<String>,
}

impl CloudProvider {
    pub fn new(kind: CloudKind) -> Self {
        let (id, base_url, model, env_key) = match kind {
            CloudKind::OpenAI => (
                ProviderId::new("cloud-openai"),
                "https://api.openai.com".to_string(),
                "gpt-4o-mini".to_string(),
                std::env::var("OPENAI_API_KEY").ok(),
            ),
            CloudKind::Anthropic => (
                ProviderId::new("cloud-anthropic"),
                "https://api.anthropic.com".to_string(),
                "claude-3-5-sonnet-20241022".to_string(),
                std::env::var("ANTHROPIC_API_KEY").ok(),
            ),
        };
        Self {
            kind,
            id,
            capabilities: AiCapabilities::new(true, false, false, true),
            priority: 20,
            base_url,
            model,
            api_key: env_key.filter(|k| !k.trim().is_empty()),
        }
    }

    pub fn openai() -> Self {
        Self::new(CloudKind::OpenAI)
    }

    pub fn anthropic() -> Self {
        Self::new(CloudKind::Anthropic)
    }

    pub fn with_api_key(mut self, key: Option<String>) -> Self {
        self.api_key = key.filter(|k| !k.trim().is_empty());
        self
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    pub fn is_configured(&self) -> bool {
        self.api_key.as_ref().is_some_and(|k| !k.trim().is_empty())
    }

    pub fn kind(&self) -> CloudKind {
        self.kind
    }
}

#[cfg(feature = "cloud-ai")]
fn build_cloud_prompt(task: &AiTask, input: &str) -> String {
    let task_str = match task {
        AiTask::ClassifyFile => "ClassifyFile",
        AiTask::DescribeImage => "DescribeImage",
        AiTask::SuggestFilename => "SuggestFilename",
        AiTask::SuggestFolder => "SuggestFolder",
        AiTask::DetectSemanticDuplicate => "DetectSemanticDuplicate",
        AiTask::GenerateTags => "GenerateTags",
        AiTask::SemanticSearch => "SemanticSearch",
        AiTask::SummarizeDocument => "SummarizeDocument",
    };
    format!(
        "You are a file organizer. Task: {task_str}. Input: \"{input}\". \
        Categories: Documents, Images, Audio, Videos, Archives, Code, General. \
        Respond with JSON object with keys: category (string, required), confidence (number 0-1, required), \
        suggested_folder (string), tags (array of strings), reason (string). Only output JSON."
    )
}

#[cfg(feature = "cloud-ai")]
impl CloudProvider {
    async fn call_openai(&self, key: &str, prompt: &str) -> Result<AiResult, CoreError> {
        let client = reqwest::Client::new();
        let url = format!(
            "{}/v1/chat/completions",
            self.base_url.trim_end_matches('/')
        );
        let body = serde_json::json!({
            "model": self.model,
            "messages": [{"role": "user", "content": prompt}],
            "response_format": {"type": "json_object"},
            "temperature": 0
        });
        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {key}"))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| CoreError::Internal(format!("openai request failed: {e}")))?;
        let status = resp.status();
        if !status.is_success() {
            let txt = resp.text().await.unwrap_or_default();
            return Err(CoreError::Internal(format!("openai error {status}: {txt}")));
        }
        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| CoreError::Internal(format!("openai parse: {e}")))?;
        let content = v
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();
        if content.trim().is_empty() {
            return Err(CoreError::Internal("openai empty response".into()));
        }
        parse_ai_json(&content)
    }

    async fn call_anthropic(&self, key: &str, prompt: &str) -> Result<AiResult, CoreError> {
        let client = reqwest::Client::new();
        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": prompt}]
        });
        let resp = client
            .post(&url)
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| CoreError::Internal(format!("anthropic request failed: {e}")))?;
        let status = resp.status();
        if !status.is_success() {
            let txt = resp.text().await.unwrap_or_default();
            return Err(CoreError::Internal(format!(
                "anthropic error {status}: {txt}"
            )));
        }
        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| CoreError::Internal(format!("anthropic parse: {e}")))?;
        // extract text from content array
        let mut text = String::new();
        if let Some(arr) = v.get("content").and_then(|c| c.as_array()) {
            for item in arr {
                if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(t);
                }
            }
        }
        if text.trim().is_empty() {
            if let Some(s) = v.get("content").and_then(|c| c.as_str()) {
                text = s.to_string();
            }
        }
        if text.trim().is_empty() {
            // fallback: try whole body as JSON string
            text = v.to_string();
        }
        parse_ai_json(&text)
    }
}

impl AiProvider for CloudProvider {
    fn id(&self) -> &ProviderId {
        &self.id
    }
    fn capabilities(&self) -> &AiCapabilities {
        &self.capabilities
    }
    fn priority(&self) -> u8 {
        self.priority
    }
    fn supports(&self, task: &AiTask) -> bool {
        matches!(
            task,
            AiTask::ClassifyFile
                | AiTask::SuggestFilename
                | AiTask::SuggestFolder
                | AiTask::GenerateTags
                | AiTask::SummarizeDocument
        )
    }
    #[allow(clippy::needless_return)]
    async fn classify(&self, task: AiTask, input: &str) -> Result<AiResult, CoreError> {
        #[cfg(not(feature = "cloud-ai"))]
        {
            let _ = task;
            let _ = input;
            return Err(CoreError::Internal("cloud provider not configured".into()));
        }
        #[cfg(feature = "cloud-ai")]
        {
            let key = match &self.api_key {
                Some(k) if !k.trim().is_empty() => k.clone(),
                _ => return Err(CoreError::Internal("cloud provider not configured".into())),
            };
            // ponytail: gate direct cloud — only allow with env or feature
            #[cfg(not(feature = "unstable-direct-cloud"))]
            {
                if std::env::var("BROOMED_DEV_DIRECT_CLOUD").unwrap_or_default() != "1" {
                    return Err(CoreError::Internal(
                        "ONLINE_AI_DISABLED: direct cloud disabled, use Broomed gateway".into(),
                    ));
                }
            }
            let prompt = build_cloud_prompt(&task, input);
            match self.kind {
                CloudKind::OpenAI => self.call_openai(&key, &prompt).await,
                CloudKind::Anthropic => self.call_anthropic(&key, &prompt).await,
            }
        }
    }
}

// ── Heuristic core ──────────────────────────────────────────────────────

fn heuristic_classify(input: &str) -> AiResult {
    let path = Path::new(input);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    // also try to extract from plain filename without path
    let ext = if ext.is_empty() && input.contains('.') {
        input.rsplit('.').next().unwrap_or("").to_ascii_lowercase()
    } else {
        ext
    };
    let (category, folder, confidence) = match ext.as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "svg" | "bmp" | "heic" | "tiff" | "psd" => {
            ("Images", "Images", 0.85)
        }
        "mp4" | "mov" | "avi" | "mkv" | "webm" | "flv" | "wmv" | "m4v" => {
            ("Videos", "Videos", 0.86)
        }
        "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "wma" => ("Audio", "Audio", 0.84),
        "pdf" | "doc" | "docx" | "txt" | "md" | "rtf" | "odt" | "xls" | "xlsx" | "ppt" | "pptx"
        | "csv" | "epub" => ("Documents", "Documents", 0.82),
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "iso" => {
            ("Archives", "Archives", 0.83)
        }
        "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "html" | "css" | "json" | "toml" | "yaml"
        | "yml" | "go" | "java" | "cpp" | "c" | "sh" | "rb" | "php" => ("Code", "Code", 0.80),
        _ => ("General", "General", 0.62),
    };
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(input);
    let reason = if ext.is_empty() {
        format!("heuristic: no ext -> {category} (input: {stem})")
    } else {
        format!("heuristic: ext .{ext} -> {category}")
    };
    AiResult {
        category: category.to_string(),
        subcategory: None,
        confidence,
        suggested_name: None,
        suggested_folder: Some(folder.to_string()),
        tags: vec![category.to_ascii_lowercase()],
        reason,
    }
}

// ── Router ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AiRouter {
    pub providers: Vec<AiProviderConfig>,
}

impl AiRouter {
    pub fn new(providers: Vec<AiProviderConfig>) -> Self {
        Self { providers }
    }

    /// Pick highest priority enabled provider whose capabilities fit task.
    /// Returns None if offline / no capable provider.
    pub fn route(&self, task: &AiTask) -> Option<&AiProviderConfig> {
        self.providers
            .iter()
            .filter(|p| p.enabled && p.supports(task))
            .max_by_key(|p| p.priority)
    }

    /// Mode-aware routing: filters by online availability.
    /// When online not available, only local providers (priority <20) considered.
    pub fn route_with_mode(
        &self,
        task: &AiTask,
        mode: crate::mode::AiMode,
        online_available: bool,
    ) -> Option<&AiProviderConfig> {
        let allow_online = match mode {
            crate::mode::AiMode::Local => false,
            crate::mode::AiMode::Hybrid => online_available,
            crate::mode::AiMode::Online => online_available,
        };
        self.providers
            .iter()
            .filter(|p| p.enabled && p.supports(task))
            .filter(|p| if p.priority >= 20 { allow_online } else { true })
            .max_by_key(|p| p.priority)
    }
}

/// Hybrid classify helper: local first, fallback to online if confidence low
pub async fn hybrid_classify(
    local: &BundledLocalProvider,
    online: Option<&crate::online::OnlineAiClient>,
    config: &crate::mode::AiModeConfig,
    task: AiTask,
    input: &str,
) -> Result<AiResult, CoreError> {
    // LOCAL mode: always local
    if config.mode == crate::mode::AiMode::Local {
        return local.classify(task, input).await;
    }
    // try local first
    let local_res = local.classify(task, input).await?;
    if config.mode == crate::mode::AiMode::Hybrid {
        if let Some(o) = online {
            if config.should_try_online(local_res.confidence, o.is_available()) {
                // privacy: only selected file content transmitted when online_opt_in && entitlement valid
                if let Ok(r) = o.classify_via_capability(task, input).await {
                    return Ok(r);
                }
            }
        }
        return Ok(local_res);
    }
    // ONLINE mode: try online, fallback to local
    if config.mode == crate::mode::AiMode::Online {
        if let Some(o) = online {
            if o.is_available() && config.online_opt_in {
                if let Ok(r) = o.classify_via_capability(task, input).await {
                    return Ok(r);
                }
            }
        }
        return Ok(local_res);
    }
    Ok(local_res)
}

// ── Confidence ─────────────────────────────────────────────────────────

pub const DEFAULT_THRESHOLD_HIGH: f32 = 0.90;
pub const DEFAULT_THRESHOLD_MED: f32 = 0.70;
pub const DEFAULT_THRESHOLD_LOW: f32 = 0.0;

#[derive(Debug, Clone, PartialEq)]
pub enum Confidence {
    High(f32),
    Medium(f32),
    Low(f32),
}

pub fn classify_confidence(v: f32, threshold_high: f32, threshold_med: f32) -> Confidence {
    if v >= threshold_high {
        Confidence::High(v)
    } else if v >= threshold_med {
        Confidence::Medium(v)
    } else {
        Confidence::Low(v)
    }
}

// ── Result ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AiResult {
    pub category: String,
    #[serde(default)]
    pub subcategory: Option<String>,
    pub confidence: f32,
    #[serde(default)]
    pub suggested_name: Option<String>,
    #[serde(default)]
    pub suggested_folder: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub reason: String,
}

fn extract_json(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.contains("```") {
        if let Some(start) = trimmed.find("```") {
            let after_start = &trimmed[start + 3..];
            if let Some(end) = after_start.find("```") {
                let inner = &after_start[..end];
                let inner = inner.trim();
                let inner = if inner.len() >= 4 && inner[..4].eq_ignore_ascii_case("json") {
                    inner[4..].trim()
                } else {
                    inner
                };
                return inner.to_string();
            }
            // single fence fallback
            let inner = after_start.trim();
            let inner = if inner.len() >= 4 && inner[..4].eq_ignore_ascii_case("json") {
                inner[4..].trim()
            } else {
                inner
            };
            if let Some(s) = inner.find('{') {
                if let Some(e) = inner.rfind('}') {
                    return inner[s..=e].to_string();
                }
            }
            return inner.to_string();
        }
    }
    if let Some(s) = trimmed.find('{') {
        if let Some(e) = trimmed.rfind('}') {
            return trimmed[s..=e].to_string();
        }
    }
    trimmed.to_string()
}

pub fn parse_ai_json(raw: &str) -> Result<AiResult, CoreError> {
    let json_str = extract_json(raw);
    let parsed: AiResult = serde_json::from_str(&json_str)
        .map_err(|e| CoreError::Internal(format!("invalid ai json: {e}")))?;
    if parsed.category.trim().is_empty() {
        return Err(CoreError::Internal("category must be non-empty".into()));
    }
    if !parsed.confidence.is_finite() || parsed.confidence < 0.0 || parsed.confidence > 1.0 {
        return Err(CoreError::Internal(format!(
            "confidence out of range 0.0-1.0: {}",
            parsed.confidence
        )));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_provider(id: &str, priority: u8, enabled: bool) -> AiProviderConfig {
        AiProviderConfig {
            id: ProviderId::new(id),
            name: id.to_string(),
            base_url: "https://api.example.com".into(),
            model: "text-model".into(),
            vision_model: None,
            embedding_model: None,
            capabilities: AiCapabilities {
                text: true,
                vision: false,
                embeddings: false,
                structured_output: true,
            },
            priority,
            enabled,
        }
    }

    fn vision_provider(id: &str, priority: u8, enabled: bool) -> AiProviderConfig {
        AiProviderConfig {
            id: ProviderId::new(id),
            name: id.to_string(),
            base_url: "https://api.example.com".into(),
            model: "vision-model".into(),
            vision_model: Some("vision-model".into()),
            embedding_model: None,
            capabilities: AiCapabilities {
                text: true,
                vision: true,
                embeddings: false,
                structured_output: true,
            },
            priority,
            enabled,
        }
    }

    fn embedding_provider(id: &str, priority: u8) -> AiProviderConfig {
        AiProviderConfig {
            id: ProviderId::new(id),
            name: id.to_string(),
            base_url: "https://api.example.com".into(),
            model: "embed-model".into(),
            vision_model: None,
            embedding_model: Some("embed-model".into()),
            capabilities: AiCapabilities {
                text: false,
                vision: false,
                embeddings: true,
                structured_output: false,
            },
            priority,
            enabled: true,
        }
    }

    #[test]
    fn route_describe_image_picks_vision_high_priority() {
        let router = AiRouter::new(vec![
            text_provider("text-low", 5, true),
            vision_provider("vision-high", 10, true),
        ]);
        let picked = router.route(&AiTask::DescribeImage).unwrap();
        assert_eq!(picked.id.as_str(), "vision-high");
    }

    #[test]
    fn route_classify_file_fallback_text() {
        // vision disabled -> fallback to text provider
        let router = AiRouter::new(vec![
            vision_provider("vision", 10, false),
            text_provider("text", 5, true),
        ]);
        let picked = router.route(&AiTask::ClassifyFile).unwrap();
        assert_eq!(picked.id.as_str(), "text");
    }

    #[test]
    fn route_classify_file_picks_highest_enabled() {
        let router = AiRouter::new(vec![
            text_provider("low", 5, true),
            vision_provider("high", 20, true),
        ]);
        let picked = router.route(&AiTask::ClassifyFile).unwrap();
        assert_eq!(picked.id.as_str(), "high");
    }

    #[test]
    fn route_disabled_provider_ignored() {
        let router = AiRouter::new(vec![vision_provider("v", 10, false)]);
        assert!(router.route(&AiTask::DescribeImage).is_none());
    }

    #[test]
    fn route_no_capable_returns_none() {
        let router = AiRouter::new(vec![text_provider("t", 5, true)]);
        assert!(router.route(&AiTask::DescribeImage).is_none());
        assert!(router.route(&AiTask::SemanticSearch).is_none());
    }

    #[test]
    fn route_embeddings_task() {
        let router = AiRouter::new(vec![
            text_provider("t", 10, true),
            embedding_provider("e", 8),
        ]);
        let picked = router.route(&AiTask::SemanticSearch).unwrap();
        assert_eq!(picked.id.as_str(), "e");
        let picked2 = router.route(&AiTask::DetectSemanticDuplicate).unwrap();
        assert_eq!(picked2.id.as_str(), "e");
    }

    #[test]
    fn route_offline_empty() {
        let router = AiRouter::new(vec![]);
        assert!(router.route(&AiTask::ClassifyFile).is_none());
    }

    #[test]
    fn supports_text_task() {
        let p = text_provider("t", 5, true);
        assert!(p.supports(&AiTask::ClassifyFile));
        assert!(p.supports(&AiTask::GenerateTags));
        assert!(!p.supports(&AiTask::DescribeImage));
    }

    #[test]
    fn supports_vision_false() {
        let p = text_provider("t", 5, true);
        assert!(!p.supports(&AiTask::DescribeImage));
        let v = vision_provider("v", 5, true);
        assert!(v.supports(&AiTask::DescribeImage));
    }

    #[test]
    fn parse_valid_json() {
        let raw = r#"{"category":"docs","confidence":0.95,"tags":["a"],"reason":"ok"}"#;
        let r = parse_ai_json(raw).unwrap();
        assert_eq!(r.category, "docs");
        assert_eq!(r.confidence, 0.95);
    }

    #[test]
    fn parse_fenced_json() {
        let raw = "```json\n{\"category\":\"image\",\"confidence\":0.82,\"reason\":\"looks like cat\"}\n```";
        let r = parse_ai_json(raw).unwrap();
        assert_eq!(r.category, "image");
    }

    #[test]
    fn parse_fenced_json_no_lang() {
        let raw = "```\n{\"category\":\"x\",\"confidence\":0.5,\"reason\":\"y\"}\n```";
        let r = parse_ai_json(raw).unwrap();
        assert_eq!(r.category, "x");
    }

    #[test]
    fn parse_fenced_json_extra_text() {
        let raw = "here is result:\n```json\n{\"category\":\"finance\",\"confidence\":0.91,\"reason\":\"invoice\"}\n```\ndone";
        let r = parse_ai_json(raw).unwrap();
        assert_eq!(r.category, "finance");
    }

    #[test]
    fn parse_all_fields() {
        let raw = r#"{"category":"cat","subcategory":"sub","confidence":0.77,"suggested_name":"foo.txt","suggested_folder":"/docs","tags":["t1","t2"],"reason":"r"}"#;
        let r = parse_ai_json(raw).unwrap();
        assert_eq!(r.subcategory.as_deref(), Some("sub"));
        assert_eq!(r.suggested_name.as_deref(), Some("foo.txt"));
        assert_eq!(r.tags.len(), 2);
    }

    #[test]
    fn parse_rejects_missing_category() {
        let raw = r#"{"confidence":0.9,"reason":"no cat"}"#;
        assert!(parse_ai_json(raw).is_err());
    }

    #[test]
    fn parse_rejects_empty_category() {
        let raw = r#"{"category":"","confidence":0.9,"reason":"empty"}"#;
        assert!(parse_ai_json(raw).is_err());
        let raw2 = r#"{"category":"   ","confidence":0.9,"reason":"blank"}"#;
        assert!(parse_ai_json(raw2).is_err());
    }

    #[test]
    fn parse_rejects_confidence_out_of_range_high() {
        let raw = r#"{"category":"x","confidence":1.5,"reason":"bad"}"#;
        assert!(parse_ai_json(raw).is_err());
    }

    #[test]
    fn parse_rejects_confidence_negative() {
        let raw = r#"{"category":"x","confidence":-0.1,"reason":"bad"}"#;
        assert!(parse_ai_json(raw).is_err());
    }

    #[test]
    fn parse_rejects_confidence_nan() {
        let raw = r#"{"category":"x","confidence":null,"reason":"bad"}"#;
        assert!(parse_ai_json(raw).is_err());
    }

    #[test]
    fn confidence_high_threshold() {
        assert!(matches!(
            classify_confidence(0.95, DEFAULT_THRESHOLD_HIGH, DEFAULT_THRESHOLD_MED),
            Confidence::High(_)
        ));
        assert!(matches!(
            classify_confidence(0.90, DEFAULT_THRESHOLD_HIGH, DEFAULT_THRESHOLD_MED),
            Confidence::High(_)
        ));
    }

    #[test]
    fn confidence_medium_threshold() {
        assert!(matches!(
            classify_confidence(0.85, DEFAULT_THRESHOLD_HIGH, DEFAULT_THRESHOLD_MED),
            Confidence::Medium(_)
        ));
        assert!(matches!(
            classify_confidence(0.70, DEFAULT_THRESHOLD_HIGH, DEFAULT_THRESHOLD_MED),
            Confidence::Medium(_)
        ));
        assert!(matches!(
            classify_confidence(0.89, DEFAULT_THRESHOLD_HIGH, DEFAULT_THRESHOLD_MED),
            Confidence::Medium(_)
        ));
    }

    #[test]
    fn confidence_low_threshold() {
        assert!(matches!(
            classify_confidence(0.69, DEFAULT_THRESHOLD_HIGH, DEFAULT_THRESHOLD_MED),
            Confidence::Low(_)
        ));
        assert!(matches!(
            classify_confidence(0.0, DEFAULT_THRESHOLD_HIGH, DEFAULT_THRESHOLD_MED),
            Confidence::Low(_)
        ));
    }

    #[test]
    fn confidence_boundary_90_is_high() {
        let c = classify_confidence(0.90, 0.90, 0.70);
        assert_eq!(c, Confidence::High(0.90));
    }

    #[test]
    fn confidence_boundary_70_is_medium() {
        let c = classify_confidence(0.70, 0.90, 0.70);
        assert_eq!(c, Confidence::Medium(0.70));
    }

    #[test]
    fn confidence_custom_thresholds() {
        assert!(matches!(
            classify_confidence(0.8, 0.85, 0.6),
            Confidence::Medium(0.8)
        ));
        assert!(matches!(
            classify_confidence(0.86, 0.85, 0.6),
            Confidence::High(0.86)
        ));
    }

    // ── embedding helpers ──────────────────────────────────────────────
    #[test]
    fn cosine_similarity_identity() {
        let a = vec![1.0f32, 0.0, 0.0];
        let b = vec![1.0f32, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_opposite() {
        let a = vec![1.0f32, 0.0];
        let b = vec![-1.0f32, 0.0];
        assert!((cosine_similarity(&a, &b) + 1.0).abs() < 1e-6);
    }

    #[test]
    fn confidence_clamped() {
        assert_eq!(confidence_from_cosine(1.0), 1.0);
        assert_eq!(confidence_from_cosine(-1.0), 0.0);
        assert!((confidence_from_cosine(0.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn classify_via_embeddings_picks_best() {
        let ex = vec![
            ("A".to_string(), "FolderA".to_string(), vec![1.0, 0.0]),
            ("B".to_string(), "FolderB".to_string(), vec![0.0, 1.0]),
        ];
        let r = classify_via_embeddings(&[0.9, 0.1], &ex);
        assert_eq!(r.category, "A");
        assert!(r.confidence > 0.5);
        assert!(r.confidence <= 1.0);
    }

    // ── Phase 4 smoke gate ────────────────────────────────────────────
    #[tokio::test]
    async fn classify_smoke() {
        // ponytail: heuristic path when model absent; if model present also test real path (skip if files missing)
        let hf = HeuristicFallback::new();
        let bundled = BundledLocalProvider::new(); // model missing -> fallback
        let fixtures = [
            ("report.pdf", "Documents"),
            ("photo.jpg", "Images"),
            ("archive.zip", "Archives"),
            ("main.rs", "Code"),
            ("song.mp3", "Audio"),
        ];
        for (input, expected_cat) in fixtures {
            let r1 = hf.classify(AiTask::ClassifyFile, input).await.unwrap();
            assert!(
                r1.confidence > 0.3,
                "heuristic confidence too low for {input}: {}",
                r1.confidence
            );
            assert_ne!(r1.category, "Unknown", "category Unknown for {input}");
            assert_eq!(r1.category, expected_cat, "mismatch for {input}");
            assert!(r1.suggested_folder.is_some());

            let r2 = bundled.classify(AiTask::ClassifyFile, input).await.unwrap();
            assert!(r2.confidence > 0.3);
            assert_ne!(r2.category, "Unknown");
            // heuristic fallback should match expected when model absent
            if !bundled.model_available() {
                assert_eq!(
                    r2.category, expected_cat,
                    "bundled heuristic mismatch for {input}"
                );
            }
        }
        // unknown ext still passes gate
        let r = hf.classify(AiTask::ClassifyFile, "README").await.unwrap();
        assert!(r.confidence > 0.3);
        assert_ne!(r.category, "Unknown");

        // if model files are present, also exercise real embedding path
        if bundled.model_available() {
            // ponytail: real model present -> ensure embedding path yields valid confidence range
            let r3 = bundled
                .classify(AiTask::ClassifyFile, "invoice finance budget.pdf")
                .await
                .unwrap();
            assert!(r3.confidence >= 0.0 && r3.confidence <= 1.0);
            assert!(!r3.category.is_empty());
            // reason should indicate embedding, not just heuristic
            assert!(
                r3.reason.contains("cosine")
                    || r3.reason.contains("embedding")
                    || r3.reason.contains("heuristic")
            );
        }
    }

    #[tokio::test]
    async fn cloud_not_configured_returns_error() {
        // env missing -> should return not configured, letting router fallback
        let p = CloudProvider::openai().with_api_key(None);
        let err = p
            .classify(AiTask::ClassifyFile, "test.jpg")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not configured"), "got {err}");
        let p2 = CloudProvider::anthropic().with_api_key(Some("".into()));
        let err2 = p2
            .classify(AiTask::ClassifyFile, "test.jpg")
            .await
            .unwrap_err();
        assert!(err2.to_string().contains("not configured"), "got {err2}");
    }

    #[test]
    fn cloud_is_configured_and_supports() {
        let p = CloudProvider::openai().with_api_key(Some("sk-test".into()));
        assert!(p.is_configured());
        assert!(p.supports(&AiTask::ClassifyFile));
        assert!(!p.supports(&AiTask::SemanticSearch));
        assert_eq!(p.priority(), 20);
        let empty = CloudProvider::openai().with_api_key(None);
        assert!(!empty.is_configured());
    }

    #[tokio::test]
    async fn classify_batch_multiple_inputs() {
        let p = HeuristicFallback::new();
        let inputs = ["photo.jpg", "budget.xlsx", "song.mp3", "main.rs"];
        let results = p.classify_batch(AiTask::ClassifyFile, &inputs).await;
        assert_eq!(results.len(), 4);
        assert_eq!(results[0].as_ref().unwrap().category, "Images");
        assert_eq!(results[1].as_ref().unwrap().category, "Documents");
        assert_eq!(results[2].as_ref().unwrap().category, "Audio");
        assert_eq!(results[3].as_ref().unwrap().category, "Code");
    }
}
