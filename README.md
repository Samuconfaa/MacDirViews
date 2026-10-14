<p align="center">
  <strong>English</strong> &nbsp;|&nbsp;
  <a href="README_IT.md">Italiano</a> &nbsp;|&nbsp;
  <a href="README_FR.md">Français</a> &nbsp;|&nbsp;
  <a href="README_ES.md">Español</a> &nbsp;|&nbsp;
  <a href="README_RU.md">Русский</a>
</p>

***

# MacDirViews

A fast, modern disk space visualizer and cleanup utility for macOS.

MacDirViews provides an intuitive, interactive graphical overview of your disk usage using squarified treemaps. It helps you instantly identify what is consuming storage across internal drives, external volumes, and individual folders, making disk cleanup fast, clear, and safe.

---

## Why Choose MacDirViews?

- **Blazing Fast & Non-Blocking**: Built in Rust with native multi-threaded scanning. Analyzes millions of files in seconds while keeping the interface completely responsive at 60 FPS.
- **Accurate APFS Accounting**: Measures actual physical disk allocation rather than nominal file sizes. It accounts for sparse files and avoids double-counting APFS copy-on-write clones and hard links.
- **Lightweight & Efficient**: Uses a native immediate-mode GUI engine with direct GPU rendering. Extremely low memory footprint with zero web runtime or Electron overhead.
- **Safe by Design**: Includes protection safeguards that prevent accidental deletion of critical macOS system directories (`/`, `/System`, `/usr`, `/bin`, home directories). Integrates seamlessly with the native macOS Trash for recoverable deletion.

---

## Key Features

- **Interactive Squarified Treemap**: Visualizes directory sizes as proportional geometric blocks. Click any block to view size and percentage details; double-click to drill down into any subfolder.
- **Top 10 Folders & Top 20 Files**: Dedicated rankings highlight the largest folders in the current view and the heaviest files across the entire scan.
- **Real-Time Filtering & Search**:
  - Filter items instantly by name with real-time text search.
  - Eliminate visual clutter using a logarithmic minimum size threshold slider.
- **External Volume Support**: Quickly select and scan mounted external drives, USB storage, and secondary partitions from the built-in volume selector.
- **Drag & Drop**: Drop any folder directly into the window from Finder to start an instant scan.
- **Finder & Trash Integration**: Right-click any file or directory to reveal it in Finder, move it to the macOS Trash, or delete it permanently with an explicit confirmation dialog.

---

## Installation & Download

### Pre-Built Binaries (.dmg / .app)

You do **not** need to compile the code yourself. Ready-to-use `.dmg` disk images and `.app` bundles are published for each version:

1. Download the latest `.dmg` or `.app` from [GitHub Releases](https://github.com/Samuconfaa/MacDirViews/releases).
2. Open the `.dmg` and drag **MacDirViews** into your `/Applications` folder.

> [!NOTE]
> If macOS Gatekeeper blocks an unsigned locally-downloaded build upon initial launch, right-click the app and choose **Open**, or run the following command in Terminal:
> ```bash
> xattr -cr /Applications/MacDirViews.app
> ```

### Building from Source (Optional)

If you prefer to compile from source:

#### Prerequisites
- macOS 11.0 (Big Sur) or later
- Apple Silicon (M1/M2/M3/M4) or Intel x86_64
- Rust toolchain (`cargo`) and Xcode Command Line Tools

```bash
# Run directly
cargo run --release

# Or build a standalone .app bundle
cargo install cargo-bundle
cargo bundle --release
```

---

## macOS Permissions (Full Disk Access)

macOS restricts access to sensitive user data (such as Mail, Messages, Safari history, and Photo libraries) via its Transparency, Consent, and Control (TCC) system.

To allow MacDirViews to scan the entire startup disk without encountering permission errors:

1. Open **System Settings** → **Privacy & Security** → **Full Disk Access**.
2. Click **+** and add **MacDirViews** (or your compiled binary).
3. Restart the application.

*(You can also click the in-app **Full Disk Access** button in the top bar to open this settings pane directly).*

---

## License

This project is licensed under the [Apache License 2.0](LICENSE).
