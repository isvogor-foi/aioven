# AIOven

A lean coding agent harness in the style of Claude Code, built as a fork of [OpenCode](https://github.com/anomalyco/opencode).

- **Terminal client in Rust (Ratatui):** agent tabs, a live blueprint of components and changed files with diffs, token stats and a usage heat map.
- **One main agent, small helpers:** `bake` writes code and `recipe` plans. Read-only subagents do the side work: `pantry` explores, `taster` reviews, `thermometer` runs tests and `cookbook` researches.
- **Model tiers:** small, medium and large, so cheap models do the cheap work. Works with GitHub Copilot and any OpenCode provider.
- **Component-based design first:** a recipe defines components, interfaces and how they communicate before any code is written.
- **Terse by default:** "caveman" mode (ultra) keeps agent output short to save tokens.

## Install

```sh
git clone git@github.com:isvogor-foi/aioven.git && cd aioven
bin/aioven-install      # needs bun and Rust
aioven                  # in any project
```

## Docs

- [AIOVEN.md](AIOVEN.md): screen, keys, agents and config
- [ARCHITECTURE-AIOVEN.md](ARCHITECTURE-AIOVEN.md): components, interfaces and design decisions
- [docs/aioven-upstream.md](docs/aioven-upstream.md): keeping up with OpenCode releases
- [docs/aioven-validation.md](docs/aioven-validation.md): real-model check and benchmark

## Credits

AIOven is based on OpenCode (MIT, see [LICENSE](LICENSE)); its original README is [README.opencode.md](README.opencode.md).
