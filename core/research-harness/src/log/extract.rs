// Migrated from tools/research-log-rs/src/extract.rs

//! Entity extraction from research text using regex pattern matching.
//!
//! No external NLP — pure `regex` patterns tuned for ML/AI/Quant research
//! vocabulary. Extractable entity kinds:
//!
//! - `method`: ML models, statistical methods, optimization techniques
//! - `dataset`: benchmark datasets, evaluation corpora
//! - `metric`: evaluation metrics, performance measures
//! - `model`: named ML models, factor models
//! - `tool`: libraries, frameworks, platforms

use regex::Regex;
use std::sync::LazyLock;

/// Entity extraction pattern groups.
pub struct ExtractionPatterns {
    pub method: Regex,
    pub dataset: Regex,
    pub metric: Regex,
    pub model: Regex,
    pub tool: Regex,
}

pub static DEFAULT_PATTERNS: LazyLock<ExtractionPatterns> = LazyLock::new(|| {
    ExtractionPatterns {
        method: Regex::new(
            r"(?i)\b(CNN|LSTM|GRU|Transformer|EWMA|PCA|GARCH|ARIMA|VARIMA|VAE|GAN|DQN|PPO|SVM|Random\s?Forest|XGBoost|CatBoost|LightGBM|K-means|DBSCAN|t-SNE|UMAP|REINFORCE|MCMC|HMM|CRF|Attention|Self-Attention|Cross-Attention|Mixture\s+of\s+Experts|LoRA|QLoRA|Batch\s+Norm|Layer\s+Norm|Dropout|Residual\s+Connection|Gradient\s+Clipping|Learning\s+Rate\s+Schedule|Fama-French|Carhart|Barra|Momentum|Mean-Reversion|Pair\s+Trading|Statistical\s+Arbitrage|Risk\s+Parity|Markowitz|Black-Litterman|Kalman\s+Filter|Particle\s+Filter|Monte\s+Carlo|MC dropout|Ensemble|Bagging|Boosting|Stacking|ResNet|Inception|MobileNet|EfficientNet|YOLO|UNet|RoBERTa|BERT|T5|GPT|Diffusion|Normalizing\s+Flow|Neural\s+ODE|PINN|GCN|GAT|Graph\s+Neural|Reinforcement\s+Learning|Deep\s+RL|Active\s+Learning|Transfer\s+Learning|Few-shot|Zero-shot|Contrastive\s+Learning|Self-supervised|Semi-supervised|Meta-Learning|Federated\s+Learning|Online\s+Learning|Batch\s+Learning|Bayesian\s+Optimization|Hyperparameter\s+Tuning|Grid\s+Search|Random\s+Search|Bayesian\s+Inference|Variational\s+Inference|Expectation-Maximization|Gibbs\s+Sampling|Hamiltonian\s+MC|NUTS|ADVI|SVI)\b"
        ).unwrap(),
        dataset: Regex::new(
            r"(?i)\b(SQuAD|ImageNet|MNIST|CIFAR-10|CIFAR-100|COCO|GLUE|SuperGLUE|MMLU|HumanEval|MBPP|CSV|Yahoo\s+Finance|CRSP|Compustat|CSI\s+300|S&P\s+500|Barra|WorldQuant|Alpha\s+Factors|TAQ|WRDS|OptionMetrics|IBES|Bloomberg|Reuters|Quandl|FRED|OECD|World\s+Bank|IMF|BIS)\b"
        ).unwrap(),
        metric: Regex::new(
            r"(?i)\b(accuracy|precision|recall|F1|F1-score|AUC|ROC-AUC|BLEU|ROUGE|perplexity|Sharpe\s*ratio|Info\s*ratio|Max\s*DD|Calmar\s*ratio|Turnover|Hit\s*Rate|IC|Rank\s*IC|Spearman|Pearson|MAE|MSE|RMSE|MAPE|SMAPE|R-squared|Adjusted\s+R2|log-likelihood|AIC|BIC|DIC|WAIC|ELBO|KL\s+divergence|JS\s+divergence|Earth\s+Mover|Wasserstein|Total\s+Variation|Mutual\s+Information|NMI|AMI|Silhouette|Calinski-Harabasz|Davies-Bouldin|Gini|Sortino|Capture\s+ratio|Up-capture|Down-capture|Tracking\s+error|Bias|Residual|Standard\s+Error|p-value|t-statistic)\b"
        ).unwrap(),
        model: Regex::new(
            r"(?i)\b(GPT-?[234]|BERT|BERT-large|BERT-base|RoBERTa|ALBERT|DistilBERT|T5|T5-small|T5-base|T5-large|Llama\s*2|Llama\s*3|Llama|Mistral|Mixtral|Gemma|Claude|ResNet-50|ResNet-101|ResNet-152|VGG-16|VGG-19|Inception-v3|Inception-v4|MobileNet-v1|MobileNet-v2|EfficientNet-B0|EfficientNet-B7|YOLOv[3-9]|UNet|Transformer|Factor\s+Model|Multi-factor|CAPM|APT|Fama-French\s+3-factor|Fama-French\s+5-factor|Carhart\s+4-factor|Barra\s+model|AQR|Two-sigma|Renaissance|DE\s+Shaw|Citadel|Momentum\s+factor|Value\s+factor|Size\s+factor|Quality\s+factor|Low\s+vol|Beta\s+factor|Growth\s+factor)\b"
        ).unwrap(),
        tool: Regex::new(
            r"(?i)\b(PyTorch|TensorFlow|JAX|Flax|Haiku|NumPy|pandas|scikit-learn|scipy|Dask|Ray|Spark|Kubernetes|Docker|AWS|GCP|Azure|Weights\s*&\s*Biases|MLflow|wandb|Hugging\s+Face|Transformers|Datasets|Accelerate|DeepSpeed|Megatron|Horovod|ONNX|TensorRT|Triton|Jupyter|Colab|Slurm|MPI|CUDA|cuDNN|OpenMP|TBB|XLA|TVM)\b"
        ).unwrap(),
    }
});

