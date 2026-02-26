import type {DocsPage} from '../types';

/**
 * MCP HTTP Server page model.
 *
 * This page covers MCP HTTP server lifecycle controls and client connection setup.
 */
export const mcpHttpServerPage: DocsPage = {
    path: '/docs/learn/mcp-http-server',
    title: 'MCP HTTP Server',
    summary: 'Start the local MCP HTTP server, verify endpoint details, and configure clients to connect reliably.',
    learnBullets: [
        'Start and stop the server from the TUI control view.',
        'Use active endpoint and client count details for verification.',
        'Configure MCP clients with the running `/mcp` endpoint.',
        'Use auto-start when you need server availability on TUI launch.',
    ],
    estimatedTime: '8-12 min',
    feedbackPrompt: 'Was this page helpful? Rate it or suggest improvements in docs feedback.',
    sections: [
        {
            id: 'prerequisites',
            title: 'Prerequisites',
            paragraphs: ['Open MCP HTTP Server view in the TUI.', 'Confirm local network policy allows loopback access.'],
            callouts: [
                {type: 'expected', content: 'Server controls and status details are visible.'},
                {
                    type: 'recovery',
                    content: 'If the view is unavailable, switch to MCP HTTP Server from left navigation.'
                },
                {
                    type: 'screenshot',
                    label: 'Oatty MCP Server View',
                    imageSrc: '/Oatty-mcp-server-view.png',
                    imageAlt: 'MCP server screenshot',
                    content: 'Capture MCP HTTP Server panel with status and controls.'
                },
            ],
        },
        {
            id: 'why-http-server-not-headless-stdio',
            title: 'Why Oatty Uses a Visible HTTP MCP Server (Not Headless stdio by Default)',
            tocTitle: 'Why HTTP (vs stdio)',
            paragraphs: [
                'Many MCP servers run as headless stdio processes. Oatty intentionally defaults to a visible local HTTP server in the TUI so operators can observe and control runtime behavior directly.',
                'The MCP HTTP Server view provides explicit lifecycle controls, endpoint visibility, connected-client counts, and last-error context. This makes troubleshooting and operational review faster than debugging an opaque background process.',
                'This choice also keeps human-in-the-loop workflows clear: agents can propose and run through MCP while operators keep direct visibility into what is connected and when execution is active.',
                'Headless stdio is still a valid pattern in some environments. If you need tightly sandboxed single-client process wiring, stdio-based integration can still fit, but Oatty\'s default prioritizes transparent multi-client local operations.',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'Operators can inspect server state and client activity in real time instead of inferring failures from disconnected client logs.',
                },
                {
                    type: 'tip',
                    content: 'Use local loopback bind plus visible controls for development and review workflows; only expose network endpoints when you have explicit security controls in place.',
                },
            ],
        },
        {
            id: 'start-stop-server',
            title: 'Start and Stop the Server',
            paragraphs: ['Use Start to launch the MCP HTTP server.', 'Use Stop to shut down the server when needed.', 'Read status changes to confirm lifecycle transitions.'],
            callouts: [
                {
                    type: 'expected',
                    content: 'Status transitions between Stopped, Starting, Running, and Stopping as actions execute.'
                },
                {type: 'recovery', content: 'If start fails, review Last error and retry from a stopped state.'},
                {
                    type: 'screenshot',
                    label: 'Oatty MCP Server Started',
                    imageSrc: '/Oatty-mcp-server-started.png',
                    imageAlt: 'MCP server view screenshot',
                    content: 'Start action, Running status, and Stop control state.'
                },
            ],
        },
        {
            id: 'endpoint-details',
            title: 'Verify Endpoint Details',
            paragraphs: [
                'Read Configured bind and Active endpoint in the details panel.',
                'Use Active endpoint as the canonical client connection target while running.',
                'Monitor Connected clients to confirm successful inbound sessions.',
            ],
            callouts: [
                {type: 'expected', content: 'Active endpoint displays `http://<bound-address>/mcp` while running.'},
                {
                    type: 'recovery',
                    content: 'If Active endpoint is `not running`, start the server and verify status returns Running.'
                },
                {
                    type: 'advanced',
                    content: 'Default bind is loopback (`127.0.0.1:62889`) unless overridden by config.'
                },
            ],
        },
        {
            id: 'configure-clients',
            title: 'Configure MCP Clients to Connect',
            paragraphs: [
                'Use `http://localhost:62889/mcp` as the server URL.',
                'When bound to localhost, the Oatty MCP HTTP server uses local HTTP and typically does not require authentication headers or tokens.',
                'Keep the server bound to loopback unless you understand the security implications of exposing it on your network.',
                'Restart or reconnect the client after updating configuration so it loads the new endpoint.',
            ],
            codeSample: `# Shared connection settings
URL: http://localhost:62889/mcp
Auth: none (localhost)

# Claude Desktop (claude_desktop_config.json)
{
  "mcpServers": {
    "oatty": {
      "url": "http://localhost:62889/mcp"
    }
  }
}

# Cursor (.cursor/mcp.json)
{
  "mcpServers": {
    "oatty": {
      "url": "http://localhost:62889/mcp"
    }
  }
}

# Cline / Roo Code (mcp_settings.json)
{
  "mcpServers": {
    "oatty": {
      "url": "http://localhost:62889/mcp"
    }
  }
}

# VS Code MCP config (.vscode/mcp.json or user MCP config)
{
  "servers": {
    "oatty": {
      "type": "http",
      "url": "http://localhost:62889/mcp"
    }
  }
}

# Generic Streamable HTTP MCP client
{
  "servers": {
    "oatty": {
      "transport": "streamable-http",
      "url": "http://localhost:62889/mcp"
    }
  }
}`,
            callouts: [
                {type: 'expected', content: 'Connected clients count increases after client connection succeeds.'},
                {
                    type: 'recovery',
                    content: 'If clients cannot connect, verify server is Running and the configured URL is exactly `http://localhost:62889/mcp`.'
                },
                {
                    type: 'advanced',
                    content: 'Use local bind addresses for local clients. Keep endpoint and client config synchronized when bind settings change.'
                },
            ],
        },
        {
            id: 'auto-start',
            title: 'Configure Auto-start',
            paragraphs: ['Toggle Auto-start when you want the server started with TUI launch.', 'Leave Auto-start disabled for manual lifecycle control.'],
            callouts: [
                {type: 'expected', content: 'Auto-start toggle state persists and reflects your runtime preference.'},
                {
                    type: 'recovery',
                    content: 'If toggle does not persist, retry toggle and confirm no configuration write errors are logged.'
                },
                {
                    type: 'screenshot',
                    label: 'Oatty MCP Server Auto-start',
                    imageSrc: '/Oatty-mcp-server-autostart.png',
                    imageAlt: 'MCP server screenshot',
                    content: 'Capture Auto-start enabled and disabled states.'
                },
            ],
        },
        {
            id: 'next-steps',
            title: 'Next Steps',
            paragraphs: ['Return to Plugins to validate tool-level integrations exposed through connected clients.', 'Continue to Reference docs for configuration and environment variable details.'],
            callouts: [
                {
                    type: 'expected',
                    content: 'You can run a stable local MCP server and connect clients without ambiguity.'
                },
            ],
        },
    ],
};
