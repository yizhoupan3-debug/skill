// Runtime attach descriptor methods.
// Extracted from frag_impl_browser_runtime.rs to keep file size ≤2000 lines.

impl BrowserRuntime {
    fn configured_runtime_attach_source(&self) -> ConfiguredAttachSource {
        if let Some(path) = self
            .attach_config
            .runtime_attach_descriptor_path
            .as_ref()
            .filter(|path| !path.trim().is_empty())
        {
            return ConfiguredAttachSource {
                source: Some("descriptor_path"),
                path: Some(path.clone()),
            };
        }
        if let Some(path) = self
            .attach_config
            .runtime_attach_artifact_path
            .as_ref()
            .filter(|path| !path.trim().is_empty())
        {
            return ConfiguredAttachSource {
                source: Some("attach_artifact_path"),
                path: Some(path.clone()),
            };
        }
        if let Some(path) = self.auto_discover_runtime_attach_artifact() {
            return ConfiguredAttachSource {
                source: Some("attach_artifact_path"),
                path: Some(path),
            };
        }
        ConfiguredAttachSource {
            source: None,
            path: None,
        }
    }

    fn resolve_attached_runtime_descriptor_context(
        &self,
    ) -> Result<ResolvedAttachedRuntimeDescriptorContext, Value> {
        let configured_source = self.configured_runtime_attach_source();
        if configured_source.source.is_none() {
            return Err(browser_error(
                "ATTACHED_RUNTIME_NOT_CONFIGURED",
                "No runtime attach descriptor is configured for browser-mcp.",
                &[
                    "start browser-mcp with --runtime-attach-descriptor-path",
                    "or --runtime-attach-artifact-path",
                    "or set BROWSER_MCP_RUNTIME_ATTACH_DESCRIPTOR_PATH",
                ],
                true,
            ));
        }

        let loaded = self.load_runtime_attach_descriptor().map_err(|err| {
            browser_error(
                "ATTACHED_RUNTIME_INVALID_DESCRIPTOR",
                &err,
                &[
                    "refresh the descriptor from describe_runtime_event_handoff",
                    "inspect browser_diagnostics",
                ],
                true,
            )
        })?;
        let descriptor = loaded.descriptor;
        let replay_supported =
            descriptor_bool(&descriptor, &["attach_capabilities", "artifact_replay"]) == Some(true);
        let trace_stream_path = descriptor_resolved_artifact(&descriptor, "trace_stream_path");
        let diagnostics_base = self.project_attached_runtime_diagnostics(
            &configured_source,
            &descriptor,
            loaded.input_artifact_kind,
            trace_stream_path.clone(),
        );

        if descriptor_string(&descriptor, &["schema_version"]).as_deref()
            != Some(RUNTIME_ATTACH_DESCRIPTOR_SCHEMA_VERSION)
            || descriptor_string(&descriptor, &["attach_mode"]).as_deref()
                != Some(RUNTIME_ATTACH_MODE)
            || !replay_supported
        {
            return Err(browser_error(
                "ATTACHED_RUNTIME_INVALID_DESCRIPTOR",
                "runtime attach descriptor must be artifact-replay capable and match the Rust-first schema.",
                &[
                    "refresh the descriptor from describe_runtime_event_handoff",
                    "inspect browser_diagnostics",
                ],
                true,
            ));
        }

        let backend_family = descriptor_string(&descriptor, &["artifact_backend_family"])
            .unwrap_or_else(|| "filesystem".to_string());
        if backend_family != "filesystem" && backend_family != "sqlite" {
            return Err(browser_error(
                "ATTACHED_RUNTIME_UNSUPPORTED_BACKEND",
                &format!(
                    "browser-mcp attach consumer currently supports filesystem/sqlite replay only (got {backend_family})"
                ),
                &[
                    "use a filesystem- or sqlite-backed attach descriptor for browser-mcp replay",
                    "inspect browser_diagnostics",
                ],
                true,
            ));
        }

        let Some(trace_stream_path) = trace_stream_path else {
            return Err(browser_error(
                "ATTACHED_RUNTIME_TRACE_UNAVAILABLE",
                "runtime attach descriptor must carry a canonical resolved_artifacts.trace_stream_path.",
                &["refresh the descriptor from describe_runtime_event_handoff"],
                true,
            ));
        };

        Ok(ResolvedAttachedRuntimeDescriptorContext {
            trace_stream_path,
            diagnostics_base,
        })
    }

