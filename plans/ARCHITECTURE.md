# Heroku CLI Modernization — Architecture

## Overview
This document describes the high-level architecture of the new Heroku CLI replacement.  
It is designed for **performance, security, and maintainability**, while delivering a modern TUI experience.

---

## Technology Choices
- **Language**: Rust — safe, fast, produces static binaries.
- **TUI**: Ratatui — modern ncurses-style library.
- **Networking**: reqwest + hyper.
- **Serialization**: serde / serde_json.
- **Schema foundation**: JSON Hyper-Schema (Heroku API).
- **Plugin execution**: Model Context Protocol (MCP) apps via stdio.

---

## Command Routing
- **Direct CLI mode**:
  - `heroku <command> [args] [flags]`
  - Routes directly to controller logic.
- **Interactive TUI mode**:
  - `heroku`
  - Launches Ratatui interface.
- **Command definitions**:
  - Args = required properties (schema).
  - Flags = optional properties (schema).

---

## Binary Size
- Expected stripped release build: **2–6 MB**.
- Schema and workflows stored externally (not embedded).
- Much smaller surface area than Node.js CLI (~30–100 MB).

---

## Benefits
- **Reduced maintenance**: schema-driven codegen.
- **Robustness**: Rust prevents large classes of runtime errors.
- **Extensibility**: Plugins isolated from core logic.
- **User experience**: unified TUI and CLI modes.
