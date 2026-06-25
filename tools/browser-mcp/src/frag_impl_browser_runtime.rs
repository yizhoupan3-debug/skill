// `impl BrowserRuntime`（与 `frag_01_through_types.rs` 中类型同模块拼接）。

impl BrowserRuntime {
    #[cfg(test)]
    fn new(repo_root: PathBuf) -> Self {
        // Initialize no-op hooks for test isolation
        static INIT_HOOKS: std::sync::Once = std::sync::Once::new();
        INIT_HOOKS.call_once(|| {
            browser_mcp_dispatch::set_hooks(browser_mcp_dispatch::BrowserMcpHooks {
                evaluate_mcp_pre_guard: |_, _, _| {
                    browser_mcp_dispatch::McpPreGuardVerdict {
                        blocked: false,
                        reason: None,
                    }
                },
                attach_runtime_event_transport: |_| {
                    Err("no runtime hooks in test".to_string())
                },
                inspect_trace_stream: |_| {
                    Err("no runtime hooks in test".to_string())
                },
            });
        });
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



    fn open(&mut self, input: &Value) -> Result<Value, Value> {
        let url = required_string_arg(input, "url")?;
        rt_core_contracts::web_fetch_guard::validate_browser_open_url(&url)
            .map_err(|e| runtime_error("SSRF_BLOCKED", &e))?;
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
            self.dispose_session(&session_id);
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
            session.cdp_session_to_tab.retain(|_, v| v != &tab_id);
            session.current_tab_id = session.tabs.keys().next().cloned();
            let remaining = session.tabs.len();
            if remaining == 0 {
                self.dispose_session(&session_id);
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
        let valid_includes = ["summary", "interactive_elements", "diff"];
        let include: Vec<_> = include.into_iter().filter(|v| valid_includes.contains(&v.as_str())).collect();
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
        // Purge old screenshots: keep at most 50 most recent files.
        const MAX_SCREENSHOTS: usize = 50;
        purge_old_screenshots(&screenshot_dir, MAX_SCREENSHOTS);
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
                "savedAt": framework_kernel::time::current_local_timestamp(),
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
            json!({"ok": true, "path": session_path.to_string_lossy(), "savedAt": framework_kernel::time::current_local_timestamp()}),
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


    fn diagnostics(&mut self, _input: &Value) -> Result<Value, Value> {
        let mut tabs = 0usize;
        let mut network_events = 0usize;
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
            Ok(resolved) => match (browser_mcp_dispatch::hooks().inspect_trace_stream)(TraceStreamInspectRequestPayload {
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

    fn get_or_create_session(&mut self) -> Result<String, Value> {
        if let Some(session_id) = self.sessions.keys().next().cloned() {
            return Ok(session_id);
        }
        let chrome_path = find_chrome_binary()?;
        let (port, _keep_alive) = allocate_debug_port()?;
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

        // 确保 wait_for_cdp / CdpClient::connect 失败时清理 Chrome 进程和临时目录
        let cleanup = CleanupGuard {
            child: Some(child),
            user_data_dir: &user_data_dir,
        };

        wait_for_cdp(port)?;
        let cdp = CdpClient::connect(port)?;

        // 所有前置操作成功后才注册到 session 状态
        let mut cleanup = std::mem::ManuallyDrop::new(cleanup);
        self.browser_processes
            .insert(session_id.clone(), cleanup.child.take().unwrap());
        self.sessions.insert(
            session_id.clone(),
            SessionRecord {
                id: session_id.clone(),
                created_at: framework_kernel::time::current_local_timestamp(),
                viewport: ViewportSize {
                    width: 1440,
                    height: 900,
                },
                current_tab_id: None,
                tabs: HashMap::new(),
                cdp_session_to_tab: HashMap::new(),
                user_data_dir,
                cdp,
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
                    session_id: session_cdp_id.clone(),
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
            // O(1) CDP session → tab lookup
            session.cdp_session_to_tab.insert(session_cdp_id.clone(), tab_id.clone());
            session.current_tab_id = Some(tab_id.clone());
        }
        Ok(tab_id)
    }

    fn dispose_session(&mut self, session_id: &str) {
        if let Some(mut child) = self.browser_processes.remove(session_id) {
            if let Err(e) = child.kill() {
                tracing::warn!("failed to kill browser process for session {session_id}: {e}");
            }
            if let Err(e) = child.wait() {
                tracing::warn!("failed to wait on browser process for session {session_id}: {e}");
            }
        }
        if let Some(session) = self.sessions.remove(session_id)
            && let Err(e) = fs::remove_dir_all(session.user_data_dir) {
                tracing::warn!("failed to remove user data dir for session {session_id}: {e}");
            }
    }

    fn shutdown(&mut self) -> Result<(), Value> {
        let ids = self.sessions.keys().cloned().collect::<Vec<_>>();
        for session_id in ids {
            self.dispose_session(&session_id);
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
        let mut delay_ms = 50u64;
        while SystemTime::now() < deadline {
            self.drain_cdp_events(session_id, delay_ms)?;
            let state = self.evaluate_string(session_id, tab_id, "document.readyState")?;
            if state == "complete" || state == "interactive" {
                self.drain_cdp_events(session_id, 250)?;
                return Ok(());
            }
            // Exponential backoff: 50ms → 100ms → 200ms → 400ms → cap at 500ms
            delay_ms = (delay_ms * 2).min(500);
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
        let mut snapshot = self.capture_snapshot(session_id, tab_id, &previous_ref_map)?;
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
                snapshot.revision = tab.page_revision;
                for element in &mut snapshot.interactive_elements {
                    element.page_revision = tab.page_revision;
                }
                tab.url = snapshot.url.clone();
                tab.title = snapshot.title.clone();
                tab.loading_state = snapshot.loading_state.clone();
                // 唯一必要的 clone：存入 snapshot_history（保留最近 8 个快照用于 diff）
                tab.snapshot_history.push_back(snapshot.clone());
                while tab.snapshot_history.len() > SNAPSHOT_HISTORY_LIMIT {
                    tab.snapshot_history.pop_front();
                }
                tab.last_snapshot = Some(snapshot);
            } else {
                snapshot = tab.last_snapshot.clone().unwrap_or(snapshot);
                tab.url = snapshot.url.clone();
                tab.title = snapshot.title.clone();
                tab.loading_state = snapshot.loading_state.clone();
            }
            // 从 tab.last_snapshot 读取（无论 changed 与否，都已是最新的）
            if let Some(ref effective) = tab.last_snapshot {
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
        }
        // 返回最新快照（从 tab 中读取，避免 snapshot 已被 move 的问题）
        Ok(self
            .sessions
            .get(session_id)
            .and_then(|s| s.tabs.get(tab_id))
            .and_then(|t| t.last_snapshot.clone())
            .unwrap_or(PageSnapshot {
                revision: 0,
                url: String::new(),
                title: String::new(),
                loading_state: "idle".to_string(),
                summary: json!({}),
                interactive_elements: Vec::new(),
                text_content: String::new(),
                text_lines: Vec::new(),
            }))
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
        let items = payload.as_array().map(Vec::as_slice).unwrap_or(&[]);
        let mut descriptors = Vec::with_capacity(items.len());
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
        for event in &events {
            self.handle_cdp_event(session_id, event);
        }
        Ok(())
    }

    fn handle_cdp_event(&mut self, session_id: &str, event: &Value) {
        let method = event.get("method").and_then(Value::as_str).unwrap_or("");
        let cdp_session_id = event.get("sessionId").and_then(Value::as_str).unwrap_or("");
        let Some(tab_id) = self.tab_id_by_cdp_session(session_id, cdp_session_id) else {
            return;
        };
        let params = match event.get("params") {
            Some(p) => p,
            None => return,
        };
        if method == "Network.responseReceived" {
            let response = params.get("response").unwrap_or(&Value::Null);
            let request = params.get("request").unwrap_or(&Value::Null);
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
        self.sessions
            .get(session_id)
            .and_then(|session| session.cdp_session_to_tab.get(cdp_session_id))
            .cloned()
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
