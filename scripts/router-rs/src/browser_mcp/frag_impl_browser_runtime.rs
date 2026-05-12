// `impl BrowserRuntime`（与 `frag_01_through_types.rs` 中类型同模块拼接）。
impl BrowserRuntime {
    #[cfg(test)]
    fn new(repo_root: PathBuf) -> Self {
        Self::with_attach_config(repo_root, BrowserAttachConfig::default())
    }

    fn with_attach_config(repo_root: PathBuf, attach_config: BrowserAttachConfig) -> Self {
        Self {
            repo_root,
            attach_config,
            sessions: HashMap::new(),
            browser_processes: HashMap::new(),
            session_counter: 0,
            tab_counter: 0,
            ref_counter: 0,
            request_counter: 0,
            screenshot_counter: 0,
        }
    }

    fn skill_route(&self, input: &Value) -> Result<Value, Value> {
        let query = required_string_arg(input, "query")?;
        let session_id =
            optional_string(input, "sessionId").unwrap_or_else(|| "cowork-mcp".to_string());
        let allow_overlay = optional_bool(input, "allowOverlay").unwrap_or(true);
        let first_turn = optional_bool(input, "firstTurn").unwrap_or(true);
        let runtime_path = skill_runtime_path(&self.repo_root);
        let manifest_path = skill_manifest_path(&self.repo_root);
        if !runtime_path.is_file() {
            return Err(skill_error(
                "SKILL_RUNTIME_MISSING",
                &format!(
                    "Missing repository skill runtime: {}",
                    runtime_path.display()
                ),
            ));
        }
        let records = load_records(Some(&runtime_path), Some(&manifest_path))
            .map_err(|err| skill_error("SKILL_ROUTE_FAILED", &err))?;
        let decision = route_with_full_manifest_fallback(
            &records,
            &manifest_path,
            &query,
            &session_id,
            allow_overlay,
            first_turn,
        )
        .map_err(|err| skill_error("SKILL_ROUTE_FAILED", &err))?;
        let selected_path = if decision.selected_skill == "none" {
            None
        } else {
            Some(
                skill_body_path(&self.repo_root, &decision.selected_skill)
                    .map_err(|err| skill_error("SKILL_READ_BLOCKED", &err))?,
            )
        };
        let overlay_path = decision
            .overlay_skill
            .as_ref()
            .map(|slug| skill_body_path(&self.repo_root, slug))
            .transpose()
            .map_err(|err| skill_error("SKILL_READ_BLOCKED", &err))?;
        Ok(json!({
            "schema_version": "cowork-skill-route-v1",
            "authority": "router-rs-browser-mcp",
            "repo_root": self.repo_root.to_string_lossy(),
            "runtime_path": runtime_path.to_string_lossy(),
            "manifest_path": manifest_path.to_string_lossy(),
            "decision": decision,
            "selected_skill_path": selected_path
                .as_ref()
                .map(|path| path.to_string_lossy().to_string()),
            "overlay_skill_path": overlay_path.map(|path| path.to_string_lossy().to_string()),
            "next_step": if selected_path.is_some() {
                "Read selected_skill_path from the canonical skills/ source before doing task work."
            } else {
                "No skill body is required; proceed with the native runtime instructions already in context."
            },
        }))
    }

    fn skill_search(&self, input: &Value) -> Result<Value, Value> {
        let query = required_string_arg(input, "query")?;
        let limit = optional_u64(input, "limit")?.unwrap_or(10).clamp(1, 50) as usize;
        let manifest_path = skill_manifest_path(&self.repo_root);
        if !manifest_path.is_file() {
            return Err(skill_error(
                "SKILL_MANIFEST_MISSING",
                &format!(
                    "Missing repository skill manifest: {}",
                    manifest_path.display()
                ),
            ));
        }
        let records = load_records_from_manifest(&manifest_path)
            .map_err(|err| skill_error("SKILL_SEARCH_FAILED", &err))?;
        let rows = search_skills(&records, &query, limit);
        let results = build_search_results_payload(&query, rows);
        serde_json::to_value(results)
            .map_err(|err| skill_error("SKILL_SEARCH_FAILED", &err.to_string()))
    }

    fn skill_read(&self, input: &Value) -> Result<Value, Value> {
        let slug = required_string_arg(input, "skill")?;
        let max_chars = optional_u64(input, "maxChars")?
            .unwrap_or(20_000)
            .clamp(1, 50_000) as usize;
        let path = skill_body_path(&self.repo_root, &slug)
            .map_err(|err| skill_error("SKILL_READ_BLOCKED", &err))?;
        let content = fs::read_to_string(&path).map_err(|err| {
            skill_error("SKILL_READ_FAILED", &format!("{}: {err}", path.display()))
        })?;
        let truncated = content.chars().count() > max_chars;
        Ok(json!({
            "schema_version": "cowork-skill-read-v1",
            "authority": "router-rs-browser-mcp",
            "skill": slug,
            "path": path.to_string_lossy(),
            "content": truncate_text(&content, max_chars),
            "truncated": truncated,
        }))
    }

    fn skill_route_status(&self) -> Result<Value, Value> {
        let runtime_path = skill_runtime_path(&self.repo_root);
        let manifest_path = skill_manifest_path(&self.repo_root);
        let mut remediation = Vec::new();
        if !runtime_path.is_file() {
            remediation.push(format!(
                "generate repository runtime artifacts so {} exists",
                runtime_path.to_string_lossy()
            ));
        }
        if !manifest_path.is_file() {
            remediation.push(format!(
                "generate repository runtime artifacts so {} exists",
                manifest_path.to_string_lossy()
            ));
        }
        remediation.push("call browser_diagnostics for attach/runtime details".to_string());
        Ok(json!({
            "schema_version": "cowork-skill-route-status-v1",
            "authority": "router-rs-browser-mcp",
            "repo_root": self.repo_root.to_string_lossy(),
            "skills_dir_exists": self.repo_root.join("skills").is_dir(),
            "runtime_path": runtime_path.to_string_lossy(),
            "runtime_exists": runtime_path.is_file(),
            "manifest_path": manifest_path.to_string_lossy(),
            "manifest_exists": manifest_path.is_file(),
            "routing_tools_exposed": skill_runtime_available(&self.repo_root),
            "remediation": remediation,
        }))
    }

    fn runtime_heartbeat(&mut self, input: &Value) -> Result<Value, Value> {
        let mut payload = json!({});
        if let Some(after_event_id) = optional_string(input, "afterEventId") {
            payload["afterEventId"] = Value::String(after_event_id);
        }
        if let Some(limit) = optional_u64(input, "limit")? {
            payload["limit"] = json!(limit);
        }
        payload["heartbeat"] = Value::Bool(true);
        self.get_attached_runtime_events(&payload)
    }

    fn session_launch(&self, input: &Value) -> Result<Value, Value> {
        Self::session_supervisor_call("launch", input, true)
    }

    fn session_list(&self, input: &Value) -> Result<Value, Value> {
        Self::session_supervisor_call("list", input, false)
    }

    fn session_inspect(&self, input: &Value) -> Result<Value, Value> {
        Self::session_supervisor_call("inspect", input, true)
    }

    fn session_terminate(&self, input: &Value) -> Result<Value, Value> {
        Self::session_supervisor_call("terminate", input, true)
    }

    fn session_mark_blocked(&self, input: &Value) -> Result<Value, Value> {
        Self::session_supervisor_call("mark_blocked", input, true)
    }

    fn session_resume_due(&self, input: &Value) -> Result<Value, Value> {
        Self::session_supervisor_call("resume_due", input, false)
    }

    fn session_classify_block(&self, input: &Value) -> Result<Value, Value> {
        Self::session_supervisor_call("classify_block", input, false)
    }