/// Extract entities from text, returning `Vec<(entity_name, entity_kind)>`.
/// Deduplicates by name (first occurrence wins).
pub fn extract_entities_from_text(text: &str) -> Vec<(String, String)> {
    let patterns = &*DEFAULT_PATTERNS;
    let mut seen = std::collections::HashSet::new();
    let mut results = Vec::new();

    // Process each pattern group
    let extractors: [(Regex, &str); 5] = [
        (patterns.method.clone(), "method"),
        (patterns.dataset.clone(), "dataset"),
        (patterns.metric.clone(), "metric"),
        (patterns.model.clone(), "model"),
        (patterns.tool.clone(), "tool"),
    ];

    for (re, kind) in &extractors {
        for cap in re.find_iter(text) {
            let name = cap.as_str().to_string();
            if seen.insert(name.clone()) {
                results.push((name, kind.to_string()));
            }
        }
    }

    results
}

/// 从文本中提取实体及其类型，返回 `(entity, type)` 对列表。
/// 兼容骨架 API 的便捷封装。
pub fn extract_entities(text: &str) -> anyhow::Result<Vec<(String, String)>> {
    Ok(extract_entities_from_text(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_method_entities() {
        let entities = extract_entities_from_text("We use Transformer and BERT for this task");
        let methods: Vec<_> = entities.iter().filter(|(_, k)| k == "method").map(|(n, _)| n.as_str()).collect();
        assert!(methods.contains(&"Transformer"), "should find Transformer as method");
    }

    #[test]
    fn extract_model_entities() {
        let entities = extract_entities_from_text("Fine-tuned GPT-3 on SQuAD");
        let models: Vec<_> = entities.iter().filter(|(_, k)| k == "model").map(|(n, _)| n.as_str()).collect();
        assert!(!models.is_empty(), "should find at least one model entity");
    }

    #[test]
    fn extract_dataset_entities() {
        let entities = extract_entities_from_text("Evaluated on ImageNet and CIFAR-10");
        let datasets: Vec<_> = entities.iter().filter(|(_, k)| k == "dataset").map(|(n, _)| n.as_str()).collect();
        assert!(datasets.contains(&"ImageNet"));
        assert!(datasets.contains(&"CIFAR-10"));
    }

    #[test]
    fn extract_metric_entities() {
        let entities = extract_entities_from_text("Measured F1-score and accuracy");
        let metrics: Vec<_> = entities.iter().filter(|(_, k)| k == "metric").map(|(n, _)| n.as_str()).collect();
        assert!(!metrics.is_empty());
    }

    #[test]
    fn extract_tool_entities() {
        let entities = extract_entities_from_text("Implemented in PyTorch with CUDA");
        let tools: Vec<_> = entities.iter().filter(|(_, k)| k == "tool").map(|(n, _)| n.as_str()).collect();
        assert!(tools.contains(&"PyTorch"));
    }

    #[test]
    fn extract_deduplicates() {
        let entities = extract_entities_from_text("BERT is great. BERT outperforms BERT.");
        let bert_count = entities.iter().filter(|(n, _)| n == "BERT").count();
        assert_eq!(bert_count, 1, "should deduplicate BERT");
    }

    #[test]
    fn extract_empty_text() {
        let entities = extract_entities_from_text("");
        assert!(entities.is_empty());
    }

    #[test]
    fn extract_no_entities() {
        let entities = extract_entities_from_text("The quick brown fox jumps over the lazy dog.");
        assert!(entities.is_empty());
    }
}
