//! PIDX MCP Server — entry point.
//!
//! Spawns a stdio MCP server that wraps the `pidx` library as MCP tools.
//! The client (Claude, Cursor, VS Code Copilot, etc.) launches this binary as
//! a subprocess and speaks JSON-RPC over stdin/stdout.
//!
//! ## Why stderr for logging?
//!
//! MCP stdio transport uses stdout exclusively for protocol messages. Any bytes
//! written to stdout that aren't valid JSON-RPC will break the client. We wire
//! tracing to stderr so logs are still visible without polluting the protocol channel.

mod tools;

use rust_mcp_sdk::{
    error::SdkResult,
    mcp_server::{server_runtime, McpServerOptions},
    schema::{
        Implementation, InitializeResult, ProtocolVersion, ServerCapabilities,
        ServerCapabilitiesTools,
    },
    McpServer, StdioTransport, ToMcpServerHandler, TransportOptions,
};

use tools::PidxHandler;

#[tokio::main]
async fn main() -> SdkResult<()> {
    // Tracing to stderr — stdout is reserved for JSON-RPC
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    tracing::info!(
        profiles_dir = %pidx::storage::ProfileStore::default_dir().display(),
        "pidx-mcp starting"
    );

    let server_info = InitializeResult {
        server_info: Implementation {
            name: "pidx-mcp".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            title: Some("PIDX Personality Indexer".into()),
            description: Some(
                "Read and manage personality profiles for AI orientation.".into(),
            ),
            icons: vec![],
            website_url: None,
        },
        capabilities: ServerCapabilities {
            tools: Some(ServerCapabilitiesTools { list_changed: None }),
            ..Default::default()
        },
        protocol_version: ProtocolVersion::V2025_11_25.into(),
        instructions: Some(concat!(
            "PIDX stores personality profiles built from structured observations. ",
            "Each observation has a status (proposed/confirmed/delta/archived) and decays over time.\n\n",
            "READ tools:\n",
            "  pidx_list   — enumerate all profiles (user IDs, confidence, last-updated)\n",
            "  pidx_show   — fetch a tier-scaled context block (tier: T1/T2/T3)\n",
            "  pidx_status — per-field observation counts + open delta and review queue sizes\n",
            "  pidx_delta_list  — list unresolved delta conflicts with both candidate values\n",
            "  pidx_review_list — list observations flagged by the decay pass\n",
            "  pidx_diff   — structured comparison of two profiles\n\n",
            "WRITE tools:\n",
            "  pidx_ingest  — ingest a .bridge.json packet (path must be absolute)\n",
            "  pidx_confirm — flip one proposed observation to confirmed (field + index)\n",
            "  pidx_reject  — reject one proposed observation\n",
            "  pidx_confirm_all — bulk-confirm all proposed obs under a dot-path prefix\n\n",
            "LIFECYCLE tools:\n",
            "  pidx_delta_resolve  — resolve a delta conflict by choosing side a or b\n",
            "  pidx_review_process — solidify (decay-exempt) or discard a review item\n",
            "  pidx_annotate — attach a text note to a field (pinned notes appear in T3 output)\n",
            "  pidx_decay    — run a decay pass; flags low-confidence obs to the review queue\n\n",
            "Workflow: ingest → status → confirm_all → show. ",
            "Use delta_list/resolve when status shows open deltas. ",
            "Use decay + review_list/process during periodic maintenance."
        ).into()),
        meta: None,
    };

    let transport = StdioTransport::new(TransportOptions::default())?;
    let handler = PidxHandler::new().to_mcp_server_handler();

    let server = server_runtime::create_server(McpServerOptions {
        server_details: server_info,
        transport,
        handler,
        task_store: None,
        client_task_store: None,
        message_observer: None,
    });

    server.start().await
}
