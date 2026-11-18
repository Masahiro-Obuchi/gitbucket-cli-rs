# GitBucket CLI

`gb` is an unofficial command-line tool for working with GitBucket repositories, issues, and pull requests. It is written in Rust and inspired by [GitHub CLI](https://cli.github.com/).

## Prerequisites

- A GitBucket instance, its base URL, and a personal access token from **Account Settings → Personal access tokens**.
- Git for operations such as cloning, checking out a pull request, or producing a diff.
- Rust and Cargo for Cargo installation or a source build. This source revision requires Rust 1.88.0 or later.

## Installation

Download the archive for your platform from [GitHub Releases](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/releases), extract it, and put `gb` or `gb.exe` on your `PATH`. Keep the extracted folder to browse its bundled documentation.

Or install with Cargo:

```sh
cargo install gitbucket-cli --locked
```

See [building from source](docs/maintainers/testing.md#build-from-source) for a checkout of this repository.

## Quick Start

```sh
gb auth login -H https://gitbucket.example.com
gb issue list -R alice/project
```

Enter your token when prompted. If your instance uses a path prefix, use the full base URL, such as `https://gitbucket.example.com/gitbucket`. Run `gb --help` for available commands.

## Documentation

See the [documentation index](docs/README.md) for command examples, configuration, automation, and the tested GitBucket compatibility baseline.

## License and Acknowledgements

[MIT](LICENSE). Thanks to the [GitBucket](https://gitbucket.github.io/) project and its community.
