import { createServer } from 'node:http';
import { parseArgs } from 'node:util';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { StreamableHTTPServerTransport } from '@modelcontextprotocol/sdk/server/streamableHttp.js';
import { createBrowserMcpServer } from './server.js';
import { BrowserRuntime } from './runtime.js';

// ---------------------------------------------------------------------------
// CLI argument parsing
// ---------------------------------------------------------------------------

const { values: args } = parseArgs({
  options: {
    transport: { type: 'string', default: 'stdio' },
    port: { type: 'string', default: '3721' },
    headless: { type: 'string', default: 'true' },
    engine: { type: 'string', default: 'chromium' },
    'capture-body': { type: 'boolean', default: false },
  },
  strict: false,
});

const transport = String(args['transport'] ?? 'stdio');
const port = parseInt(String(args['port'] ?? '3721'), 10);
const headless = String(args['headless'] ?? 'true') !== 'false';
const engine = String(args['engine'] ?? 'chromium') as 'chromium' | 'firefox' | 'webkit';
const captureBody = Boolean(args['capture-body']);

// ---------------------------------------------------------------------------
// Server startup
// ---------------------------------------------------------------------------

/**
 * Starts the browser-mcp server over stdio or HTTP (Streamable HTTP transport).
 */
async function main(): Promise<void> {
  const runtime = new BrowserRuntime({
    headless,
    browserEngine: engine,
    captureBody,
  });

  const server = createBrowserMcpServer(runtime);

  const shutdown = async (): Promise<void> => {
    await runtime.shutdown();
  };

  process.on('SIGINT', () => void shutdown().finally(() => process.exit(0)));
  process.on('SIGTERM', () => void shutdown().finally(() => process.exit(0)));

  if (transport === 'http') {
    // Streamable HTTP transport — one stateless-capable handler per request
    const httpTransport = new StreamableHTTPServerTransport({ sessionIdGenerator: undefined });
    await server.connect(httpTransport);

    const httpServer = createServer(async (req, res) => {
      await httpTransport.handleRequest(req, res);
    });

    httpServer.listen(port, '0.0.0.0', () => {
      console.error(`browser-mcp HTTP server listening on port ${port}`);
    });

    httpServer.on('error', (err) => {
      console.error('HTTP server error:', err);
      process.exit(1);
    });
  } else {
    // Default: stdio transport
    const stdioTransport = new StdioServerTransport();
    await server.connect(stdioTransport);
    console.error(
      `browser-mcp stdio server running [engine=${engine} headless=${headless} captureBody=${captureBody}]`,
    );
  }
}

void main().catch((error) => {
  console.error('Fatal error in main():', error);
  process.exit(1);
});
