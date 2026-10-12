# forgejo_import

Mirror GitHub organisations, users and repositories into a Forgejo instance from the command line.

[Install](#install) · [Quick start](#quick-start) · [Configuration](#configuration)

| Feature | Notes |
| --- | --- |
| `mirror-org` | Creates the Forgejo organisation (name, description, website, avatar) and mirrors every repository the token can see |
| `mirror-user` | Same for a GitHub user; includes private repositories when the token belongs to that user |
| `mirror-repo` | Mirrors a single repository into any Forgejo user or organisation |
| `delete-org` | Deletes a Forgejo organisation and all of its repositories, after confirmation |
| Forgejo versions | Works with every Forgejo release from 1.18 on (and Gitea); the organisation avatar needs Gitea API 1.21 or newer and is skipped otherwise |

Private GitHub repositories stay private on Forgejo. Mirrors cannot carry issues, labels, pull requests, releases or milestones (Forgejo drops them for mirrors), so only wiki and LFS migration are offered.

## Install

```sh
cargo install --git https://github.com/DevYukine/forgejo_import
```

## Quick start

```sh
# mirror a whole organisation into a private Forgejo organisation of the same name
forgejo_import mirror-org --forgejo-url https://forgejo.example.org --forgejo-token <token> --github-token <token> --visibility private octo-org

# mirror your own account, wiki included, into the organisation "octo-mirror"
forgejo_import mirror-user --migrate-wiki --output-organisation-name octo-mirror octocat

# mirror one repository into a user or organisation
forgejo_import mirror-repo --output-owner octo-mirror https://github.com/octocat/Hello-World

# delete an organisation without the prompt
forgejo_import delete-org --yes octo-mirror
```

Run `forgejo_import <command> --help` for all flags.

## Configuration

Connection details can live in `forgejo_import.config.json` instead of the command line. Flags given on the command line win; `migrateWiki` and `migrateLfs` from the file switch a migration on but cannot switch a flag off.

```json
{
  "forgejoUrl": "https://forgejo.example.org",
  "forgejoToken": "...",
  "githubToken": "...",
  "migrateWiki": true,
  "migrateLfs": false
}
```

| Option | Use it to |
| --- | --- |
| `forgejoUrl` | Point at the Forgejo instance |
| `forgejoToken` | Authenticate against Forgejo with a token that may create and delete organisations and repositories |
| `githubToken` | Read from GitHub; Forgejo stores it for mirror syncs, so use a read-only fine-grained token |
| `migrateWiki` | Also mirror wikis |
| `migrateLfs` | Also mirror LFS objects |

The first file found wins, searched in this order: current directory, `%APPDATA%\forgejo_import` (Windows), `$XDG_CONFIG_HOME/forgejo_import`, `~/.config/forgejo_import`, `~`. Unknown keys are rejected so typos do not pass silently.

## Development

```sh
cargo test
```

`cargo build` produces the binary; `cargo test` runs the unit tests.

## Built with

- **reqwest** for HTTP with rustls
- **clap** for the command line
- **serde** for JSON

## Contributing

Open an issue or a pull request. Commits follow Conventional Commits (`feat(scope): ...`, `fix(scope): ...`).

## License

MIT, see [LICENSE](LICENSE).
