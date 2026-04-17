document.addEventListener('DOMContentLoaded', () => {
    'use strict';

    const state = {
        frozenDashboard: null,
        frozenJournal: [],
        accounts: [],
        isRefreshingAccounts: false,
        lastAccountRefreshAt: null,
        currentView: 'dashboard',
    };

    const elements = {
        navItems: document.querySelectorAll('nav li'),
        views: document.querySelectorAll('.view'),
        refreshBtn: document.getElementById('refresh-btn'),
        syncBtn: document.getElementById('sync-btn'),
        auditBtn: document.getElementById('run-audit-btn'),
        auditLog: document.getElementById('audit-log'),
        accountContainer: document.getElementById('accounts-container'),
        accountRefreshStatus: document.getElementById('account-refresh-status'),
        snapshotState: document.getElementById('snapshot-state'),
        currentTs: document.getElementById('current-ts'),
        systemStatus: document.getElementById('system-status'),
        dashboardSnapshotTs: document.getElementById('dashboard-snapshot-ts'),
        totalUsage: document.getElementById('total-usage'),
        totalSkills: document.getElementById('total-skills'),
        criticalSkills: document.getElementById('critical-skills'),
        globalHealthValue: document.getElementById('global-health-val'),
        outliersList: document.getElementById('outliers-list'),
        recentJournal: document.getElementById('recent-journal'),
        journalFeed: document.getElementById('journal-feed'),
        skillTableBody: document.getElementById('skill-table-body'),
        skillSearch: document.getElementById('skill-search'),
        addAccountBtn: document.getElementById('add-account-btn'),
        mainApiName: document.getElementById('main-api-name'),
        mainApiUrl: document.getElementById('main-api-url'),
    };

    /**
     * Escape a string for safe HTML insertion.
     *
     * @param {unknown} value - Raw value to escape.
     * @returns {string} Escaped text safe for innerHTML usage.
     */
    function escapeHtml(value) {
        return String(value ?? '')
            .replaceAll('&', '&amp;')
            .replaceAll('<', '&lt;')
            .replaceAll('>', '&gt;')
            .replaceAll('"', '&quot;')
            .replaceAll("'", '&#39;');
    }

    /**
     * Convert any value into a finite number.
     *
     * @param {unknown} value - Value to normalize.
     * @param {number} fallback - Value to use when parsing fails.
     * @returns {number} Parsed finite number or fallback.
     */
    function toFiniteNumber(value, fallback = 0) {
        const parsed = Number(value);
        return Number.isFinite(parsed) ? parsed : fallback;
    }

    /**
     * Format a value as a locale-aware number.
     *
     * @param {unknown} value - Value to format.
     * @returns {string} Locale formatted number string.
     */
    function formatNumber(value) {
        return toFiniteNumber(value, 0).toLocaleString('zh-CN');
    }

    /**
     * Format a timestamp-like value for display.
     *
     * @param {unknown} value - Timestamp in ISO, epoch, or date-like format.
     * @returns {string} Human-friendly timestamp string.
     */
    function formatTimestamp(value) {
        if (!value) {
            return '—';
        }

        const parsed = new Date(value);
        if (Number.isNaN(parsed.getTime())) {
            return String(value);
        }

        return parsed.toLocaleString('zh-CN', {
            year: 'numeric',
            month: '2-digit',
            day: '2-digit',
            hour: '2-digit',
            minute: '2-digit',
            second: '2-digit',
        });
    }

    /**
     * Derive a readable token mask without exposing raw secrets.
     *
     * @param {Record<string, unknown>} account - Account payload from the API.
     * @returns {string} Safe token mask for display.
     */
    function getTokenMask(account) {
        const candidates = [
            account.token_preview,
            account.token_mask,
            account.masked_token,
            account.token_display,
            account.api_key_mask,
            account.secret_mask,
            account.token?.masked_preview,
        ];

        const found = candidates.find((value) => typeof value === 'string' && value.trim());
        if (found) {
            return found;
        }

        return '••••••••';
    }

    /**
     * Normalize the token/account status into a badge label and CSS class.
     *
     * @param {Record<string, unknown>} account - Account payload from the API.
     * @returns {{ label: string, className: string }} Visual status metadata.
     */
    function getTokenStatusMeta(account) {
        const rawStatus = String(
            account.token_status ?? account.token?.status ?? account.status ?? account.state ?? 'unknown',
        ).toLowerCase();

        const statusMap = {
            active: { label: '在线', className: 'status-active' },
            ready: { label: '在线', className: 'status-active' },
            healthy: { label: '健康', className: 'status-active' },
            red: { label: '异常', className: 'status-red' },
            offline: { label: '离线', className: 'status-red' },
            expired: { label: '过期', className: 'status-red' },
            revoked: { label: '已撤销', className: 'status-red' },
            paused: { label: '暂停', className: 'status-warning' },
            warning: { label: '警告', className: 'status-warning' },
            stable: { label: '稳定', className: 'status-stable' },
        };

        return statusMap[rawStatus] ?? { label: rawStatus === 'unknown' ? '未知' : rawStatus, className: 'status-stable' };
    }

    /**
     * Normalize the alert message for a token card.
     *
     * @param {Record<string, unknown>} account - Account payload from the API.
     * @returns {string} Safe alert text or fallback.
     */
    function getAlertText(account) {
        const alerts = [
            account.alert_message,
            account.alert,
            account.warning,
            account.note,
            Array.isArray(account.warnings) ? account.warnings[0] : null,
        ];

        const list = alerts.filter((value) => typeof value === 'string' && value.trim());
        if (list.length > 0) {
            return list[0];
        }

        if (String(account.status ?? '').toLowerCase() !== 'active' && String(account.status ?? '').toLowerCase() !== 'healthy') {
            return '需要关注 token 状态';
        }

        return '—';
    }

    /**
     * Normalize one account payload into a render-ready record.
     *
     * @param {Record<string, unknown>} account - Raw account payload.
     * @returns {Record<string, unknown>} Normalized account record.
     */
    function normalizeAccount(account) {
        const used5h = toFiniteNumber(
            account.used_5h ?? account.used5h ?? account.usage_5h ?? account.usage?.used_5h,
            0,
        );
        const used7d = toFiniteNumber(
            account.used_7d ?? account.used7d ?? account.usage_7d ?? account.usage?.used_7d,
            0,
        );
        const stabilityFromApi = toFiniteNumber(
            account.stability_score ?? account.stability ?? account.token?.stability ?? account.health_score,
            Number.NaN,
        );
        const heuristicStability = Math.max(
            10,
            100 - (String(account.status ?? '').toLowerCase() === 'active' ? 0 : 35) - Math.min(35, used5h / 2) - Math.min(20, used7d / 100),
        );

        return {
            id: account.id ?? account.channel_id ?? account.name ?? 'unknown',
            name: String(account.name ?? 'Unnamed Account'),
            statusMeta: getTokenStatusMeta(account),
            tokenMask: getTokenMask(account),
            refreshedAt: account.refreshed_at ?? account.last_refreshed_at ?? account.token?.last_refreshed_at ?? account.updated_at ?? null,
            stability: Number.isFinite(stabilityFromApi) ? stabilityFromApi : Math.round(heuristicStability),
            alertText: getAlertText(account),
            used5h,
            used7d,
        };
    }

    /**
     * Build a total usage number from the skills payload.
     *
     * @param {Record<string, { usage_30d?: number }>} skills - Skills payload from the API.
     * @returns {number} Total 30-day usage count.
     */
    function computeTotalUsage(skills) {
        return Object.values(skills ?? {}).reduce((sum, skill) => sum + toFiniteNumber(skill?.usage_30d, 0), 0);
    }

    /**
     * Switch the visible section in the sidebar layout.
     *
     * @param {string} viewName - Target view name.
     * @returns {void}
     */
    function switchView(viewName) {
        elements.navItems.forEach((item) => item.classList.remove('active'));
        const targetNav = document.querySelector(`nav li[data-view="${CSS.escape(viewName)}"]`);
        if (targetNav) {
            targetNav.classList.add('active');
        }

        elements.views.forEach((view) => view.classList.remove('active'));
        const targetView = document.getElementById(`${viewName}-view`);
        if (targetView) {
            targetView.classList.add('active');
        }

        state.currentView = viewName;
    }

    /**
     * Render the frozen dashboard snapshot without hitting the network again.
     *
     * @param {Record<string, unknown>} snapshot - Cached dashboard snapshot.
     * @returns {void}
     */
    function renderFrozenDashboard(snapshot) {
        if (!snapshot) {
            return;
        }

        const summary = snapshot.summary ?? {};
        const skills = snapshot.skills ?? {};
        const fallbackTotalUsage = computeTotalUsage(skills);
        const totalUsage = toFiniteNumber(summary.total_usage, fallbackTotalUsage) || fallbackTotalUsage;
        const avgHealth = toFiniteNumber(summary.avg_health, 0);
        const totalSkills = toFiniteNumber(summary.total_skills, Object.keys(skills).length);
        const criticalSkills = toFiniteNumber(summary.critical_skills, toFiniteNumber(snapshot.critical_outliers?.length, 0));
        const snapshotTs = snapshot.ts ?? snapshot.loadedAt ?? new Date().toISOString();

        if (elements.globalHealthValue) {
            elements.globalHealthValue.textContent = `${avgHealth.toFixed(1)}%`;
        }
        if (elements.totalSkills) {
            elements.totalSkills.textContent = formatNumber(totalSkills);
        }
        if (elements.criticalSkills) {
            elements.criticalSkills.textContent = formatNumber(criticalSkills);
        }
        if (elements.totalUsage) {
            elements.totalUsage.textContent = formatNumber(totalUsage);
        }
        if (elements.currentTs) {
            elements.currentTs.textContent = `首页统计快照：${formatTimestamp(snapshotTs)}`;
        }
        if (elements.systemStatus) {
            elements.systemStatus.textContent = '正常运行 · 统计已冻结';
        }
        if (elements.snapshotState) {
            elements.snapshotState.textContent = '首页统计冻结';
        }
        if (elements.dashboardSnapshotTs) {
            elements.dashboardSnapshotTs.textContent = formatTimestamp(snapshotTs);
        }

        const outliers = Array.isArray(snapshot.critical_outliers) ? snapshot.critical_outliers : [];
        if (elements.outliersList) {
            elements.outliersList.innerHTML = outliers.slice(0, 5).map((skill) => `
                <li>
                    <span class="skill-name">${escapeHtml(skill)}</span>
                    <span class="badge badge-critical">紧急修复</span>
                </li>
            `).join('') || '<li class="empty-row">暂无高风险技能</li>';
        }

        const journalEntries = Array.isArray(state.frozenJournal) ? state.frozenJournal : [];
        renderRecentJournal(journalEntries);
        renderSkillsTable(skills);
    }

    /**
     * Render the skills table from the frozen dashboard snapshot.
     *
     * @param {Record<string, { health_status?: string, dynamic_score?: number, usage_30d?: number, reroutes_30d?: number }>} skills - Skills payload.
     * @returns {void}
     */
    function renderSkillsTable(skills) {
        if (!elements.skillTableBody) {
            return;
        }

        const entries = Object.entries(skills ?? {});
        if (entries.length === 0) {
            elements.skillTableBody.innerHTML = '<tr><td colspan="5" class="empty-row">暂无技能数据</td></tr>';
            return;
        }

        elements.skillTableBody.innerHTML = entries.map(([name, info]) => {
            const healthStatus = String(info?.health_status ?? 'Unknown');
            const badgeClass = `badge-${healthStatus.toLowerCase()}`;
            return `
                <tr>
                    <td>${escapeHtml(name)}</td>
                    <td><span class="badge ${badgeClass}">${escapeHtml(healthStatus)}</span></td>
                    <td>${formatNumber(info?.dynamic_score ?? 0)}%</td>
                    <td>${formatNumber(info?.usage_30d ?? 0)}</td>
                    <td>${formatNumber(info?.reroutes_30d ?? 0)}</td>
                </tr>
            `;
        }).join('');
    }

    /**
     * Render the frozen journal snapshot.
     *
     * @param {Array<Record<string, unknown>>} entries - Journal entries.
     * @returns {void}
     */
    function renderRecentJournal(entries) {
        if (elements.recentJournal) {
            const latest = entries.slice(-5).reverse();
            elements.recentJournal.innerHTML = latest.map((entry) => `
                <li>
                    <small>${escapeHtml(formatTimestamp(entry.ts))}</small>
                    <span>${escapeHtml(entry.final ?? 'unknown')}</span>
                    <b>${escapeHtml(entry.init ?? 'unknown')}</b>
                </li>
            `).join('') || '<li class="empty-row">暂无日志快照</li>';
        }

        if (elements.journalFeed) {
            const feedEntries = [...entries].reverse();
            elements.journalFeed.innerHTML = feedEntries.map((entry) => `
                <div class="journal-entry glass">
                    <span class="ts">${escapeHtml(formatTimestamp(entry.ts))}</span>
                    <span class="task">${escapeHtml(entry.task ?? 'unknown task')}</span>
                    <div class="flow"><b>${escapeHtml(entry.init ?? 'unknown')}</b> → <b>${escapeHtml(entry.final ?? 'unknown')}</b> (${formatNumber((entry.conf ?? 0) * 100)}%)</div>
                </div>
            `).join('') || '<div class="empty-state">暂无日志快照</div>';
        }
    }

    /**
     * Render the token pool cards using the latest account payload.
     *
     * @param {Array<Record<string, unknown>>} accounts - Raw account payloads.
     * @returns {void}
     */
    function renderAccounts(accounts) {
        if (!elements.accountContainer) {
            return;
        }

        const normalizedAccounts = accounts.map(normalizeAccount);
        state.accounts = normalizedAccounts;

        if (normalizedAccounts.length === 0) {
            elements.accountContainer.innerHTML = '<div class="empty-state glass">暂无账号数据</div>';
            if (elements.accountRefreshStatus) {
                elements.accountRefreshStatus.textContent = '未找到账号数据';
            }
            return;
        }

        const activeCount = normalizedAccounts.filter((account) => account.statusMeta.className !== 'status-red').length;
        const now = state.lastAccountRefreshAt ? formatTimestamp(state.lastAccountRefreshAt) : formatTimestamp(new Date().toISOString());

        if (elements.accountRefreshStatus) {
            elements.accountRefreshStatus.textContent = `已刷新 ${formatNumber(normalizedAccounts.length)} 个账号 · 活动 ${formatNumber(activeCount)} 个 · ${now}`;
        }

        elements.accountContainer.innerHTML = normalizedAccounts.map((account) => {
            const stabilityWidth = Math.min(Math.max(account.stability, 0), 100);
            const usage5hWidth = Math.min((account.used5h / 40) * 100, 100);
            const usage7dWidth = Math.min((account.used7d / 1000) * 100, 100);
            const statusClass = account.statusMeta.className;
            const statusLabel = account.statusMeta.label;
            const alertClass = account.alertText === '—' ? 'alert-muted' : 'alert-warning';
            return `
                <article class="card glass account-card">
                    <div class="account-header">
                        <div>
                            <div class="account-name">${escapeHtml(account.name)}</div>
                            <div class="account-token-line">
                                <span class="token-label">Token Mask</span>
                                <code class="token-mask">${escapeHtml(account.tokenMask)}</code>
                            </div>
                        </div>
                        <div class="account-status-stack">
                            <span class="status-pill ${statusClass}">${escapeHtml(statusLabel)}</span>
                            <span class="refresh-time">${escapeHtml(formatTimestamp(account.refreshedAt))}</span>
                        </div>
                    </div>

                    <div class="account-meta-grid">
                        <div class="metric-block">
                            <div class="metric-label">稳定性</div>
                            <div class="metric-value metric-value--small">${formatNumber(account.stability)}%</div>
                            <div class="stability-bar"><div class="stability-fill" style="width: ${stabilityWidth}%"></div></div>
                        </div>
                        <div class="metric-block">
                            <div class="metric-label">告警</div>
                            <div class="alert-text ${alertClass}">${escapeHtml(account.alertText)}</div>
                        </div>
                    </div>

                    <div class="usage-grid">
                        <div class="usage-item">
                            <span class="metric-label">5h 请求数</span>
                            <strong>${formatNumber(account.used5h)}</strong>
                            <div class="progress-bar"><div class="progress-fill" style="width: ${usage5hWidth}%"></div></div>
                        </div>
                        <div class="usage-item">
                            <span class="metric-label">7d 请求数</span>
                            <strong>${formatNumber(account.used7d)}</strong>
                            <div class="progress-bar"><div class="progress-fill" style="width: ${usage7dWidth}%"></div></div>
                        </div>
                    </div>
                </article>
            `;
        }).join('');
    }

    /**
     * Render the main API probe card and account payload together.
     *
     * @param {Record<string, unknown>} payload - Accounts response payload.
     * @returns {void}
     */
    function renderAccountsPayload(payload) {
        const mainApi = payload?.main_api ?? {};
        const accounts = Array.isArray(payload?.accounts) ? payload.accounts : [];
        const summary = payload?.summary ?? {};
        const fetchedAt = payload?.fetched_at ?? state.lastAccountRefreshAt ?? new Date().toISOString();

        if (elements.mainApiName) {
            elements.mainApiName.textContent = String(mainApi.name ?? 'Codex API');
        }
        if (elements.mainApiUrl) {
            elements.mainApiUrl.textContent = String(mainApi.url ?? mainApi.base_url ?? '未配置');
        }

        renderAccounts(accounts);

        if (elements.accountRefreshStatus) {
            const statusText = String(mainApi.status ?? 'unconfigured');
            const totalAccounts = formatNumber(summary.total_accounts ?? accounts.length);
            elements.accountRefreshStatus.textContent = `Main API: ${statusText} · 账号 ${totalAccounts} 个 · ${formatTimestamp(fetchedAt)}`;
        }
    }

    /**
     * Load the latest masked accounts snapshot.
     *
     * @returns {Promise<void>} Resolves after the account payload is rendered.
     */
    async function loadAccountsSnapshot() {
        const res = await fetch('/api/accounts', { cache: 'no-store' });
        const data = await res.json();
        state.lastAccountRefreshAt = data?.fetched_at ?? new Date().toISOString();
        renderAccountsPayload(data);
    }

    /**
     * Load the frozen dashboard snapshot once per page session.
     *
     * @returns {Promise<void>} Resolves after the snapshot is rendered.
     */
    async function loadFrozenDashboard() {
        const [healthRes, journalRes] = await Promise.all([
            fetch('/api/health', { cache: 'no-store' }),
            fetch('/api/journal?limit=100', { cache: 'no-store' }),
        ]);

        const healthData = await healthRes.json();
        const journalData = await journalRes.json();

        state.frozenDashboard = healthData;
        state.frozenJournal = Array.isArray(journalData) ? journalData : [];
        renderFrozenDashboard(healthData);
    }

    /**
     * Replay the cached frozen dashboard without issuing any network requests.
     *
     * @returns {void}
     */
    function replayFrozenDashboard() {
        if (state.frozenDashboard) {
            renderFrozenDashboard(state.frozenDashboard);
        }
    }

    /**
     * Refresh the token pool and keep the dashboard snapshot untouched.
     *
     * @returns {Promise<void>} Resolves after the account cards are updated.
     */
    async function refreshTokenPool() {
        if (state.isRefreshingAccounts) {
            return;
        }

        state.isRefreshingAccounts = true;
        if (elements.refreshBtn) {
            elements.refreshBtn.disabled = true;
            elements.refreshBtn.textContent = '刷新中...';
        }

        try {
            const refreshRes = await fetch('/api/accounts/refresh', {
                method: 'POST',
                cache: 'no-store',
                headers: {
                    'Content-Type': 'application/json',
                    'Cache-Control': 'no-cache',
                    Pragma: 'no-cache',
                },
                body: JSON.stringify({ reason: 'manual_ui_refresh' }),
            });
            await refreshRes.json();
            await loadAccountsSnapshot();
        } catch (error) {
            console.error('Accounts fetch failed', error);
            if (elements.accountRefreshStatus) {
                elements.accountRefreshStatus.textContent = '账号刷新失败，请检查后端 /api/accounts 契约';
            }
        } finally {
            state.isRefreshingAccounts = false;
            if (elements.refreshBtn) {
                elements.refreshBtn.disabled = false;
                elements.refreshBtn.textContent = '刷新 Token 池';
            }
        }
    }

    /**
     * Run the audit endpoint and print the returned report.
     *
     * @returns {Promise<void>} Resolves after the audit view is updated.
     */
    async function runAudit() {
        if (!elements.auditLog) {
            return;
        }

        elements.auditLog.innerText = '正在执行深度审计分析...';
        try {
            const res = await fetch('/api/audit', { cache: 'no-store' });
            const data = await res.json();
            elements.auditLog.innerText = JSON.stringify(data.repair_suggestions ?? data, null, 2);
        } catch (error) {
            elements.auditLog.innerText = `审计失败: ${error}`;
        }
    }

    /**
     * Open the add-account affordance without exposing secrets in the UI.
     *
     * @returns {void} Shows a safe placeholder message.
     */
    function handleAddAccountClick() {
        window.alert('新增账号入口暂未接入；请通过后端 token 池接口维护账号，并避免在前端展示原始 token。');
    }

    /**
     * Handle the refresh button click and refresh only the token pool.
     *
     * @returns {Promise<void>} Resolves after the token pool refresh completes.
     */
    async function handleRefreshClick() {
        await loadAccountsSnapshot();
    }

    /**
     * Handle the sync button click by replaying the cached dashboard snapshot.
     *
     * @returns {void} Re-renders the frozen dashboard snapshot.
     */
    function handleSyncClick() {
        replayFrozenDashboard();
    }

    /**
     * Handle sidebar navigation click events.
     *
     * @param {MouseEvent} event - Click event.
     * @returns {void}
     */
    function handleNavigationClick(event) {
        const target = event.currentTarget;
        const viewName = target?.getAttribute('data-view');
        if (viewName) {
            switchView(viewName);
        }
    }

    /**
     * Handle skill table search input with a simple client-side filter.
     *
     * @returns {void} Filters the current visible skill rows.
     */
    function handleSkillSearch() {
        if (!elements.skillSearch || !elements.skillTableBody) {
            return;
        }

        const keyword = elements.skillSearch.value.trim().toLowerCase();
        elements.skillTableBody.querySelectorAll('tr').forEach((row) => {
            const text = row.textContent?.toLowerCase() ?? '';
            row.style.display = text.includes(keyword) ? '' : 'none';
        });
    }

    /**
     * Bind all DOM events used by the dashboard.
     *
     * @returns {void} Attaches event listeners.
     */
    function bindEvents() {
        elements.navItems.forEach((item) => item.addEventListener('click', handleNavigationClick));
        elements.refreshBtn?.addEventListener('click', handleRefreshClick);
        elements.syncBtn?.addEventListener('click', handleSyncClick);
        elements.auditBtn?.addEventListener('click', runAudit);
        elements.addAccountBtn?.addEventListener('click', handleAddAccountClick);
        elements.skillSearch?.addEventListener('input', handleSkillSearch);
    }

    /**
     * Initialize the dashboard by loading the frozen snapshot and token pool.
     *
     * @returns {Promise<void>} Resolves when the bootstrap sequence finishes.
     */
    async function initializeDashboard() {
        bindEvents();
        if (elements.refreshBtn) {
            elements.refreshBtn.textContent = '刷新 Token 池';
        }
        if (elements.syncBtn) {
            elements.syncBtn.textContent = '重绘快照';
        }
        if (elements.snapshotState) {
            elements.snapshotState.textContent = '首页统计冻结';
        }

        try {
            await loadFrozenDashboard();
        } catch (error) {
            console.error('Failed to load frozen dashboard', error);
            if (elements.currentTs) {
                elements.currentTs.textContent = '首页统计快照加载失败';
            }
        }

        await loadAccountsSnapshot();
    }

    initializeDashboard();
});
