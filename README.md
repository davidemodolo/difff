# difff

Side-by-side diff viewer for Linux, built with GTK4 and Rust. Supports semantic normalization to reduce noise in comparisons (whitespace, line endings, Unicode variants).

## Requirements

GTK 4.14 or later.

## Build

```
cargo build --release
```

## Install (Debian/Ubuntu)

```
cargo deb
sudo dpkg -i target/debian/difff_*.deb
```

## Usage

```
difff <file1> <file2>
```

## License

MIT
