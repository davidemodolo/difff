# difff

Side-by-side diff viewer for Linux, built with GTK4 and Rust. Supports semantic normalization to reduce noise in comparisons (whitespace, line endings, Unicode variants).

## Requirements

- **Rust** 1.70 or later ([rustup](https://rustup.rs))
- **GTK 4.14** or later development libraries

### Install GTK4 (platform-specific)

| Platform      | Command                                        |
|---------------|------------------------------------------------|
| Debian/Ubuntu | `sudo apt install libgtk-4-dev build-essential` |
| Fedora        | `sudo dnf install gtk4-devel`                   |
| Arch          | `sudo pacman -S gtk4`                           |
| openSUSE      | `sudo zypper install gtk4-devel`                |

## Install from source

### Using cargo

```bash
cargo install --path .
```

Installs to `~/.cargo/bin/`. Ensure it's in your `$PATH`.

### Manual build

```bash
cargo build --release
sudo cp target/release/difff /usr/local/bin/
```

## Debian package (.deb)

### Building the .deb

Install `cargo-deb` if you haven't already:

```bash
cargo install cargo-deb
```

Build the release binary and package it:

```bash
cargo build --release
cargo deb
```

The `.deb` will be placed in `target/debian/difff_<version>_amd64.deb`.

### Installing the .deb

```bash
sudo dpkg -i target/debian/difff_*.deb
```

Or with automatic dependency resolution:

```bash
sudo apt install ./target/debian/difff_*.deb
```

This installs:
- `difff` binary → `/usr/bin/difff`
- Desktop entry → `/usr/share/applications/difff.desktop`
- App icon → `/usr/share/icons/hicolor/scalable/apps/difff.svg`

After installation, the app appears in your application launcher under **Development** / **Utilities**.

### Uninstalling

```bash
sudo apt remove difff
```

## Usage

Launch the GUI, then either:
- Click **Load Left** / **Load Right** to pick files individually
- Click **Open Both…** to select two files at once
- Drag and drop files from your file manager onto either panel

Enable **Ignore formatting** to hide lines that differ only in whitespace/bracket formatting.

## License

MIT