    fn load_runtime_attach_descriptor(&self) -> Result<LoadedRuntimeAttachDescriptor, String> {
        let configured_source = self.configured_runtime_attach_source();
        match configured_source.source {
            Some("descriptor_path") => {
                self.read_runtime_attach_descriptor_file(configured_source.path.as_deref())
            }
            Some("attach_artifact_path") => self
                .build_runtime_attach_descriptor_from_artifact_path(
                    configured_source.path.as_deref(),
                ),
            _ => Err("runtime attach descriptor is not configured".to_string()),
        }
    }

    fn read_runtime_attach_descriptor_file(
        &self,
        descriptor_path: Option<&str>,
    ) -> Result<LoadedRuntimeAttachDescriptor, String> {
        let descriptor_path = descriptor_path
            .ok_or_else(|| "runtime attach descriptor path is missing".to_string())?;
        let raw = fs::read_to_string(descriptor_path)
            .map_err(|err| format!("read runtime attach descriptor failed: {err}"))?;
        let parsed = serde_json::from_str::<Value>(&raw)
            .map_err(|err| format!("parse runtime attach descriptor failed: {err}"))?;
        if !parsed.is_object() {
            return Err("runtime attach descriptor must decode to a JSON object".to_string());
        }
        self.canonicalize_attach_descriptor_if_possible(parsed)
    }

    fn build_runtime_attach_descriptor_from_artifact_path(
        &self,
        artifact_path: Option<&str>,
    ) -> Result<LoadedRuntimeAttachDescriptor, String> {
        let artifact_path =
            artifact_path.ok_or_else(|| "runtime attach artifact path is missing".to_string())?;
        let resolved_path = normalize_runtime_locator_for_existing_file(artifact_path);
        if let Ok(raw) = fs::read_to_string(&resolved_path) {
            let parsed = serde_json::from_str::<Value>(&raw)
                .map_err(|err| format!("parse runtime attach artifact failed: {err}"))?;
            if !parsed.is_object() {
                return Err("runtime attach artifact returned an unknown schema".to_string());
            }
            let schema = descriptor_string(&parsed, &["schema_version"]);
            if matches!(
                schema.as_deref(),
                Some(RUNTIME_EVENT_TRANSPORT_SCHEMA_VERSION)
                    | Some(RUNTIME_EVENT_HANDOFF_SCHEMA_VERSION)
                    | Some(TRACE_RESUME_MANIFEST_SCHEMA_VERSION)
            ) {
                if let Ok(loaded) =
                    self.try_hydrate_runtime_attach_descriptor_from_artifact_path(&resolved_path)
                {
                    return Ok(loaded);
                }
            }
            if schema.as_deref() == Some(RUNTIME_ATTACH_DESCRIPTOR_SCHEMA_VERSION) {
                return self.canonicalize_attach_descriptor_if_possible(parsed);
            }
            if let Ok(loaded) =
                self.try_hydrate_runtime_attach_descriptor_from_artifact_path(&resolved_path)
            {
                return Ok(loaded);
            }
            return Err("runtime attach artifact returned an unknown schema".to_string());
        }
        self.try_hydrate_runtime_attach_descriptor_from_artifact_path(artifact_path)
    }

    fn try_hydrate_runtime_attach_descriptor_from_artifact_path(
        &self,
        artifact_path: &str,
    ) -> Result<LoadedRuntimeAttachDescriptor, String> {
        self.hydrate_runtime_attach_descriptor_via_rust(None, Some(artifact_path), None, None)
            .or_else(|_| {
                self.hydrate_runtime_attach_descriptor_via_rust(
                    None,
                    None,
                    Some(artifact_path),
                    None,
                )
            })
            .or_else(|_| {
                self.hydrate_runtime_attach_descriptor_via_rust(
                    None,
                    None,
                    None,
                    Some(artifact_path),
                )
            })
    }

