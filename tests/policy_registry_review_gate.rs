mod common;

use common::{project_root, read_json, read_text};
use serde_json::Value;
use std::collections::{BTreeSet, HashSet};

include!("policy/registry_review_gate.rs");
