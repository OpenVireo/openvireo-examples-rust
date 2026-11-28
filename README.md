# OpenVireo examples: Rust

Runnable examples for the [OpenVireo](https://openvireo.com) Rust core (`openvireo-core`, library `ov_core`), one file per guide on
[developer.openvireo.com](https://developer.openvireo.com). Each file holds the code from its guide's
**Rust** tab, wrapped in a program you can run.

Examples in other languages: [TypeScript](https://github.com/openvireo/openvireo-examples-typescript) · [Python](https://github.com/openvireo/openvireo-examples-python) · [Kotlin](https://github.com/openvireo/openvireo-examples-kotlin) · [Java](https://github.com/openvireo/openvireo-examples-java) · [Swift](https://github.com/openvireo/openvireo-examples-swift) · [C++](https://github.com/openvireo/openvireo-examples-cpp) · [ov CLI](https://github.com/openvireo/openvireo-examples-cli)

## Run them

You need Rust 1.98 or later. The core isn't on crates.io yet: `Cargo.toml` takes `openvireo-core` (and
`openvireo-share`, for `parse_share_link`) from the [v1.2.2 tag](https://github.com/openvireo/openvireo-core/releases/tag/v1.2.2)
of the core repository, and `Cargo.lock` pins the commit. The libraries keep their `ov_*` names, so the code
says `use ov_core::...`.

```toml
[dependencies]
openvireo-core = { git = "https://github.com/openvireo/openvireo-core", tag = "v1.2.2" }
```

```sh
git clone https://github.com/openvireo/openvireo-examples-rust
cd openvireo-examples-rust
cargo run -- check
```

Set your account in the environment. `OV_SECRET_KEY` is only needed the first time on a device;
`OV_ISSUER` picks an account server other than `https://accounts.openvireo.com`.

```sh
export OV_EMAIL=you@example.com OV_PASSWORD='...' OV_SECRET_KEY='A3-...'
```

```sh
cargo run -- first-program "ov://Personal/GitHub/username"
cargo run -- vaults-and-sharing
```

Every call that may touch the network or run the account KDF is `async` and takes a `CancelToken`;
the examples run on tokio. Calling `cancel()` on a token, from any thread, makes the call return
`aborted`. Handles (`OvClient`, `Account`, `Vault`, ...) are cheap to clone and safe to share between
tasks. Secrets go in and come out as `Vec<u8>` (UTF-8 for text); wipe the ones you keep.

### The Host

The core does no I/O: every program passes it a Host for HTTP, a secure store, an offline cache, a
clock and logging. [`src/host.rs`](src/host.rs) is a small one: reqwest with no redirects and no
cookies, and memory for the store and the cache. A real app keeps the store in the OS keychain and the
cache on disk so the device stays trusted and vaults open offline.

### Another core release

`scripts/update-core.sh <version>` points both dependencies at another release tag and updates
`Cargo.lock`.

## The examples

| Example | What it does | Guide |
|---|---|---|
| [`first_program.rs`](src/first_program.rs) | Sign in, list the vaults, read one field, sign out | [A first program](https://developer.openvireo.com/sdk/#a-first-program) |
| [`hosted_account.rs`](src/hosted_account.rs) | Connect, sign up, sign in with two-step codes, lock, devices, account deletion | [Hosted accounts](https://developer.openvireo.com/sdk/guides/hosted-account/) |
| [`vaults_and_sharing.rs`](src/vaults_and_sharing.rs) | List, open, read, write and edit vaults; share, revoke, organisations | [Vaults and sharing](https://developer.openvireo.com/sdk/guides/vaults-and-sharing/) |
| [`events.rs`](src/events.rs) | Follow live changes from other devices | [Events](https://developer.openvireo.com/sdk/guides/events/) |
| [`offline.rs`](src/offline.rs) | Edit offline, merge when back online | [Offline](https://developer.openvireo.com/sdk/guides/offline/) |
| [`move_to_account.rs`](src/move_to_account.rs) | Import a `.kdbx` file as a new vault or merge it into one | [Moving to an account](https://developer.openvireo.com/sdk/guides/move-to-account/) |
| [`recovery.rs`](src/recovery.rs) | Inspect a recovery link, reset, restore the old vaults | [Recovery](https://developer.openvireo.com/sdk/guides/recovery/) |
| [`self_hosted_sync.rs`](src/self_hosted_sync.rs) | Pair with a self-hosted sync server and sync a `.kdbx` file | [Self-hosted sync](https://developer.openvireo.com/sdk/guides/self-hosted-sync/) |
| [`service_accounts.rs`](src/service_accounts.rs) | Create a service account; read, inject and write with its token | [Service accounts](https://developer.openvireo.com/sdk/guides/service-accounts/) |
| [`share_links.rs`](src/share_links.rs) | Share an item or text by link, list and revoke links, open one | [Share links](https://developer.openvireo.com/sdk/guides/share-links/) |

Each file has one function per section of its guide. Running an example does something safe and
mostly read-only; the functions that change an account for good (deleting it, revoking access,
rotating keys) are there to read and copy, and the programs don't call them. Try the rest on a test
account. `check` reaches the account server without signing in, to test the setup.

## Checks

CI runs `cargo fmt --check`, `cargo clippy -D warnings` and `check` on Linux (x64 and arm64), macOS and
Windows, on every push and weekly.

## License

MIT. Copy the code into your own programs.
