---
name: torrentleech
description: Reference for using the tl command with TorrentLeech: login, search, show details, download torrent files, and inspect configuration.
---

# TorrentLeech CLI reference

`tl` is a terminal client for TorrentLeech. It signs in with saved cookies,
queries browse pages, prints result metadata, and saves `.torrent` files. It does
not launch a BitTorrent client.

## Quick reference

| Task | Command |
| - | - |
| Sign in | `TL_USERNAME=<user> TL_PASSWORD_FILE=<path> tl login` |
| Check session | `tl whoami` |
| Search | `tl search '<query>'` |
| Filter search | `tl search '<query>' --category apps --freeleech --limit 25` |
| JSON search | `tl search '<query>' --json` |
| Show details | `tl show <id>` |
| Description text | `tl show <id> --description-only` |
| Save torrent file | `tl download <id> --output-dir <dir> --print-path` |
| Categories | `tl categories` |

## Recommended sequence

Start by checking whether the cookie jar is still valid:

```bash
tl whoami
```

If that fails and credentials are available, sign in:

```bash
TL_USERNAME=<user> TL_PASSWORD_FILE=<path> tl login
```

Search with a small limit first:

```bash
tl search '<query>' --limit 10
```

Use `show` when a result needs confirmation:

```bash
tl show <id> --description-only
```

Save the selected `.torrent` file and capture the printed path:

```bash
tl download <id> --output-dir <dir> --print-path
```

## Search

Basic form:

```bash
tl search [query] [--category <name-or-id>]... [--freeleech] [--limit <n>] \
  [--page <n>] [--sort <field>] [--order asc|desc] [--json]
```

The query can be omitted when browsing categories. Category values may be group
names, category names, aliases, or numeric IDs. Matching ignores case, spaces,
hyphens, and underscores.

Sort fields are `added`, `size`, `seeders`, `leechers`, `completed`, `comments`,
and `name`. The limit comes from config `default_limit` when present, otherwise
it is 10. The maximum limit is 100.

Plain search output is one result per line:

```text
241774912 2d s111 l23 6.8GiB Apps/0-day FL TL-0day MAY26-2026
```

Column order is ID, age, seeders, leechers, size, category, freeleech marker,
and title. A compact search with no matches exits successfully and prints no
rows.

Use JSON when exact field names are needed:

```bash
tl search '<query>' --json
```

## Details

`show` accepts an ID, a TorrentLeech detail URL, or a direct download URL:

```bash
tl show 12345
tl show 12345 --description-only
tl show 12345 --json
tl show 'https://www.torrentleech.org/torrent/12345'
tl show 'https://www.torrentleech.org/download/12345/example.torrent'
```

Default detail output contains a compact metadata block followed by description
and NFO text when the page provides them. JSON includes `id`, `title`,
`category`, `added`, `size`, `seeders`, `leechers`, `completed`, `uploader`,
`tags`, `description`, `nfo`, and `download_url`.

## Downloading torrent files

```bash
tl download <id> --output-dir <dir> --print-path
tl download <id> --output-dir <dir> --force --print-path
tl download 'https://www.torrentleech.org/torrent/12345' --print-path
tl download 'https://www.torrentleech.org/download/12345/example.torrent' --print-path
```

The target can be an ID, a detail URL, or a direct download URL. Existing output
files are not overwritten unless `--force` is supplied. Successful downloads are
quiet unless `--print-path` is present.

## Authentication and configuration

Recognized environment variables:

```text
XDG_CONFIG_HOME
XDG_STATE_HOME
TL_CONFIG
TL_USERNAME
TL_PASSWORD_FILE
TL_COOKIE_JAR
TL_OUTPUT_DIR
TL_DEFAULT_LIMIT
```

Defaults follow XDG: `${XDG_CONFIG_HOME:-~/.config}/tl/config.toml` for config and `${XDG_STATE_HOME:-~/.local/state}/tl/cookies.json` for cookies.

Config example:

```toml
username = "your-user"
password_file = "/path/to/password"
cookie_jar = "~/.local/state/tl/cookies.json"
output_dir = "."
default_limit = 10
```

Commands that need authentication read the cookie jar before attempting login.
When a saved session has expired, the command signs in once and retries if
credentials are available. Browser challenges are reported as failures.

Use `TL_PASSWORD_FILE`, config `password_file`, or `--password-stdin`. Do not use raw password environment variables for service deployments. Password files must be regular files. On Unix, they must not be readable by group or others.

For Linux services, prefer systemd credentials:

```ini
[Service]
User=tl
Environment=TL_CONFIG=/etc/tl/config.toml
Environment=TL_PASSWORD_FILE=%d/torrentleech-password
LoadCredentialEncrypted=torrentleech-password:/etc/credstore.encrypted/torrentleech-password.cred
StateDirectory=tl
```

## Config commands

```bash
tl config init
tl config init --path ./config.toml
tl config path
tl config show
tl config show --json
```

`config init` creates a starter file without replacing an existing file.
`config show` prints values from the config file, not resolved environment or
flag overrides.

## Categories

Common groups are:

```text
movies, tv, games, apps, education, animation, books, music, foreign
```

Run `tl categories` for IDs and aliases. Add `--json` for grouped structured
output.

## Process results

| Code | Meaning |
| - | - |
| 0 | Success |
| 1 | Unexpected, network, or site failure |
| 2 | Invalid input or config |
| 3 | Authentication required, login failed, or browser challenge required |
| 4 | Parse failure |
| 5 | Output conflict |
