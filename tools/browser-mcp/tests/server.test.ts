import { describe, expect, it, vi } from 'vitest';
import { createBrowserMcpServer } from '../src/server.js';

describe('createBrowserMcpServer', () => {
  it('registers the full browser tool surface on the real MCP server', () => {
    const runtime = {
      setLogEmitter: vi.fn(),
    };

    const server = createBrowserMcpServer(runtime as never);
    const registeredTools = Object.keys((server as unknown as { _registeredTools: Record<string, unknown> })._registeredTools);

    expect(registeredTools).toEqual([
      'browser_open',
      'browser_tabs',
      'browser_close',
      'browser_get_state',
      'browser_get_elements',
      'browser_get_text',
      'browser_get_network',
      'browser_screenshot',
      'browser_click',
      'browser_fill',
      'browser_press',
      'browser_wait_for',
      'browser_save_session',
      'browser_restore_session',
      'browser_get_attached_runtime_events',
      'browser_diagnostics',
    ]);
    expect(runtime.setLogEmitter).toHaveBeenCalledTimes(1);
  });

  it('keeps the public server identity aligned with the packaged browser-mcp surface', () => {
    const runtime = {
      setLogEmitter: vi.fn(),
    };

    const server = createBrowserMcpServer(runtime as never);
    const info = (server as unknown as { server: { _serverInfo: { name: string; version: string } } }).server._serverInfo;
    const tools = (server as unknown as { _registeredTools: Record<string, { description?: string }> })._registeredTools;

    expect(info).toEqual({ name: 'browser-mcp', version: '0.2.0' });
    expect(tools.browser_screenshot?.description).toContain('inline image');
    expect(tools.browser_restore_session?.description).toContain('Restore a previously saved browser session');
    expect(tools.browser_get_attached_runtime_events?.description).toContain('Replay runtime events');
  });
});
