# ai-query-check

A [Claude Code hook](https://docs.anthropic.com/en/docs/claude-code/hooks) that auto-approves read-only SQL queries so you stop clicking "approve" on every `SELECT`.

Modifying statements (`INSERT`, `UPDATE`, `DELETE`, `DROP`, etc.) still trigger the normal permission prompt.

## Supported commands

- `mysql -e "..."` / `mysql --execute="..."`
- `psql -c "..."` / `psql --command="..."`

Works through `docker exec`, pipes, subshells, and other bash wrappers.

## How it works

1. Claude Code pipes the bash command as JSON to stdin on `PermissionRequest`
2. Bash is parsed with `brush-parser` to pull SQL out of `mysql`/`psql` arguments
3. Each SQL statement is parsed with `sqlparser` and classified as read-only or modifying
4. All read-only? Outputs `{"behavior": "allow"}` to auto-approve
5. Anything else? Exits silently, falls through to the normal prompt

## Build

```sh
cargo build --release
```

Binary ends up at `target/release/ai-query-check`.

## Configure

Add to your Claude Code hooks config (e.g. `~/.claude/settings.json`):

```json
{
  "hooks": {
    "PermissionRequest": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/ai-query-check"
          }
        ]
      }
    ]
  }
}
```

## Tests

```sh
cargo test
```