    fn canonicalize_attach_descriptor_if_possible(
        &self,
        descriptor: Value,
    ) -> Result<LoadedRuntimeAttachDescriptor, String> {
        match self.hydrate_runtime_attach_descriptor_via_rust(
            Some(descriptor.clone()),
            None,
            None,
            None,
        ) {
            Ok(hydrated) => {
                assert_attach_descriptor_matches_canonical(&descriptor, &hydrated.descriptor)?;
                assert_attach_descriptor_contract(&hydrated.descriptor)?;
                Ok(hydrated)
            }
            Err(err) => {
                if attach_descriptor_needs_rust_hydration(&descriptor) {
                    return Err(err);
                }
                assert_attach_descriptor_contract(&descriptor)?;
                Ok(LoadedRuntimeAttachDescriptor {
                    descriptor,
                    input_artifact_kind: Some("attach_descriptor"),
                })
            }
        }
    }

    fn hydrate_runtime_attach_descriptor_via_rust(
        &self,
        attach_descriptor: Option<Value>,
        binding_artifact_path: Option<&str>,
        handoff_path: Option<&str>,
        resume_manifest_path: Option<&str>,
    ) -> Result<LoadedRuntimeAttachDescriptor, String> {
        let attached = attach_runtime_event_transport(json!({
            "attach_descriptor": attach_descriptor,
            "binding_artifact_path": binding_artifact_path,
            "handoff_path": handoff_path,
            "resume_manifest_path": resume_manifest_path,
        }))?;
        let descriptor = attached
            .get("attach_descriptor")
            .cloned()
            .filter(Value::is_object)
            .ok_or_else(|| {
                "runtime attach transport payload is missing attach_descriptor".to_string()
            })?;
        let input_artifact_kind = if attach_descriptor.is_some() {
            Some("attach_descriptor")
        } else if binding_artifact_path.is_some() {
            Some("binding_artifact")
        } else if handoff_path.is_some() {
            Some("handoff")
        } else if resume_manifest_path.is_some() {
            Some("resume_manifest")
        } else {
            None
        };
        Ok(LoadedRuntimeAttachDescriptor {
            descriptor,
            input_artifact_kind,
        })
    }

    fn project_attached_runtime_diagnostics(
        &self,
        configured_source: &ConfiguredAttachSource,
        descriptor: &Value,
        input_artifact_kind: Option<&str>,
        trace_stream_path: Option<String>,
    ) -> Value {
        json!({
            "status": "ready",
            "descriptorSource": configured_source.source,
            "descriptorPath": configured_source.path,
            "inputArtifactKind": input_artifact_kind,
            "schemaVersion": descriptor_string(descriptor, &["schema_version"]),
            "attachMode": descriptor_string(descriptor, &["attach_mode"]),
            "artifactBackendFamily": descriptor_string(descriptor, &["artifact_backend_family"]),
            "recommendedEntrypoint": descriptor_string(descriptor, &["recommended_entrypoint"]),
            "sourceTransportMethod": descriptor_string(descriptor, &["source_transport_method"]),
            "sourceHandoffMethod": descriptor_string(descriptor, &["source_handoff_method"]),
            "traceStreamPath": trace_stream_path,
            "bindingArtifactSource": descriptor_string(descriptor, &["resolution", "binding_artifact_path"]),
            "handoffSource": descriptor_string(descriptor, &["resolution", "handoff_path"]),
            "resumeManifestSource": descriptor_string(descriptor, &["resolution", "resume_manifest_path"]),
            "traceStreamSource": descriptor_string(descriptor, &["resolution", "trace_stream_path"]),
            "replaySupported": descriptor_bool(descriptor, &["attach_capabilities", "artifact_replay"]).unwrap_or(false),
            "eventCount": 0,
            "latestEventId": null,
            "latestEventKind": null,
            "latestEventTimestamp": null,
            "warning": null,
        })
    }

    fn auto_discover_runtime_attach_artifact(&self) -> Option<String> {
        resolve_browser_mcp_attach_artifact(&self.repo_root, None)
    }

}
