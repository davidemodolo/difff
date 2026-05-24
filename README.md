# difff

Side-by-side diff viewer for Linux, built with GTK4 and Rust. Supports semantic normalization to reduce noise in comparisons (whitespace, line endings, Unicode variants).

## Requirements

- **Rust** 1.70 or later ([rustup](https://rustup.rs))
- **GTK 4.14** or later development libraries

### Install GTK4 (platform-specific)

| Platform | Command |
|---|---|
| Debian/Ubuntu | `sudo apt install libgtk-4-dev build-essential` |
| Fedora | `sudo dnf install gtk4-devel` |
| Arch | `sudo pacman -S gtk4` |
| openSUSE | `sudo zypper install gtk4-devel` |

## Install

### From source (with cargo)

```
cargo install --path .
```

This compiles and installs `difff` to `~/.cargo/bin/`. Make sure that directory is in your `$PATH`.

### From source (manual build)

```
cargo build --release
sudo cp target/release/difff /usr/local/bin/
```

### Debian package

```
cargo install cargo-deb
cargo deb
sudo dpkg -i target/debian/difff_*.deb
```

## Usage

```
difff <file1> <file2>
```

## License

MIT