    fn session_supervisor_call(
        operation: &str,
        input: &Value,
        requires_worker_id: bool,
    ) -> Result<Value, Value> {
        let mut payload = json!({
            "operation": operation,
        });
        if let Some(state_path) = optional_string(input, "statePath") {
            payload["state_path"] = Value::String(state_path);
        }
        if let Some(dry_run) = optional_bool(input, "dryRun") {
            payload["dry_run"] = Value::Bool(dry_run);
        }
        if requires_worker_id {
            payload["worker_id"] = Value::String(required_string_arg(input, "workerId")?);
        } else if let Some(worker_id) = optional_string(input, "workerId") {
            payload["worker_id"] = Value::String(worker_id);
        }
        if let Some(host) = optional_string(input, "host") {
            payload["host"] = Value::String(host);
        }
        if let Some(cwd) = optional_string(input, "cwd") {
            payload["cwd"] = Value::String(cwd);
        }
        if let Some(prompt) = optional_string(input, "prompt") {
            payload["prompt"] = Value::String(prompt);
        }
        if let Some(resume_target) = optional_string(input, "resumeTarget") {
            payload["resume_target"] = Value::String(resume_target);
        }
        if let Some(resume_mode) = optional_string(input, "resumeMode") {
            payload["resume_mode"] = Value::String(resume_mode);
        }
        if let Some(tmux_session) = optional_string(input, "tmuxSession") {
            payload["tmux_session"] = Value::String(tmux_session);
        }
        if let Some(native_tmux) = optional_bool(input, "nativeTmux") {
            payload["native_tmux"] = Value::Bool(native_tmux);
        }
        if let Some(evidence_text) = optional_string(input, "evidenceText") {
            payload["evidence_text"] = Value::String(evidence_text);
        }
        if let Some(blocked_reason) = optional_string(input, "blockedReason") {
            payload["blocked_reason"] = Value::String(blocked_reason);
        }
        if let Some(backoff_seconds) = optional_u64(input, "backoffSeconds")? {
            payload["backoff_seconds"] = json!(backoff_seconds);
        }
        handle_session_supervisor_operation(payload)
            .map_err(|err| runtime_error("SESSION_SUPERVISOR_FAILED", &err))
    }

    fn background_list(&self, input: &Value) -> Result<Value, Value> {
        self.background_state_call("snapshot", input)
    }

    fn background_inspect(&self, input: &Value) -> Result<Value, Value> {
        let mut payload = json!({
            "operation": "get",
            "job_id": required_string_arg(input, "jobId")?,
        });
        Self::populate_background_common_fields(input, &mut payload)?;
        Self::dispatch_background_state(payload)
    }

    fn background_terminate(&self, input: &Value) -> Result<Value, Value> {
        let payload = json!({
            "operation": "apply_mutation",
            "job_id": required_string_arg(input, "jobId")?,
            "mutation": {
                "status": "interrupted",
                "error": optional_string(input, "error").unwrap_or_else(|| "terminated from browser-mcp".to_string()),
            }
        });
        let mut payload = payload;
        Self::populate_background_common_fields(input, &mut payload)?;
        Self::dispatch_background_state(payload)
    }

    fn background_state_call(&self, operation: &str, input: &Value) -> Result<Value, Value> {
        let mut payload = json!({
            "operation": operation,
        });
        Self::populate_background_common_fields(input, &mut payload)?;
        Self::dispatch_background_state(payload)
    }

    fn populate_background_common_fields(input: &Value, payload: &mut Value) -> Result<(), Value> {
        payload["schema_version"] =
            Value::String(BACKGROUND_STATE_REQUEST_SCHEMA_VERSION.to_string());
        if let Some(state_path) = optional_string(input, "statePath") {
            payload["state_path"] = Value::String(state_path);
        }
        if let Some(backend_family) = optional_string(input, "backendFamily") {
            payload["backend_family"] = Value::String(backend_family);
        }
        if let Some(sqlite_db_path) = optional_string(input, "sqliteDbPath") {
            payload["sqlite_db_path"] = Value::String(sqlite_db_path);
        }
        Ok(())
    }

    fn dispatch_background_state(payload: Value) -> Result<Value, Value> {
        handle_background_state_operation(payload)
            .map_err(|err| runtime_error("BACKGROUND_STATE_FAILED", &err))
    }

    fn open(&mut self, input: &Value) -> Result<Value, Value> {
        let url = required_string_arg(input, "url")?;
        let new_tab = optional_bool(input, "newTab").unwrap_or(false);
        let session_id = self.get_or_create_session()?;
        let tab_id = {
            let current_tab_id = self
                .sessions
                .get(&session_id)
                .and_then(|session| session.current_tab_id.clone());
            if new_tab || current_tab_id.is_none() {
                self.create_tab(&session_id)?
            } else {
                current_tab_id.unwrap_or_default()
            }
        };

        let session_cdp_id = self.tab_session_id(&session_id, &tab_id)?;
        let cdp = self.cdp_mut(&session_id)?;
        cdp.call(Some(&session_cdp_id), "Page.navigate", json!({"url": url}))?;
        self.wait_for_page_ready(&session_id, &tab_id, DEFAULT_WAIT_MS)?;
        self.refresh_snapshot(&session_id, &tab_id)?;
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.current_tab_id = Some(tab_id.clone());
        }

