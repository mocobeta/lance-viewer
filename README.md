# lance-viewer

`lance-viewer` is a read-only terminal user interface for exploring datasets
stored in the [Lance](https://docs.lancedb.com/lance) lakehouse format. The
binary is named `lancev`.

It is intended for inspecting a dataset without writing a custom script. The
viewer brings version history, schema and fragment metadata, data-file and
index layouts, and the underlying storage tree into one navigable interface.

## Features

- Summarizes dataset metadata, storage statistics, and schema fields.
- Browses manifests, schemas, fragments, and indices by dataset version.
- Shows the files associated with each fragment and index.
- Inspects the logical and byte-level layout of supported Lance files.
- Displays the dataset storage tree with file sizes.
- Supports light and dark terminal themes.
- Provides keyboard navigation and an in-application Help tab.

## Requirements

Development requires a Rust installation with Cargo. Your terminal should
support colors and the alternate screen used by terminal applications.

You also need access to a Lance dataset. Local filesystem paths work directly;
other URI schemes depend on the storage support and credentials available to
the Lance libraries in your environment.

## Quick start

Launch the viewer with a dataset URI:

```bash
lancev --uri /path/to/dataset.lance
```

If `--uri` is omitted, the viewer attempts to open the current directory:

```bash
lancev
```

Choose the startup theme with `--theme` or `-t`:

```bash
lancev --uri /path/to/dataset.lance --theme light
```

## Command-line options

```text
Usage: lancev [OPTIONS]

Options:
      --uri <URI>            Lance dataset URI
  -t, --theme <light|dark>   Startup theme (default: dark)
  -h, --help                 Print help
```

Both `--uri=value` and `--theme=value` forms are accepted. Run
`lancev --help` to print the options from the current build.

## Navigation

The interface uses nested panes. Select a row, move right to inspect its child
pane, and move left or press Escape to return toward the tab bar.

| Key | Action |
| --- | --- |
| `←` / `→`, `h` / `l` | Change tab or move between panes |
| `↑` / `↓`, `k` / `j` | Move the current selection |
| `Enter` | Open an item or expand/collapse a tree node |
| `Esc` | Return to the parent pane or tab bar |
| `Tab` | Select the next tab while the tab bar is focused |
| `t` | Toggle the light/dark theme |
| `q` | Quit |

The Help tab contains the same shortcuts and a short description of every
screen.

## Screens

- **Dataset Overview** shows dataset identity, row and file statistics, index
  statistics, and the top-level schema.
- **Tables** browses versions and their manifest, schema, fragment, and index
  metadata. Selected fragments and indices can be opened in their dedicated
  screens.
- **Data Files** browses versions, fragments, and their data files, including
  a layout outline and a visual byte-layout summary.
- **Indices** browses versioned indices, index metadata, index files, and
  supported file layouts.
- **Storage Layout** presents the paths belonging to the dataset as an
  expandable tree and includes file sizes when available.
- **Help** documents startup options, shortcuts, and screens.

## Development

Common validation commands are:

```bash
cargo check
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

Use `cargo fmt --all` to apply formatting. Keep `Cargo.toml` and `Cargo.lock`
in sync when dependencies change.

### Making changes

- Keep UI state transitions in `app.rs` where practical so they can be tested
  without running an interactive terminal.
- Keep Lance and object-store access in `dataset.rs`, and pass display-oriented
  data to the tab renderers.
- Add regression tests for navigation, parsing, metadata handling, and layout
  rendering changes.
- Avoid committing datasets, credentials, build output, or machine-specific
  configuration.
