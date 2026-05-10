//! clap 类型与 execute / trace JSON 载荷（serde）。
use crate::router_self;
use clap::{ArgAction, Args, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

include!("args.inc");
