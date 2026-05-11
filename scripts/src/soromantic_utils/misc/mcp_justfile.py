#!/usr/bin/env python3
"""
MCP Server for exposing justfile commands to agents.

Provides tools:
- list_tasks: List available just tasks with descriptions
- run_task: Execute a specific just task

Usage:
    uv run python scripts/mcp_justfile.py
"""

import re
import subprocess
from pathlib import Path
from mcp.server.stdio import stdio_server

from mcp.server import Server
from mcp.types import TextContent, Tool

# Find project root (where justfile is)
SCRIPT_DIR = Path(__file__).parent
PROJECT_ROOT = SCRIPT_DIR.parent


def parse_just_list() -> list[dict]:
    """Parse `just --list` output into structured task info."""
    result = subprocess.run(
        ["just", "--list"],
        cwd=PROJECT_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )

    tasks = []
    for line in result.stdout.strip().split("\n"):
        # Skip header line and empty lines
        if not line.strip() or line.startswith("Available"):
            continue

        # Parse "task_name # description" or just "task_name"
        match = re.match(r"^\s*(\S+)\s*(?:#\s*(.*))?$", line)
        if match:
            name = match.group(1)
            description = match.group(2) or ""
            tasks.append({"name": name, "description": description.strip()})

    return tasks


def run_just_task(task: str, args: list[str] | None = None) -> dict:
    """Run a just task and return the result."""
    cmd = ["just", task]
    if args:
        cmd.extend(args)

    result = subprocess.run(
        cmd,
        cwd=PROJECT_ROOT,
        capture_output=True,
        text=True,
        timeout=300,  # 5 minute timeout
        check=False,
    )

    return {
        "exit_code": result.returncode,
        "stdout": result.stdout,
        "stderr": result.stderr,
        "success": result.returncode == 0,
    }


# Create MCP server
server = Server("justfile")


@server.list_tools()
async def list_tools() -> list[Tool]:
    """List available MCP tools."""
    return [
        Tool(
            name="list_tasks",
            description="List all available just tasks with their descriptions",
            inputSchema={"type": "object", "properties": {}},
        ),
        Tool(
            name="run_task",
            description="Run a specific just task. Use list_tasks first to see available tasks.",
            inputSchema={
                "type": "object",
                "properties": {
                    "task": {
                        "type": "string",
                        "description": "Name of the just task to run (e.g., 'check', 'build', 'test')",
                    },
                    "args": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Optional arguments to pass to the task",
                    },
                },
                "required": ["task"],
            },
        ),
    ]


@server.call_tool()
async def call_tool(name: str, arguments: dict) -> list[TextContent]:
    """Handle tool calls."""
    if name == "list_tasks":
        tasks = parse_just_list()
        lines = ["Available just tasks:", ""]
        for t in tasks:
            if t["description"]:
                lines.append(f"  {t['name']}: {t['description']}")
            else:
                lines.append(f"  {t['name']}")
        return [TextContent(type="text", text="\n".join(lines))]

    if name == "run_task":
        task = arguments.get("task", "")
        args = arguments.get("args", [])

        if not task:
            return [TextContent(type="text", text="Error: task name is required")]

        result = run_just_task(task, args)

        output_parts = []
        if result["stdout"]:
            output_parts.append(f"STDOUT:\n{result['stdout']}")
        if result["stderr"]:
            output_parts.append(f"STDERR:\n{result['stderr']}")
        output_parts.append(f"\nExit code: {result['exit_code']}")

        return [TextContent(type="text", text="\n".join(output_parts))]

    return [TextContent(type="text", text=f"Unknown tool: {name}")]


async def main():
    """Run the MCP server."""
    async with stdio_server() as (read_stream, write_stream):
        await server.run(read_stream, write_stream, server.create_initialization_options())


if __name__ == "__main__":
    import asyncio

    asyncio.run(main())
