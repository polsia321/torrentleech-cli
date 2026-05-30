# torrentleech-cli

`torrentleech-cli` provides the `tl` command for searching, authenticating with, and downloading torrents from TorrentLeech.

## Installation

Download the archive for your platform from the latest GitHub release:

- `tl-linux-x86_64.tar.gz`
- `tl-macos-aarch64.tar.gz`
- `tl-windows-x86_64.zip`

Linux:

```bash
mkdir -p ~/.local/bin
curl -L https://github.com/polsia321/torrentleech-cli/releases/latest/download/tl-linux-x86_64.tar.gz \
  | tar -xz
install -m 755 tl ~/.local/bin/tl
```

macOS on Apple Silicon:

```bash
mkdir -p ~/.local/bin
curl -L https://github.com/polsia321/torrentleech-cli/releases/latest/download/tl-macos-aarch64.tar.gz \
  | tar -xz
install -m 755 tl ~/.local/bin/tl
```

Make sure `~/.local/bin` is on `PATH`.

Windows:

1. Download `tl-windows-x86_64.zip`.
2. Extract `tl.exe`.
3. Put `tl.exe` in a directory on `PATH`.

Build from source:

```bash
cargo install --git https://github.com/polsia321/torrentleech-cli.git --locked
```

Nix users can run the flake directly:

```bash
nix run github:polsia321/torrentleech-cli
```

## Configuration

Create a starter config:

```bash
tl config init
tl config init --path ./config.toml
```

`config init` does not overwrite an existing file.

Print the active config path:

```bash
tl config path
```

Show non-secret config file values:

```bash
tl config show
tl config show --json
```

`config show` reads the config file only. It does not include environment or flag overrides.

The active config path follows XDG: `${XDG_CONFIG_HOME:-~/.config}/tl/config.toml`. The cookie jar defaults to `${XDG_STATE_HOME:-~/.local/state}/tl/cookies.json`. The config path can be overridden with `--config` or `TL_CONFIG`:

```bash
tl --config ~/.config/tl/config.toml config show
TL_CONFIG=./config.toml tl config path
```

Supported config file keys:

```toml
username = "your-user"
cookie_jar = "~/.local/state/tl/cookies.json"
output_dir = "."
default_limit = 10
password_file = "~/.config/tl/password"
```

Relative paths in config file values are resolved by the current process. Paths beginning with `~` are expanded when read from the config file. If `cookie_jar` is omitted, the default is `~/.local/state/tl/cookies.json`.

## Environment variables

| Variable | Purpose |
| - | - |
| `TL_CONFIG` | Config file path |
| `TL_USERNAME` | Login username |
| `TL_PASSWORD_FILE` | File containing the login password |
| `TL_COOKIE_JAR` | Cookie jar path for authenticated sessions |
| `TL_OUTPUT_DIR` | Default download output directory |
| `TL_DEFAULT_LIMIT` | Default search result limit |

Command-line flags take precedence over environment variables. Environment variables take precedence over config file values.

## Authentication

Log in and save session cookies. On success, `login` prints the cookie jar path it wrote:

```bash
TL_USERNAME=your-user TL_PASSWORD_FILE=./password tl login
tl login --username your-user --password-file ./password --cookie-jar ./cookies.json
```

You can also read the password from stdin:

```bash
secret-tool lookup service torrentleech username your-user | tl login --username your-user --password-stdin
```

Commands that need authentication load the saved cookie jar first. If the session is missing or expired and credentials are available from flags, environment, or config, they log in once and retry. Password files must be regular files. On Unix, they must not be readable by group or others.

For a Linux service, keep config non-secret and store the password as a private file or systemd credential:

```toml
username = "your-user"
password_file = "/etc/tl/torrentleech-password"
cookie_jar = "/var/lib/tl/cookies.json"
output_dir = "/srv/torrents/watch"
default_limit = 10
```

```bash
sudo install -d -m 700 -o tl -g tl /etc/tl /var/lib/tl
sudo install -m 600 -o tl -g tl ./password /etc/tl/torrentleech-password
```

With systemd credentials, omit `password_file` from config and point `TL_PASSWORD_FILE` at the credential path:

```ini
[Service]
User=tl
Environment=TL_CONFIG=/etc/tl/config.toml
Environment=TL_PASSWORD_FILE=%d/torrentleech-password
LoadCredentialEncrypted=torrentleech-password:/etc/credstore.encrypted/torrentleech-password.cred
StateDirectory=tl
```

Check the authenticated user:

```bash
tl whoami
tl whoami --cookie-jar ./cookies.json
```

Remove the saved cookie jar:

```bash
tl logout
tl logout --cookie-jar ./cookies.json
```

## Categories

List categories in compact text form:

```bash
tl categories
```

Print categories as JSON:

```bash
tl categories --json
```

JSON output is grouped by category group:

```json
{
  "groups": [
    {
      "name": "Movies",
      "categories": [
        { "id": 8, "name": "Cam", "aliases": ["cam"] }
      ]
    }
  ]
}
```

## Search

Search by query:

```bash
tl search ubuntu
```

Browse by category without a query:

```bash
tl search --category movies
tl search --category 33 --category apps
```

Categories accept IDs, group names, category names, and aliases. Matching ignores case, spaces, hyphens, and underscores, so `0-day`, `0day`, and `0_day` are equivalent.

Useful search flags:

```bash
tl search ubuntu --category movies --freeleech --limit 25 --page 2 --sort seeders --order desc
tl search ubuntu --json
```

`--sort` accepts `added`, `size`, `seeders`, `leechers`, `completed`, `comments`, and `name`. `--order` accepts `asc` and `desc`. `--limit` defaults to config `default_limit`, then 10, and must be at most 100. Zero-result searches exit successfully and print nothing in compact mode.

## Show

Read torrent details and description text:

```bash
tl show 12345
tl show 12345 --description-only
tl show 'https://www.torrentleech.org/torrent/12345' --json
tl show 'https://www.torrentleech.org/download/12345/example.torrent'
```

Default output prints compact metadata followed by description text and NFO text when available. `--description-only` prints only the description text, falling back to NFO text when no description is present. JSON output includes the parsed detail fields: `id`, `title`, `category`, `added`, `size`, `seeders`, `leechers`, `completed`, `uploader`, `tags`, `description`, `nfo`, and `download_url`.

## Download

Download by torrent id:

```bash
tl download 12345 --output-dir ./downloads
```

Download from a TorrentLeech detail or direct download URL:

```bash
tl download 'https://www.torrentleech.org/torrent/12345' --print-path
tl download 'https://www.torrentleech.org/download/12345/example.torrent' --filename example.torrent
```

By default, downloads fail if the target file already exists. Use `--force` to overwrite:

```bash
tl download 12345 --output-dir ./downloads --force
```

Successful downloads are silent unless `--print-path` is used.

## Output and exit codes

Successful command output is written to stdout. Human-readable errors are written to stderr with an `error:` prefix.

| Exit code | Meaning |
| - | - |
| 0 | Success |
| 1 | Unexpected or network failure |
| 2 | Invalid input |
| 3 | Authentication failure or required browser challenge |
| 4 | Parse failure |
| 5 | Output conflict |

## Validation

Run the project checks:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D clippy::all
cargo test
```

The tests use unit coverage and mock HTTP integration fixtures. They do not require live TorrentLeech credentials.
