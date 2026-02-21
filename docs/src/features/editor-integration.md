# Editor Integration

Boundary ships a Language Server Protocol (LSP) server — `boundary-lsp` — that brings architectural violation detection directly into your editor as you code.

## What It Does

- **Inline diagnostics** — layer boundary violations, missing ports, and other violations appear as errors and warnings on the offending import lines
- **Hover info** — hover over any type to see its architectural layer classification
- **Live feedback** — re-analyzes on every file save so diagnostics stay current

## Installation

`boundary-lsp` is distributed alongside the main `boundary` binary. If you installed via Homebrew, it is already available:

```bash
which boundary-lsp
```

If you installed from source, build it with:

```bash
cargo install --git https://github.com/rebelopsio/boundary boundary-lsp
```

## Editor Setup

### Neovim

The recommended way is [boundary.nvim](https://github.com/rebelopsio/boundary.nvim), a dedicated plugin that provides LSP integration, commands, and statusline support. Requires Neovim 0.11+.

**lazy.nvim:**

```lua
{
  "rebelopsio/boundary.nvim",
  opts = {},
}
```

This gives you inline diagnostics, hover info, and commands like `:BoundaryAnalyze`, `:BoundaryScore`, `:BoundaryCheck`, and `:BoundaryDiagram`. See the [boundary.nvim README](https://github.com/rebelopsio/boundary.nvim) for the full feature list and configuration options.

**Manual setup (nvim-lspconfig, Neovim < 0.11):**

```lua
local lspconfig = require("lspconfig")
local configs = require("lspconfig.configs")

if not configs.boundary then
  configs.boundary = {
    default_config = {
      cmd = { "boundary-lsp" },
      filetypes = { "go", "rust", "typescript", "java" },
      root_dir = lspconfig.util.root_pattern(".boundary.toml", ".git"),
      single_file_support = false,
    },
  }
end

lspconfig.boundary.setup({})
```

### VS Code

Install the [Boundary extension](https://marketplace.visualstudio.com/items?itemName=rebelopsio.boundary) from the VS Code Marketplace. It manages `boundary-lsp` automatically.

To configure manually, add to your `settings.json`:

```json
{
  "boundary.lsp.enable": true,
  "boundary.lsp.path": "boundary-lsp"
}
```

### Helix

Add to `~/.config/helix/languages.toml`:

```toml
[[language]]
name = "go"
language-servers = ["boundary-lsp"]

[[language]]
name = "rust"
language-servers = ["boundary-lsp"]

[language-server.boundary-lsp]
command = "boundary-lsp"
```

### Emacs (eglot)

```elisp
(with-eval-after-load 'eglot
  (add-to-list 'eglot-server-programs
               '((go-mode go-ts-mode) . ("boundary-lsp"))))
```

## How It Works

`boundary-lsp` runs `boundary`'s analysis pipeline in the background using the project's `.boundary.toml` configuration. On initialization and after each file save, it re-analyzes the project and publishes LSP diagnostics mapped to the exact import lines that cause violations.

The server auto-detects languages from file extensions, so no additional configuration is needed beyond what your `.boundary.toml` already defines.
