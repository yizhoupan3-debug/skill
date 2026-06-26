use core_errors::FrameworkError;
use std::collections::HashSet;

use super::types::{FrameworkProfileContract, HOST_SPECIFIC_METADATA_KEYS, REQUIRED_CORE_CAPABILITIES};

pub fn validate_framework_profile(profile: &FrameworkProfileContract) -> Result<(), FrameworkError> {
    if profile.profile_id.trim().is_empty() {
        return Err(FrameworkError::validation("framework profile missing profile_id"));
    }
    if profile.display_name.trim().is_empty() {
        return Err(FrameworkError::validation("framework profile missing display_name"));
    }
    if profile.framework_profile_version.trim().is_empty() {
        return Err(FrameworkError::validation("framework profile missing framework_profile_version"));
    }
    if profile.host_family.trim() != "shared-rust-core" {
        return Err(FrameworkError::validation("framework core must be pinned to shared-rust-core"));
    }

    let capability_set = profile
        .core_capabilities
        .iter()
        .map(|value| value.as_str())
        .collect::<HashSet<_>>();
    let missing = REQUIRED_CORE_CAPABILITIES
        .iter()
        .filter(|cap| !capability_set.contains(**cap))
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(FrameworkError::validation(format!(
            "framework profile missing core capabilities: {}",
            missing.join(", ")
        )));
    }
    let host_specific_metadata = profile
        .metadata
        .keys()
        .filter(|key| HOST_SPECIFIC_METADATA_KEYS.contains(&key.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !host_specific_metadata.is_empty() {
        return Err(FrameworkError::validation(format!(
            "framework profile metadata must stay shared-core-only; move host-private keys into explicit host payloads: {}",
            host_specific_metadata.join(", ")
        )));
    }
    Ok(())
}
