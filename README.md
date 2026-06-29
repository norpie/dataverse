# Dataverse

A Rust-based suite of tools for interacting with Microsoft Dataverse/Dynamics 365.

## Projects

### dataverse-lib

Core library providing the foundation for Dataverse/Dynamics 365 interactions. Handles authentication, API communication, and data operations.

### dataverse-cli

Command-line interface for Dataverse operations. Built on top of dataverse-lib for scripting and automation tasks.

### dataverse-tui

Terminal user interface for interactive Dataverse management. This is the main distributable binary.

### rafter

A web-inspired TUI framework featuring:
- Macro-based declarative views with HTML-like structure
- Inline CSS-inspired styling and alignment
- First-class async support for interactions

Designed to be general-purpose and applicable beyond Dataverse use cases.

## Goals

- Provide ergonomic Rust APIs for Dataverse/Dynamics 365
- Enable automation through CLI tooling
- Offer an interactive terminal experience for data management
- Create a reusable TUI framework for the broader Rust ecosystem

## Installation

From source:

```bash
cargo install --path dataverse-tui
```

Run directly:

```bash
cargo run -p dataverse-tui
```

Release builds are published from `vX.Y.Z` tags on GitHub Releases.
