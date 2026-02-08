# Lazy RSS

Lazy RSS (`lazyrss`) is Rust-based RSS TUI reader inspired by all things lazy:
[lazygit](https://github.com/jesseduffield/lazygit),
[lazydocker](https://github.com/jesseduffield/lazydocker), and lazy vibe coding.

![](./docs/lazyrss.png)

About 99% of this codebase is written exclusively by LLM code companions in a
matter of a few tens of minutes. I claim authorship only to the extent that I
prompted the agents to do so.

## Installation

Lazy RSS  is available on AUR and can be installed with `yay`:

```sh
yay -S lazyrss
```

It is available on Crates and can be installed with `cargo`:

```sh
cargo install lazyrss
```

Alternatively, it can be built and installed manually with `cargo` if Rust is
installed:

```sh
git clone https://github.com/sujaltv/lazyrss
cd lazyrss
cargo build --release
sudo install -m 755 target/release/lazyrss /usr/local/bin/ # or in ~/.local/...
sudo install -m 644 man/lazyrss.1.gz /usr/local/share/man/man1/ # or in ~/.local/...
mandb
```

## Usage

Simply run in the CLI:

```sh
lazyrss
```

Configurations may be optionally made in `$XDG_CONFIG_HOME/lazyrss/config.yaml`.
To see all the options available for configuration, run one of:

```sh
man 1 lazyrss

# or
lazyrss --help
```

The news articles are stored in `$XDG_DATA_HOME/lazyrss/news.db` as an SQLite
database file.

## Licence

[MIT](./LICENCE)
