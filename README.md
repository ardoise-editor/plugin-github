# GitHub — an Ardoise Editor plugin

GitHub (github.com or GitHub Enterprise) in [Ardoise Editor](https://github.com/ardoise-editor/ardoise), as a tab of the Git panel. It works with or without the Git plugin, and needs `git` and Ardoise 2.8 or later.

- **Pull requests**: the open, merged or closed ones; make one (draft or not) from the branch checked out; read one with its description, its files and their diffs in a tab, its comments and reviews (comment, answer on lines), its checks; approve or request changes, merge (merge or squash, delete the branch when you tick it), close, reopen. GitHub does not delete pull requests.
- **Actions**: the latest runs, their jobs and logs; start a workflow by hand; rerun failed jobs, cancel, delete a run, rerun a job.
- **Tags**: list, create (annotated with a message), delete.

The repository is the one of the `origin` remote. For each server a personal access token is asked for in the panel (a classic token with the `repo` and `workflow` scopes). It is kept in the system keychain (or a file only you can read when there is none) and **sent only to the server you typed it for**, over HTTPS.

Merging asks GitHub for the commit you saw: if the branch moved since, GitHub refuses.

## Build

```sh
cargo test
cargo build --release && mkdir -p bin && cp target/release/github bin/
```

## License

MIT
