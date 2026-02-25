# Documentation

This directory contains all documentation for the Myoro Polymarket Terminal project.

## Structure

Documentation is organized by feature/purpose:

### `ai-rules/` — AI Assistant Rules

Centralized rules for AI coding assistants (Cursor, Claude Code, Windsurf, etc.). These rules define coding standards, project philosophy, and communication style.

**Files:**
- `code-owner.md` — You own the full development lifecycle
- `polymarket-arbitrage.md` — Project philosophy and domain focus
- `rust-standards.md` — Rust coding standards summary
- `concise-responses.md` — Communication style guidelines
- `plans-contain-only-plan.md` — Planning document standards
- `visual-and-themes.md` — TUI theme consistency rules

**Usage:**
- Referenced by `.cursor/rules/rules.mdc` for Cursor
- Referenced by `CLAUDE.md` for Claude Code
- Easy to add support for other AI assistants (Windsurf, Bolt, etc.)

### `standards/` — Code Standards

Detailed technical standards and best practices.

**Files:**
- `STANDARDS.md` — Comprehensive Rust standards: principles, layout, error handling, async, testing, dependencies

### `setup/` — Setup & Onboarding

Everything needed to get started with development.

**Files:**
- `CREDENTIALS.md` — What credentials are needed (quick reference)
- `DATA_AND_CREDENTIALS.md` — Detailed credential setup instructions
- `POLYMARKET_SETUP.md` — Polymarket integration details
- `GETTING_STARTED.md` — Quick start guide for development

## Philosophy

This documentation follows a **feature-based folder structure** principle:
- Related files are grouped by feature/purpose, not by file type
- Makes it easy to find all information about a specific aspect
- Scales better than flat file structures
- See `standards/STANDARDS.md` for more on this pattern

## For AI Assistants

If you're an AI assistant (Claude Code, Cursor, etc.) working in this repository:

1. Read all files in `ai-rules/` for project-wide rules
2. Reference `standards/STANDARDS.md` for detailed Rust standards
3. Check `setup/` for environment and credential setup information
4. Follow the feature-based organization when adding new documentation
