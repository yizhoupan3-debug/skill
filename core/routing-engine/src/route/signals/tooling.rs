use super::has_signal_by_name;

pub fn has_codegraph_index_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("codegraph_index_ready", query_text, query_token_list)
}
