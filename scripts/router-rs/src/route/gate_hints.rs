use super::constants::ARTIFACT_GATE_PHRASES;

pub(crate) fn gate_hint_phrases(gate: &str) -> Vec<String> {
    match gate {
        "source" => vec![
            "官方".to_string(),
            "官方文档".to_string(),
            "文档".to_string(),
            "docs".to_string(),
            "readme".to_string(),
            "api".to_string(),
            "openai".to_string(),
            "github".to_string(),
            "look up".to_string(),
            "search".to_string(),
        ],
        "artifact" => ARTIFACT_GATE_PHRASES
            .iter()
            .map(|phrase| (*phrase).to_string())
            .collect(),
        "evidence" => vec![
            "报错".to_string(),
            "失败".to_string(),
            "崩".to_string(),
            "截图".to_string(),
            "渲染".to_string(),
            "日志".to_string(),
            "traceback".to_string(),
            "error".to_string(),
            "bug".to_string(),
            "why".to_string(),
            "为什么".to_string(),
        ],
        "delegation" => vec![
            "sidecar".to_string(),
            "subagent".to_string(),
            "delegation".to_string(),
            "并行 sidecar".to_string(),
            "multiagent".to_string(),
            "multi-agent".to_string(),
            "多 agent".to_string(),
            "子代理".to_string(),
            "主线程".to_string(),
            "local-supervisor".to_string(),
            "跨文件".to_string(),
            "长运行".to_string(),
        ],
        _ => Vec::new(),
    }
}