        Ok(json!({
            "session": self.session_view(&session_id)?,
            "tab": self.tab_view(&session_id, &tab_id)?,
        }))
    }

    fn tabs(&mut self, input: &Value) -> Result<Value, Value> {
        let action = required_string_arg(input, "action")?;
        let session_id = self.required_session_id()?;
        if action == "select" {
            let tab_id = required_string_arg(input, "tabId")?;
            if !self
                .sessions
                .get(&session_id)
                .is_some_and(|session| session.tabs.contains_key(&tab_id))
            {
                return Err(browser_error(
                    "TAB_NOT_FOUND",
                    &format!("Tab {tab_id} was not found."),
                    &["call browser_tabs with action=list"],
                    true,
                ));
            }
            if let Some(session) = self.sessions.get_mut(&session_id) {
                session.current_tab_id = Some(tab_id);
            }
        } else if action != "list" {
            return Err(browser_error(
                "INVALID_INPUT",
                "action must be list or select.",
                &["pass action=list or action=select"],
                true,
            ));
        }

        let session = self
            .sessions
            .get(&session_id)
            .ok_or_else(session_not_found_error)?;
        let tabs = session
            .tabs
            .keys()
            .map(|tab_id| self.tab_view(&session_id, tab_id))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(json!({"currentTabId": session.current_tab_id, "tabs": tabs}))
    }

    fn close(&mut self, input: &Value) -> Result<Value, Value> {
        let target = required_string_arg(input, "target")?;
        let session_id = self.required_session_id()?;
        if target == "session" {
            let remaining_tabs = self
                .sessions
                .get(&session_id)
                .map(|session| session.tabs.len())
                .unwrap_or_default();
            self.dispose_session(&session_id)?;
            return Ok(json!({"ok": true, "closed": "session", "remainingTabs": remaining_tabs}));
        }
        if target != "tab" {
            return Err(browser_error(
                "INVALID_INPUT",
                "target must be tab or session.",
                &["pass target=tab or target=session"],
                true,
            ));
        }
        let tab_id = optional_string(input, "tabId")
            .or_else(|| {
                self.sessions
                    .get(&session_id)
                    .and_then(|session| session.current_tab_id.clone())
            })
            .ok_or_else(|| {
                browser_error(
                    "TAB_NOT_FOUND",
                    "No active tab is available.",
                    &["call browser_open"],
                    true,
                )
            })?;
        let target_id = self
            .sessions
            .get(&session_id)
            .and_then(|session| session.tabs.get(&tab_id))
            .map(|tab| tab.target_id.clone())
            .ok_or_else(|| {
                browser_error(
                    "TAB_NOT_FOUND",
                    &format!("Tab {tab_id} was not found."),
                    &["call browser_tabs with action=list"],
                    true,
                )
            })?;
        let cdp = self.cdp_mut(&session_id)?;
        let _ = cdp.call(None, "Target.closeTarget", json!({"targetId": target_id}));
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.tabs.remove(&tab_id);
            session.current_tab_id = session.tabs.keys().next().cloned();
            let remaining = session.tabs.len();
            if remaining == 0 {
                let _ = self.dispose_session(&session_id);
            }
            return Ok(json!({"ok": true, "closed": "tab", "remainingTabs": remaining}));
        }
        Err(session_not_found_error())
    }

    fn get_state(&mut self, input: &Value) -> Result<Value, Value> {
        let (session_id, tab_id) = self.resolve_tab_ids(input)?;
        let previous = self
            .sessions
            .get(&session_id)
            .and_then(|session| session.tabs.get(&tab_id))
            .and_then(|tab| tab.last_snapshot.clone());
        let snapshot = self.refresh_snapshot(&session_id, &tab_id)?;
        let include = optional_string_array(input, "include").unwrap_or_else(|| {
            vec![
                "summary".to_string(),
                "interactive_elements".to_string(),
                "diff".to_string(),
            ]
        });
        let max_elements = optional_usize(input, "maxElements", DEFAULT_MAX_ELEMENTS)?;
        let text_budget = optional_usize(input, "textBudget", DEFAULT_TEXT_BUDGET)?;
        let since_revision = optional_u64(input, "sinceRevision")?;
        let base_snapshot = if let Some(revision) = since_revision {
            self.sessions
                .get(&session_id)
                .and_then(|session| session.tabs.get(&tab_id))
                .and_then(|tab| {
                    tab.snapshot_history
                        .iter()
                        .find(|snapshot| snapshot.revision == revision)
                        .cloned()
                })
        } else {
            previous
        };
        if since_revision.is_some() && base_snapshot.is_none() {
            return Err(browser_error(
                "STALE_STATE_REVISION",
                "Requested sinceRevision is no longer retained.",
                &["call browser_get_state without sinceRevision"],
                true,
            ));
        }

        let mut state = Map::new();
        state.insert("tab".to_string(), self.tab_view(&session_id, &tab_id)?);
        if include.iter().any(|item| item == "summary") {
            state.insert(
                "summary".to_string(),
                compact_summary(&snapshot.summary, text_budget),
            );
        }
        if include.iter().any(|item| item == "interactive_elements") {
            state.insert(
                "interactiveElements".to_string(),
                Value::Array(
                    snapshot
                        .interactive_elements
                        .iter()
                        .take(max_elements)
                        .map(interactive_element_value)
                        .collect(),
                ),
            );
        }
        if include.iter().any(|item| item == "diff") {
            let delta = base_snapshot
                .as_ref()
                .map(|base| compute_delta(base, &snapshot))
                .unwrap_or_else(|| {
                    json!({
                        "fromRevision": snapshot.revision,
                        "toRevision": snapshot.revision,
                        "urlChanged": false,
                        "titleChanged": false,
                        "newElements": [],
                        "removedRefs": [],
                        "newText": [],
                        "alerts": [],
                    })
                });
            state.insert("diff".to_string(), delta);
        }
        Ok(Value::Object(state))
    }

    fn get_elements(&mut self, input: &Value) -> Result<Value, Value> {
        let (session_id, tab_id) = self.resolve_tab_ids(input)?;
        let snapshot = self.refresh_snapshot(&session_id, &tab_id)?;
        let role = optional_string(input, "role").map(|value| value.to_lowercase());
        let query = optional_string(input, "query").map(|value| value.to_lowercase());
        let limit = optional_usize(input, "limit", DEFAULT_MAX_ELEMENTS)?;
        let matches = snapshot
            .interactive_elements
            .into_iter()
            .filter(|element| {
                role.as_ref()
                    .map(|role| element.role.to_lowercase() == *role)
                    .unwrap_or(true)
            })
            .filter(|element| {
                query
                    .as_ref()
                    .map(|query| {
                        format!("{} {}", element.name, element.text)
                            .to_lowercase()
                            .contains(query)
                    })
                    .unwrap_or(true)
            })
            .take(limit)
            .map(|element| interactive_element_value(&element))
            .collect::<Vec<_>>();
        Ok(json!({"matches": matches}))
    }

    fn get_text(&mut self, input: &Value) -> Result<Value, Value> {
        let (session_id, tab_id) = self.resolve_tab_ids(input)?;
        let max_chars = optional_usize(input, "maxChars", DEFAULT_TEXT_BUDGET)?;
        let text = if let Some(scope_ref) = optional_string(input, "scopeRef") {
            let selector = self.selector_for_ref(&session_id, &tab_id, &scope_ref)?;
            self.evaluate_string(
                &session_id,
                &tab_id,
                &format!(
                    "(function(){{const el=document.querySelector({}); return el ? (el.innerText || el.textContent || '') : '';}})()",
                    json_string_literal(&selector)
                ),
            )?
        } else {
            self.evaluate_string(
                &session_id,
                &tab_id,
                "document.body ? (document.body.innerText || '').replace(/\\s+$/g, '').trim() : ''",
            )?
        };
        Ok(
            json!({"text": truncate_text(&text, max_chars), "tab": self.tab_view(&session_id, &tab_id)?}),
        )
    }

    fn get_network(&mut self, input: &Value) -> Result<Value, Value> {
        let (session_id, tab_id) = self.resolve_tab_ids(input)?;
        self.drain_cdp_events(&session_id, DEFAULT_WAIT_MS / 5)?;
        let since_seconds = optional_u64(input, "sinceSeconds")?.unwrap_or(20);
        let limit = optional_usize(input, "limit", DEFAULT_NETWORK_LIMIT)?;
        let resource_types = optional_string_array(input, "resourceTypes")
            .unwrap_or_default()
            .into_iter()
            .map(|item| item.to_lowercase())
            .collect::<Vec<_>>();
        let cutoff = now_millis().saturating_sub((since_seconds as u128) * 1000);
        let requests = self
            .sessions
            .get(&session_id)
            .and_then(|session| session.tabs.get(&tab_id))
            .map(|tab| {
                tab.network_events
                    .iter()
                    .filter(|event| event.timestamp >= cutoff)
                    .filter(|event| {
                        resource_types.is_empty()
                            || resource_types.contains(&event.resource_type.to_lowercase())
                    })
                    .rev()
                    .take(limit)
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
            .into_iter()
            .rev()
            .map(network_event_value)
            .collect::<Vec<_>>();
        Ok(json!({"requests": requests}))
    }

    fn screenshot_result(&mut self, input: &Value) -> Result<Value, Value> {
        let (session_id, tab_id) = self.resolve_tab_ids(input)?;
        let full_page = optional_bool(input, "fullPage").unwrap_or(false);
        let clip = if let Some(scope_ref) = optional_string(input, "scopeRef") {
            Some(self.element_clip(&session_id, &tab_id, &scope_ref)?)
        } else {
            None
        };
        let image_id = format!("img_{}_{}", now_millis(), self.screenshot_counter + 1);
        self.screenshot_counter += 1;
        let screenshot_dir = self
            .repo_root
            .join("output")
            .join("browser-mcp-screenshots");
        fs::create_dir_all(&screenshot_dir).map_err(|err| {
            browser_error(
                "SCREENSHOT_FAILED",
                &format!("create screenshot directory failed: {err}"),
                &["verify output directory permissions"],
                true,
            )
        })?;
        let path = screenshot_dir.join(format!("{image_id}.png"));
        let mut params = Map::new();
        params.insert("format".to_string(), Value::String("png".to_string()));
        params.insert("fromSurface".to_string(), Value::Bool(true));
        if full_page {
            params.insert("captureBeyondViewport".to_string(), Value::Bool(true));
        }
        if let Some(clip) = clip {
            params.insert("clip".to_string(), clip);
        }
        let response = {
            let session_cdp_id = self.tab_session_id(&session_id, &tab_id)?;
            let cdp = self.cdp_mut(&session_id)?;
            cdp.call(
                Some(&session_cdp_id),
                "Page.captureScreenshot",
                Value::Object(params),
            )?
        };
        let data = response
            .get("data")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                browser_error(
                    "SCREENSHOT_FAILED",
                    "Chrome did not return screenshot data.",
                    &["try browser_screenshot again"],
                    true,
                )
            })?;
        let bytes = decode_base64(data).map_err(|err| {
            browser_error(
                "SCREENSHOT_FAILED",
                &format!("decode screenshot failed: {err}"),
                &["try browser_screenshot again"],
                true,
            )
        })?;
        fs::write(&path, bytes).map_err(|err| {
            browser_error(
                "SCREENSHOT_FAILED",
                &format!("write screenshot failed: {err}"),
                &["verify output directory permissions"],
                true,
            )
        })?;
        let meta = json!({"imageId": image_id, "path": path.to_string_lossy()});
        Ok(json!({
            "structuredContent": meta,
            "content": [
                {"type": "image", "data": data, "mimeType": "image/png"},
                {"type": "text", "text": serde_json::to_string_pretty(&meta).unwrap_or_else(|_| "{}".to_string())}
            ],
            "isError": false,
        }))
    }

    fn click(&mut self, input: &Value) -> Result<Value, Value> {
        let ref_id = required_string_arg(input, "ref")?;
        let timeout_ms = optional_u64(input, "timeoutMs")?.unwrap_or(DEFAULT_WAIT_MS);
        let (session_id, tab_id) = self.resolve_tab_ids(input)?;
        let before = self.refresh_snapshot(&session_id, &tab_id)?;
        let selector = self.selector_for_ref(&session_id, &tab_id, &ref_id)?;
        self.runtime_call(
            &session_id,
            &tab_id,
            &format!(
                "async function(){{const el=document.querySelector({}); if(!el) throw new Error('element not found'); el.scrollIntoView({{block:'center',inline:'center'}}); el.click(); return true;}}",
                json_string_literal(&selector)
            ),
            timeout_ms,
        )?;
        self.wait_for_page_ready(&session_id, &tab_id, timeout_ms)?;
        let after = self.refresh_snapshot(&session_id, &tab_id)?;
        Ok(json!({
            "ok": true,
            "action": "click",
            "ref": ref_id,
            "tab": self.tab_view(&session_id, &tab_id)?,
            "delta": compute_delta(&before, &after),
        }))
    }

    fn fill(&mut self, input: &Value) -> Result<Value, Value> {
        let ref_id = required_string_arg(input, "ref")?;
        let value = required_string_arg(input, "value")?;
        let submit = optional_bool(input, "submit").unwrap_or(false);
        let (session_id, tab_id) = self.resolve_tab_ids(input)?;
        let before = self.refresh_snapshot(&session_id, &tab_id)?;
        let selector = self.selector_for_ref(&session_id, &tab_id, &ref_id)?;
        self.runtime_call(
            &session_id,
            &tab_id,
            &format!(
                "async function(){{const el=document.querySelector({}); if(!el) throw new Error('element not found'); el.scrollIntoView({{block:'center',inline:'center'}}); el.focus(); el.value={}; el.dispatchEvent(new Event('input',{{bubbles:true}})); el.dispatchEvent(new Event('change',{{bubbles:true}})); if({}){{const event=new KeyboardEvent('keydown',{{key:'Enter',bubbles:true}}); el.dispatchEvent(event); if(el.form) el.form.requestSubmit ? el.form.requestSubmit() : el.form.submit();}} return true;}}",
                json_string_literal(&selector),
                json_string_literal(&value),
                if submit { "true" } else { "false" }
            ),
            DEFAULT_WAIT_MS,
        )?;
        self.wait_for_page_ready(&session_id, &tab_id, DEFAULT_WAIT_MS)?;
        let after = self.refresh_snapshot(&session_id, &tab_id)?;
        Ok(json!({
            "ok": true,
            "action": "fill",
            "ref": ref_id,
            "tab": self.tab_view(&session_id, &tab_id)?,
            "delta": compute_delta(&before, &after),
        }))
    }

    fn press(&mut self, input: &Value) -> Result<Value, Value> {
        let key = required_string_arg(input, "key")?;
        let (session_id, tab_id) = self.resolve_tab_ids(input)?;
        let before = self.refresh_snapshot(&session_id, &tab_id)?;
        let cdp_key = cdp_key_name(&key);
        let session_cdp_id = self.tab_session_id(&session_id, &tab_id)?;
        let cdp = self.cdp_mut(&session_id)?;
        cdp.call(
            Some(&session_cdp_id),
            "Input.dispatchKeyEvent",
            json!({"type": "keyDown", "key": cdp_key}),
        )?;
        cdp.call(
            Some(&session_cdp_id),
            "Input.dispatchKeyEvent",
            json!({"type": "keyUp", "key": cdp_key}),
        )?;
        self.wait_for_page_ready(&session_id, &tab_id, DEFAULT_WAIT_MS)?;
        let after = self.refresh_snapshot(&session_id, &tab_id)?;
        Ok(json!({
            "ok": true,
            "action": "press",
            "tab": self.tab_view(&session_id, &tab_id)?,
            "delta": compute_delta(&before, &after),
        }))
    }

    fn wait_for(&mut self, input: &Value) -> Result<Value, Value> {
        let condition = input.get("condition").ok_or_else(|| {
            browser_error(
                "INVALID_INPUT",
                "condition is required.",
                &["provide condition.type"],
                true,
            )
        })?;
        let condition_type = required_string_arg(condition, "type")?;
        let condition_value = optional_string(condition, "value");
        let timeout_ms = optional_u64(input, "timeoutMs")?.unwrap_or(DEFAULT_WAIT_MS);
        let (session_id, tab_id) = self.resolve_tab_ids(input)?;
        match condition_type.as_str() {
            "text_appears" => {
                let value = condition_value.ok_or_else(|| {
                    browser_error(
                        "INVALID_INPUT",
                        "value is required for text_appears.",
                        &["provide condition.value"],
                        true,
                    )
                })?;
                self.wait_for_js_condition(
                    &session_id,
                    &tab_id,
                    &format!(
                        "document.body && document.body.innerText.includes({})",
                        json_string_literal(&value)
                    ),
                    timeout_ms,
                )?;
            }
            "text_disappears" => {
                let value = condition_value.ok_or_else(|| {
                    browser_error(
                        "INVALID_INPUT",
                        "value is required for text_disappears.",
                        &["provide condition.value"],
                        true,
                    )
                })?;
                self.wait_for_js_condition(
                    &session_id,
                    &tab_id,
                    &format!(
                        "!(document.body && document.body.innerText.includes({}))",
                        json_string_literal(&value)
                    ),
                    timeout_ms,
                )?;
            }
            "element_appears" => {
                let ref_id = condition_value.ok_or_else(|| {
                    browser_error(
                        "INVALID_INPUT",
                        "value is required for element_appears.",
                        &["provide element ref"],
                        true,
                    )
                })?;
                let selector = self.selector_for_ref(&session_id, &tab_id, &ref_id)?;
                self.wait_for_js_condition(
                    &session_id,
                    &tab_id,
                    &format!(
                        "!!document.querySelector({})",
                        json_string_literal(&selector)
                    ),
                    timeout_ms,
                )?;
            }
            "element_disappears" => {
                let ref_id = condition_value.ok_or_else(|| {
                    browser_error(
                        "INVALID_INPUT",
                        "value is required for element_disappears.",
                        &["provide element ref"],
                        true,
                    )
                })?;
                let selector = self.selector_for_ref(&session_id, &tab_id, &ref_id)?;
                self.wait_for_js_condition(
                    &session_id,
                    &tab_id,
                    &format!(
                        "!document.querySelector({})",
                        json_string_literal(&selector)
                    ),
                    timeout_ms,
                )?;
            }
            "url_contains" => {
                let value = condition_value.ok_or_else(|| {
                    browser_error(
                        "INVALID_INPUT",
                        "value is required for url_contains.",
                        &["provide condition.value"],
                        true,
                    )
                })?;
                self.wait_for_js_condition(
                    &session_id,
                    &tab_id,
                    &format!("location.href.includes({})", json_string_literal(&value)),
                    timeout_ms,
                )?;
            }
            "network_idle" => self.drain_cdp_events(&session_id, timeout_ms)?,
            _ => {
                return Err(browser_error(
                    "UNSUPPORTED_OPERATION",
                    &format!("Unsupported wait condition: {condition_type}."),
                    &["use a supported condition type"],
                    true,
                ))
            }
        }
        self.refresh_snapshot(&session_id, &tab_id)?;
        Ok(json!({"ok": true, "tab": self.tab_view(&session_id, &tab_id)?, "condition": condition}))
    }

    fn save_session(&mut self, input: &Value) -> Result<Value, Value> {
        let session_id = self.required_session_id()?;
        let default_path = self
            .repo_root
            .join("output")
            .join("browser-mcp-sessions")
            .join(format!("{session_id}.json"));
        let session_path = optional_string(input, "sessionPath")
            .map(PathBuf::from)
            .unwrap_or(default_path);
        if let Some(parent) = session_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                browser_error(
                    "SESSION_SAVE_FAILED",
                    &format!("create session directory failed: {err}"),
                    &["verify output directory permissions"],
                    true,
                )
            })?;
        }
        let cdp = self.cdp_mut(&session_id)?;
        let cookies = cdp.call(None, "Storage.getCookies", json!({}))?;
        fs::write(
            &session_path,
            serde_json::to_string_pretty(&json!({
                "schemaVersion": "browser-mcp-rust-session-v1",
                "savedAt": current_local_timestamp(),
                "cookies": cookies.get("cookies").cloned().unwrap_or_else(|| json!([])),
            }))
            .unwrap_or_else(|_| "{}".to_string()),
        )
        .map_err(|err| {
            browser_error(
                "SESSION_SAVE_FAILED",
                &format!("write session failed: {err}"),
                &["verify output directory permissions"],
                true,
            )
        })?;
        Ok(
            json!({"ok": true, "path": session_path.to_string_lossy(), "savedAt": current_local_timestamp()}),
        )
    }

    fn restore_session(&mut self, input: &Value) -> Result<Value, Value> {
        let session_path = PathBuf::from(required_string_arg(input, "sessionPath")?);
        let raw = fs::read_to_string(&session_path).map_err(|err| {
            browser_error(
                "INVALID_INPUT",
                &format!(
                    "Session snapshot not found: {} ({err})",
                    session_path.display()
                ),
                &["call browser_save_session first", "verify the path"],
                true,
            )
        })?;
        let payload: Value = serde_json::from_str(&raw).map_err(|err| {
            browser_error(
                "INVALID_INPUT",
                &format!("Session snapshot is invalid JSON: {err}"),
                &["call browser_save_session again"],
                true,
            )
        })?;
        let session_id = self.get_or_create_session()?;
        if let Some(cookies) = payload.get("cookies").and_then(Value::as_array) {
            let cdp = self.cdp_mut(&session_id)?;
            cdp.call(None, "Storage.setCookies", json!({"cookies": cookies}))
                .map_err(|err| {
                    browser_error(
                        "SESSION_RESTORE_FAILED",
                        &format!("restore cookies failed: {err}"),
                        &[
                            "call browser_save_session again",
                            "verify the session snapshot is valid",
                        ],
                        true,
                    )
                })?;
        }
        Ok(
            json!({"ok": true, "restoredFrom": session_path.to_string_lossy(), "sessionId": session_id}),
        )
    }

    fn get_attached_runtime_events(&mut self, input: &Value) -> Result<Value, Value> {
        let limit = optional_usize(input, "limit", 100)?;
        if limit == 0 {
            return Err(browser_error(
                "INVALID_INPUT",
                "limit must be a positive integer.",
                &["provide a positive integer limit"],
                true,
            ));
        }
        let resolved = self.resolve_attached_runtime_descriptor_context()?;
        let after_event_id = optional_string(input, "afterEventId");
        let replay = replay_trace_stream(TraceStreamReplayRequestPayload {
            path: Some(resolved.trace_stream_path.clone()),
            event_stream_text: None,
            compaction_manifest_path: None,
            compaction_manifest_text: None,
            compaction_state_text: None,
            compaction_artifact_index_text: None,
            compaction_delta_text: None,
            session_id: None,
            job_id: None,
            stream_scope_fields: None,
            after_event_id: after_event_id.clone(),
            limit: Some(limit),
        })
        .map_err(|err| {
            if err.contains("Unknown event id for stream resume") {
                browser_error(
                    "ATTACHED_RUNTIME_CURSOR_NOT_FOUND",
                    &format!(
                        "No attached runtime event was found for afterEventId={}.",
                        after_event_id.clone().unwrap_or_default()
                    ),
                    &[
                        "call browser_get_attached_runtime_events without afterEventId",
                        "inspect browser_diagnostics",
                    ],
                    true,
                )
            } else {
                browser_error(
                    "ATTACHED_RUNTIME_TRACE_UNAVAILABLE",
                    &err,
                    &[
                        "inspect browser_diagnostics",
                        "refresh the attach descriptor or trace artifacts",
                    ],
                    true,
                )
            }
        })?;
        if replay.schema_version != ROUTER_RS_TRACE_STREAM_REPLAY_SCHEMA_VERSION
            || replay.authority != ROUTER_RS_TRACE_IO_AUTHORITY
        {
            return Err(browser_error(
                "ATTACHED_RUNTIME_TRACE_UNAVAILABLE",
                "router-rs trace replay returned an unexpected schema.",
                &[
                    "inspect browser_diagnostics",
                    "refresh the attach descriptor or trace artifacts",
                ],
                true,
            ));
        }
        let last_event = replay.events.last();
        let next_cursor = last_event.map(|event| {
            json!({
                "eventId": event.get("event_id").and_then(Value::as_str),
                "eventIndex": replay.next_cursor.as_ref().map(|cursor| cursor.event_index).unwrap_or_else(|| replay.window_start_index + replay.events.len().saturating_sub(1)),
            })
        });
        let mut attached_runtime = resolved.diagnostics_base.clone();
        attached_runtime["eventCount"] = json!(replay.event_count);
        attached_runtime["latestEventId"] = opt_string_value(replay.latest_event_id);
        attached_runtime["latestEventKind"] = opt_string_value(replay.latest_event_kind);
        attached_runtime["latestEventTimestamp"] = opt_string_value(replay.latest_event_timestamp);
        Ok(json!({
            "ok": true,
            "attachedRuntime": attached_runtime,
            "replayContext": attached_runtime_replay_context(&resolved.diagnostics_base),
            "events": replay.events,
            "afterEventId": after_event_id,
            "hasMore": replay.has_more,
            "nextCursor": next_cursor,
            "heartbeat": if optional_bool(input, "heartbeat").unwrap_or(false) && replay.events.is_empty() { json!({"status": "idle"}) } else { Value::Null },
        }))
    }

    fn diagnostics(&mut self, _input: &Value) -> Result<Value, Value> {
        let mut tabs = 0usize;
        let mut network_events = 0usize;
        let runtime_path = skill_runtime_path(&self.repo_root);
        let manifest_path = skill_manifest_path(&self.repo_root);
        let routing_tools_exposed = skill_runtime_available(&self.repo_root);
        let mut skill_remediation = Vec::new();
        if !runtime_path.is_file() {
            skill_remediation.push(format!("generate {}", runtime_path.to_string_lossy()));
        }
        if !manifest_path.is_file() {
            skill_remediation.push(format!("generate {}", manifest_path.to_string_lossy()));
        }
        if skill_remediation.is_empty() {
            skill_remediation.push("skill runtime artifacts look healthy".to_string());
        }
        for session in self.sessions.values() {
            tabs += session.tabs.len();
            for tab in session.tabs.values() {
                network_events += tab.network_events.len();
            }
        }
        let screenshot_count = fs::read_dir(
            self.repo_root
                .join("output")
                .join("browser-mcp-screenshots"),
        )
        .ok()
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry.path().extension().and_then(|value| value.to_str()) == Some("png")
                })
                .count()
        })
        .unwrap_or(0);
        Ok(json!({
            "sessions": self.sessions.len(),
            "tabs": tabs,
            "networkEventBufferSize": network_events,
            "screenshotCount": screenshot_count,
            "runtimeVersion": SERVER_VERSION,
            "skillRouting": {
                "runtimePath": runtime_path.to_string_lossy(),
                "runtimeExists": runtime_path.is_file(),
                "manifestPath": manifest_path.to_string_lossy(),
                "manifestExists": manifest_path.is_file(),
                "routingToolsExposed": routing_tools_exposed,
                "remediation": skill_remediation,
            },
            "attachedRuntime": self.attached_runtime_diagnostics(),
        }))
    }

    fn attached_runtime_diagnostics(&self) -> Value {
        let configured_source = self.configured_runtime_attach_source();
        let base = base_attached_runtime_diagnostics(&configured_source);
        if configured_source.source.is_none() {
            return base;
        }
        match self.resolve_attached_runtime_descriptor_context() {
            Ok(resolved) => match inspect_trace_stream(TraceStreamInspectRequestPayload {
                path: Some(resolved.trace_stream_path),
                event_stream_text: None,
                compaction_manifest_path: None,
                compaction_manifest_text: None,
                compaction_state_text: None,
                compaction_artifact_index_text: None,
                compaction_delta_text: None,
                session_id: None,
                job_id: None,
                stream_scope_fields: None,
            }) {
                Ok(summary) => {
                    if summary.schema_version != ROUTER_RS_TRACE_STREAM_INSPECT_SCHEMA_VERSION
                        || summary.authority != ROUTER_RS_TRACE_IO_AUTHORITY
                    {
                        let mut diagnostics = resolved.diagnostics_base;
                        diagnostics["status"] = Value::String("trace_unavailable".to_string());
                        diagnostics["warning"] = Value::String(
                            "router-rs trace inspect returned an unexpected schema.".to_string(),
                        );
                        return diagnostics;
                    }
                    let mut diagnostics = resolved.diagnostics_base;
                    diagnostics["eventCount"] = json!(summary.event_count);
                    diagnostics["latestEventId"] = opt_string_value(summary.latest_event_id);
                    diagnostics["latestEventKind"] = opt_string_value(summary.latest_event_kind);
                    diagnostics["latestEventTimestamp"] =
                        opt_string_value(summary.latest_event_timestamp);
                    diagnostics
                }
                Err(err) => {
                    let mut diagnostics = resolved.diagnostics_base;
                    diagnostics["status"] = Value::String("trace_unavailable".to_string());
                    diagnostics["warning"] = Value::String(err);
                    diagnostics
                }
            },
            Err(error) => self.attached_runtime_error_diagnostics(&configured_source, base, error),
        }
    }

    fn attached_runtime_error_diagnostics(
        &self,
        configured_source: &ConfiguredAttachSource,
        base: Value,
        error: Value,
    ) -> Value {
        let code = error.get("code").and_then(Value::as_str).unwrap_or("");
        let mut diagnostics = self
            .load_runtime_attach_descriptor()
            .ok()
            .map(|loaded| {
                self.project_attached_runtime_diagnostics(
                    configured_source,
                    &loaded.descriptor,
                    loaded.input_artifact_kind,
                    descriptor_resolved_artifact(&loaded.descriptor, "trace_stream_path"),
                )
            })
            .unwrap_or(base);
        diagnostics["status"] = Value::String(
            match code {
                "ATTACHED_RUNTIME_UNSUPPORTED_BACKEND" => "unsupported_backend",
                "ATTACHED_RUNTIME_TRACE_UNAVAILABLE" => "trace_unavailable",
                _ => "invalid_descriptor",
            }
            .to_string(),
        );
        diagnostics["warning"] = error.get("message").cloned().unwrap_or_else(|| {
            Value::String("failed to load runtime attach descriptor".to_string())
        });
        diagnostics
    }

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

    fn get_or_create_session(&mut self) -> Result<String, Value> {
        if let Some(session_id) = self.sessions.keys().next().cloned() {
            return Ok(session_id);
        }
        let chrome_path = find_chrome_binary()?;
        let port = allocate_debug_port();
        let session_id = format!("sess_{:03}", self.session_counter + 1);
        self.session_counter += 1;
        let user_data_dir = std::env::temp_dir().join(format!(
            "browser-mcp-rust-{}-{}",
            std::process::id(),
            now_millis()
        ));
        fs::create_dir_all(&user_data_dir).map_err(|err| {
            browser_error(
                "BROWSER_LAUNCH_FAILED",
                &format!("create user data dir failed: {err}"),
                &["verify temp directory permissions"],
                false,
            )
        })?;
        let mut command = Command::new(&chrome_path);
        command
            .arg(format!("--remote-debugging-port={port}"))
            .arg(format!("--user-data-dir={}", user_data_dir.display()));
        if self.attach_config.headless {
            command.arg("--headless=new");
        }
        let child = command
            .arg("--disable-gpu")
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("about:blank")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| {
                browser_error(
                    "BROWSER_LAUNCH_FAILED",
                    &format!("launch Chrome failed: {err}"),
                    &["install Google Chrome or set BROWSER_MCP_CHROME_PATH"],
                    false,
                )
            })?;
        let browser_pid = child.id();
        wait_for_cdp(port)?;
        self.browser_processes.insert(session_id.clone(), child);
        self.sessions.insert(
            session_id.clone(),
            SessionRecord {
                id: session_id.clone(),
                created_at: current_local_timestamp(),
                viewport: ViewportSize {
                    width: 1440,
                    height: 900,
                },
                current_tab_id: None,
                tabs: HashMap::new(),
                _browser_pid: browser_pid,
                user_data_dir,
                cdp: CdpClient::connect(port)?,
            },
        );
        Ok(session_id)
    }

    fn create_tab(&mut self, session_id: &str) -> Result<String, Value> {
        let target = self.cdp_mut(session_id)?.call(
            None,
            "Target.createTarget",
            json!({"url": "about:blank"}),
        )?;
        let target_id = target
            .get("targetId")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                browser_error(
                    "BROWSER_TARGET_FAILED",
                    "Chrome did not return a targetId.",
                    &["try browser_open again"],
                    true,
                )
            })?
            .to_string();
        let attached = self.cdp_mut(session_id)?.call(
            None,
            "Target.attachToTarget",
            json!({"targetId": target_id, "flatten": true}),
        )?;
        let session_cdp_id = attached
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                browser_error(
                    "BROWSER_TARGET_FAILED",
                    "Chrome did not return a CDP sessionId.",
                    &["try browser_open again"],
                    true,
                )
            })?
            .to_string();
        let tab_id = format!("tab_{:02}", self.tab_counter + 1);
        self.tab_counter += 1;
        {
            let cdp = self.cdp_mut(session_id)?;
            cdp.call(Some(&session_cdp_id), "Page.enable", json!({}))?;
            cdp.call(Some(&session_cdp_id), "Runtime.enable", json!({}))?;
            cdp.call(Some(&session_cdp_id), "Network.enable", json!({}))?;
            cdp.call(
                Some(&session_cdp_id),
                "Emulation.setDeviceMetricsOverride",
                json!({"width": 1440, "height": 900, "deviceScaleFactor": 1, "mobile": false}),
            )?;
        }
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.tabs.insert(
                tab_id.clone(),
                TabRecord {
                    id: tab_id.clone(),
                    target_id,
                    session_id: session_cdp_id,
                    url: "about:blank".to_string(),
                    title: "Untitled".to_string(),
                    page_revision: 0,
                    loading_state: "loading".to_string(),
                    indexed_elements: HashMap::new(),
                    fingerprint_to_ref: HashMap::new(),
                    last_snapshot: None,
                    snapshot_history: VecDeque::new(),
                    network_events: Vec::new(),
                },
            );
            session.current_tab_id = Some(tab_id.clone());
        }
        Ok(tab_id)
    }

    fn dispose_session(&mut self, session_id: &str) -> Result<(), Value> {
        if let Some(mut child) = self.browser_processes.remove(session_id) {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(session) = self.sessions.remove(session_id) {
            let _ = fs::remove_dir_all(session.user_data_dir);
        }
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), Value> {
        let ids = self.sessions.keys().cloned().collect::<Vec<_>>();
        for session_id in ids {
            self.dispose_session(&session_id)?;
        }
        Ok(())
    }

    fn cdp_mut(&mut self, session_id: &str) -> Result<&mut CdpClient, Value> {
        self.sessions
            .get_mut(session_id)
            .map(|session| &mut session.cdp)
            .ok_or_else(session_not_found_error)
    }

    fn required_session_id(&self) -> Result<String, Value> {
        self.sessions
            .keys()
            .next()
            .cloned()
            .ok_or_else(session_not_found_error)
    }

    fn resolve_tab_ids(&self, input: &Value) -> Result<(String, String), Value> {
        let session_id = self.required_session_id()?;
        let tab_id = optional_string(input, "tabId")
            .or_else(|| {
                self.sessions
                    .get(&session_id)
                    .and_then(|session| session.current_tab_id.clone())
            })
            .ok_or_else(|| {
                browser_error(
                    "TAB_NOT_FOUND",
                    "No active tab exists.",
                    &["call browser_open"],
                    true,
                )
            })?;
        if !self
            .sessions
            .get(&session_id)
            .is_some_and(|session| session.tabs.contains_key(&tab_id))
        {
            return Err(browser_error(
                "TAB_NOT_FOUND",
                &format!("Tab {tab_id} was not found."),
                &["call browser_tabs with action=list"],
                true,
            ));
        }
        Ok((session_id, tab_id))
    }

    fn tab_session_id(&self, session_id: &str, tab_id: &str) -> Result<String, Value> {
        self.sessions
            .get(session_id)
            .and_then(|session| session.tabs.get(tab_id))
            .map(|tab| tab.session_id.clone())
            .ok_or_else(|| {
                browser_error(
                    "TAB_NOT_FOUND",
                    &format!("Tab {tab_id} was not found."),
                    &["call browser_tabs with action=list"],
                    true,
                )
            })
    }

    fn session_view(&self, session_id: &str) -> Result<Value, Value> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or_else(session_not_found_error)?;
        Ok(json!({
            "sessionId": session.id,
            "createdAt": session.created_at,
            "viewport": {"width": session.viewport.width, "height": session.viewport.height},
            "currentTabId": session.current_tab_id,
        }))
    }

    fn tab_view(&self, session_id: &str, tab_id: &str) -> Result<Value, Value> {
        let tab = self
            .sessions
            .get(session_id)
            .and_then(|session| session.tabs.get(tab_id))
            .ok_or_else(|| {
                browser_error(
                    "TAB_NOT_FOUND",
                    &format!("Tab {tab_id} was not found."),
                    &["call browser_tabs with action=list"],
                    true,
                )
            })?;
        Ok(json!({
            "tabId": tab.id,
            "url": tab.url,
            "title": tab.title,
            "pageRevision": tab.page_revision,
            "loadingState": tab.loading_state,
        }))
    }

    fn wait_for_page_ready(
        &mut self,
        session_id: &str,
        tab_id: &str,
        timeout_ms: u64,
    ) -> Result<(), Value> {
        let deadline = SystemTime::now() + Duration::from_millis(timeout_ms);
        while SystemTime::now() < deadline {
            self.drain_cdp_events(session_id, 100)?;
            let state = self.evaluate_string(session_id, tab_id, "document.readyState")?;
            if state == "complete" || state == "interactive" {
                self.drain_cdp_events(session_id, 250)?;
                return Ok(());
            }
        }
        Err(browser_error(
            "BROWSER_PAGE_NOT_READY",
            "Page readiness timed out before document.readyState became interactive/complete.",
            &[
                "wait briefly and retry",
                "verify the target page is accessible",
            ],
            true,
        ))
    }

    fn refresh_snapshot(&mut self, session_id: &str, tab_id: &str) -> Result<PageSnapshot, Value> {
        self.drain_cdp_events(session_id, 250)?;
        let previous_ref_map = self
            .sessions
            .get(session_id)
            .and_then(|session| session.tabs.get(tab_id))
            .map(|tab| tab.fingerprint_to_ref.clone())
            .unwrap_or_default();
        let snapshot = self.capture_snapshot(session_id, tab_id, &previous_ref_map)?;
        let mut effective = snapshot.clone();
        if let Some(session) = self.sessions.get_mut(session_id) {
            let tab = session.tabs.get_mut(tab_id).ok_or_else(|| {
                browser_error(
                    "TAB_NOT_FOUND",
                    &format!("Tab {tab_id} was not found."),
                    &["call browser_tabs with action=list"],
                    true,
                )
            })?;
            let changed = tab
                .last_snapshot
                .as_ref()
                .map(|previous| has_meaningful_change(previous, &snapshot))
                .unwrap_or(true);
            if changed {
                tab.page_revision += 1;
                effective.revision = tab.page_revision;
                for element in &mut effective.interactive_elements {
                    element.page_revision = tab.page_revision;
                }
                tab.last_snapshot = Some(effective.clone());
                tab.snapshot_history.push_back(effective.clone());
                while tab.snapshot_history.len() > SNAPSHOT_HISTORY_LIMIT {
                    tab.snapshot_history.pop_front();
                }
            } else if let Some(last) = tab.last_snapshot.clone() {
                effective = last;
            }
            tab.url = effective.url.clone();
            tab.title = effective.title.clone();
            tab.loading_state = effective.loading_state.clone();
            tab.indexed_elements = effective
                .interactive_elements
                .iter()
                .map(|element| (element.ref_id.clone(), element.clone()))
                .collect();
            tab.fingerprint_to_ref = effective
                .interactive_elements
                .iter()
                .map(|element| (element.fingerprint.clone(), element.ref_id.clone()))
                .collect();
        }
        Ok(effective)
    }

    fn capture_snapshot(
        &mut self,
        session_id: &str,
        tab_id: &str,
        previous_ref_map: &HashMap<String, String>,
    ) -> Result<PageSnapshot, Value> {
        let loading_state = self.detect_loading_state(session_id, tab_id)?;
        let title = self.evaluate_string(session_id, tab_id, "document.title")?;
        let url = self.evaluate_string(session_id, tab_id, "location.href")?;
        let summary = self.evaluate_json(session_id, tab_id, summary_expression())?;
        let text_content = truncate_text(
            &self.evaluate_string(
                session_id,
                tab_id,
                "document.body ? (document.body.innerText || '').replace(/\\s+$/g, '').trim() : ''",
            )?,
            DEFAULT_TEXT_BUDGET,
        );
        let descriptors = self.collect_element_descriptors(session_id, tab_id)?;
        let interactive_elements = self.build_interactive_elements(descriptors, previous_ref_map);
        Ok(PageSnapshot {
            revision: 0,
            url,
            title,
            loading_state,
            summary,
            interactive_elements,
            text_lines: to_text_lines(&text_content),
            text_content,
            _created_at: now_millis(),
        })
    }

    fn detect_loading_state(&mut self, session_id: &str, tab_id: &str) -> Result<String, Value> {
        match self
            .evaluate_string(session_id, tab_id, "document.readyState")?
            .as_str()
        {
            "loading" => Ok("loading".to_string()),
            "interactive" => Ok("domcontentloaded".to_string()),
            _ => Ok("idle".to_string()),
        }
    }

    fn collect_element_descriptors(
        &mut self,
        session_id: &str,
        tab_id: &str,
    ) -> Result<Vec<ElementDescriptor>, Value> {
        let payload = self.evaluate_json(session_id, tab_id, element_collection_expression())?;
        let items = payload.as_array().cloned().unwrap_or_default();
        let mut descriptors = Vec::new();
        for item in items {
            descriptors.push(ElementDescriptor {
                role: value_str(item.get("role")).to_string(),
                name: value_str(item.get("name")).to_string(),
                text: value_str(item.get("text")).to_string(),
                visible: item
                    .get("visible")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                enabled: item.get("enabled").and_then(Value::as_bool).unwrap_or(true),
                tag: value_str(item.get("tag")).to_string(),
                test_id: item
                    .get("testId")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                _ordinal: item.get("ordinal").and_then(Value::as_u64).unwrap_or(0) as usize,
                selector: value_str(item.get("selector")).to_string(),
            });
        }
        Ok(descriptors)
    }

    fn build_interactive_elements(
        &mut self,
        descriptors: Vec<ElementDescriptor>,
        previous_ref_map: &HashMap<String, String>,
    ) -> Vec<InteractiveElement> {
        let mut fingerprint_counts: HashMap<String, usize> = HashMap::new();
        descriptors
            .into_iter()
            .take(DEFAULT_MAX_ELEMENTS * 3)
            .map(|descriptor| {
                let fingerprint = create_fingerprint(&descriptor, &mut fingerprint_counts);
                let ref_id = previous_ref_map
                    .get(&fingerprint)
                    .cloned()
                    .unwrap_or_else(|| {
                        self.ref_counter += 1;
                        format!("el_{}", self.ref_counter)
                    });
                InteractiveElement {
                    ref_id,
                    page_revision: 0,
                    role: descriptor.role,
                    name: descriptor.name,
                    text: descriptor.text,
                    visible: descriptor.visible,
                    enabled: descriptor.enabled,
                    tag: descriptor.tag,
                    test_id: descriptor.test_id,
                    fingerprint,
                    selector: descriptor.selector,
                }
            })
            .collect()
    }

    fn selector_for_ref(
        &self,
        session_id: &str,
        tab_id: &str,
        ref_id: &str,
    ) -> Result<String, Value> {
        let tab = self
            .sessions
            .get(session_id)
            .and_then(|session| session.tabs.get(tab_id))
            .ok_or_else(|| {
                browser_error(
                    "TAB_NOT_FOUND",
                    &format!("Tab {tab_id} was not found."),
                    &["call browser_tabs with action=list"],
                    true,
                )
            })?;
        let element = tab.indexed_elements.get(ref_id).ok_or_else(|| {
            browser_error(
                "STALE_ELEMENT_REF",
                &format!("Element ref {ref_id} is stale or unknown."),
                &["call browser_get_state", "call browser_get_elements"],
                true,
            )
        })?;
        if element.page_revision != tab.page_revision {
            return Err(browser_error(
                "STALE_ELEMENT_REF",
                &format!(
                    "Ref {ref_id} belongs to revision {}; current is {}.",
                    element.page_revision, tab.page_revision
                ),
                &["call browser_get_state", "call browser_get_elements"],
                true,
            ));
        }
        Ok(element.selector.clone())
    }

    fn element_clip(
        &mut self,
        session_id: &str,
        tab_id: &str,
        ref_id: &str,
    ) -> Result<Value, Value> {
        let selector = self.selector_for_ref(session_id, tab_id, ref_id)?;
        let payload = self.evaluate_json(
            session_id,
            tab_id,
            &format!(
                "(function(){{const el=document.querySelector({}); if(!el) return null; const r=el.getBoundingClientRect(); return {{x:Math.max(0,r.x), y:Math.max(0,r.y), width:Math.max(1,r.width), height:Math.max(1,r.height), scale:1}};}})()",
                json_string_literal(&selector)
            ),
        )?;
        if payload.is_null() {
            return Err(browser_error(
                "ELEMENT_NOT_VISIBLE",
                &format!("Unable to resolve locator for {ref_id}."),
                &["call browser_get_state", "use a fresher ref"],
                true,
            ));
        }
        Ok(payload)
    }

    fn evaluate_string(
        &mut self,
        session_id: &str,
        tab_id: &str,
        expression: &str,
    ) -> Result<String, Value> {
        let value = self.evaluate_json(session_id, tab_id, expression)?;
        Ok(value_string(Some(&value)))
    }

    fn evaluate_json(
        &mut self,
        session_id: &str,
        tab_id: &str,
        expression: &str,
    ) -> Result<Value, Value> {
        let session_cdp_id = self.tab_session_id(session_id, tab_id)?;
        let cdp = self.cdp_mut(session_id)?;
        let response = cdp.call(
            Some(&session_cdp_id),
            "Runtime.evaluate",
            json!({"expression": expression, "returnByValue": true, "awaitPromise": true}),
        )?;
        if let Some(details) = response.get("exceptionDetails") {
            return Err(browser_error(
                "EVALUATION_FAILED",
                &format!("page evaluation failed: {details}"),
                &["retry after the page settles"],
                true,
            ));
        }
        Ok(response
            .get("result")
            .and_then(|result| result.get("value"))
            .cloned()
            .unwrap_or(Value::Null))
    }

    fn runtime_call(
        &mut self,
        session_id: &str,
        tab_id: &str,
        declaration: &str,
        _timeout_ms: u64,
    ) -> Result<Value, Value> {
        let session_cdp_id = self.tab_session_id(session_id, tab_id)?;
        let cdp = self.cdp_mut(session_id)?;
        let response = cdp.call(
            Some(&session_cdp_id),
            "Runtime.evaluate",
            json!({"expression": format!("({declaration})()"), "awaitPromise": true, "returnByValue": true}),
        )?;
        if response.get("exceptionDetails").is_some() {
            return Err(browser_error(
                "ACTION_FAILED",
                "browser action failed in page context.",
                &["call browser_get_state", "use a fresher ref"],
                true,
            ));
        }
        Ok(response
            .get("result")
            .and_then(|result| result.get("value"))
            .cloned()
            .unwrap_or(Value::Null))
    }

    fn wait_for_js_condition(
        &mut self,
        session_id: &str,
        tab_id: &str,
        expression: &str,
        timeout_ms: u64,
    ) -> Result<(), Value> {
        let deadline = SystemTime::now() + Duration::from_millis(timeout_ms);
        while SystemTime::now() < deadline {
            if self
                .evaluate_json(session_id, tab_id, expression)
                .ok()
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
            {
                return Ok(());
            }
            self.drain_cdp_events(session_id, 100)?;
        }
        Err(browser_error(
            "WAIT_TIMEOUT",
            "Timed out waiting for browser condition.",
            &["inspect browser_get_state", "increase timeoutMs"],
            true,
        ))
    }

    fn drain_cdp_events(&mut self, session_id: &str, timeout_ms: u64) -> Result<(), Value> {
        let events = {
            let cdp = self.cdp_mut(session_id)?;
            cdp.drain_events(Duration::from_millis(timeout_ms))?
        };
        for event in events {
            self.handle_cdp_event(session_id, event);
        }
        Ok(())
    }

    fn handle_cdp_event(&mut self, session_id: &str, event: Value) {
        let method = event.get("method").and_then(Value::as_str).unwrap_or("");
        let cdp_session_id = event.get("sessionId").and_then(Value::as_str).unwrap_or("");
        let params = event.get("params").cloned().unwrap_or_else(|| json!({}));
        let Some(tab_id) = self.tab_id_by_cdp_session(session_id, cdp_session_id) else {
            return;
        };
        if method == "Network.responseReceived" {
            let response = params.get("response").cloned().unwrap_or_else(|| json!({}));
            let request = params.get("request").cloned().unwrap_or_else(|| json!({}));
            let event = NetworkEvent {
                id: format!("req_{}", self.request_counter + 1),
                method: value_str(request.get("method")).to_string(),
                url: value_str(response.get("url")).to_string(),
                status: response.get("status").and_then(Value::as_i64),
                content_type: response
                    .get("mimeType")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                resource_type: value_str(params.get("type")).to_string(),
                timestamp: now_millis(),
                ok: response
                    .get("status")
                    .and_then(Value::as_i64)
                    .map(|status| (200..400).contains(&status))
                    .unwrap_or(false),
                error_text: None,
                duration_ms: None,
            };
            self.request_counter += 1;
            self.push_network_event(session_id, &tab_id, event);
        } else if method == "Network.loadingFailed" {
            let event = NetworkEvent {
                id: format!("req_{}", self.request_counter + 1),
                method: String::new(),
                url: String::new(),
                status: None,
                content_type: None,
                resource_type: value_str(params.get("type")).to_string(),
                timestamp: now_millis(),
                ok: false,
                error_text: params
                    .get("errorText")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                duration_ms: None,
            };
            self.request_counter += 1;
            self.push_network_event(session_id, &tab_id, event);
        }
    }

    fn tab_id_by_cdp_session(&self, session_id: &str, cdp_session_id: &str) -> Option<String> {
        self.sessions.get(session_id).and_then(|session| {
            session
                .tabs
                .iter()
                .find(|(_, tab)| tab.session_id == cdp_session_id)
                .map(|(tab_id, _)| tab_id.clone())
        })
    }

    fn push_network_event(&mut self, session_id: &str, tab_id: &str, event: NetworkEvent) {
        if let Some(tab) = self
            .sessions
            .get_mut(session_id)
            .and_then(|session| session.tabs.get_mut(tab_id))
        {
            tab.network_events.push(event);
            if tab.network_events.len() > MAX_NETWORK_EVENTS {
                let remove = tab.network_events.len() - MAX_NETWORK_EVENTS;
                tab.network_events.drain(0..remove);
            }
        }
    }
}
