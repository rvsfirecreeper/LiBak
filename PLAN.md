# LiBak design plan (draft, revised 2026-11-13)

Consolidated from the architecture discussion. Items marked DECIDED were chosen by the project owner. Items marked RECOMMENDED were suggested during discussion and not yet confirmed. Items marked OPEN need a decision.

## 1. Goals and constraints

- Encrypted backup tool, fast, usable by non-technical people, UI kept small.
- Must not re-encrypt the whole backup on every change.
- AI assistance is limited to high-level architecture, bugfixing, and UI. Nothing else.
- No cloud service is assumed, so all security comes from client-side encryption and the user's password.
- Security is preferred over convenience, with one stated exception: convenience may win where it does not require holding a decryption key in memory for long periods.
- Avoid web-stack UI technology (Electron-like ecosystem, including webview-based frameworks).

## 2. Decisions at a glance

| Area | Decision | Status |
|---|---|---|
| Language | Rust (owner is very comfortable with it; better key hygiene via zeroize; reuse of existing DEK/KEK streaming encryptor) | DECIDED |
| UI | Slint (native rendering, no webview, no npm chain). Replaces the earlier Tauri choice | DECIDED |
| License | GPLv3 (satisfies Slint's GPLv3 option, see section 12) | DECIDED |
| Structure | Core library + CLI + daemon. The UI calls the library for foreground work and talks to the daemon over local IPC for schedule and status | DECIDED (daemon re-added 2026-11-13, IPC details RECOMMENDED) |
| Daemon | Re-added 2026-11-13. Resident scheduler (and optional file-watch hint) that holds no keys and never decrypts. Notifies the UI when a backup is due. See section 3 and section 5 | DECIDED (re-added). Key-free scope RECOMMENDED, confirm |
| Change detection | Periodic scan (mtime, size, inode vs last snapshot). File watching, if added, is only a hint to the daemon | DECIDED |
| Chunking | FastCDC (content-defined chunking) | DECIDED |
| Cipher | AES-GCM, random nonce per chunk | DECIDED |
| Key hierarchy | Argon2-derived KEK wraps DEK | DECIDED |
| zeroize | Mandatory for all key and secret handling | DECIDED |
| Unlock | Password required each run, prompted on a timer | DECIDED |
| Export | Archive of the store for external drive or cloud | DECIDED (format see section 7) |
| Quantum | Avoid classical-only public-key crypto. Use ML-KEM (hybrid) if a public-key mode is ever added | DECIDED |
| Encrypted manifest | Manifest is encrypted and authenticated | RECOMMENDED (owner agreed) |

## 3. Architecture

- **Core library (Rust)**: scanning, chunking, compression, encryption, store, restore, verify, prune. All logic lives here.
- **CLI**: thin wrapper over the library, shipped first.
- **Daemon (re-added 2026-11-13)**: small resident process that holds no secrets and never decrypts anything.
  - Owns the backup schedule, replacing the earlier idea of the UI running its own prompt timer.
  - Decides when a backup is due and tells the UI, which shows the "backup due" notification and the unlock dialog (section 5).
  - Optionally runs a file watcher as a hint that something changed. The scan stays the source of truth (section 9).
  - Uses the library for scanning and change detection only. Any step that needs the DEK happens in a foreground run after the user unlocks, then the key is zeroized.
  - IPC (RECOMMENDED): a local Unix domain socket (named pipe on Windows), no network port, with a tiny request/response protocol (status, backup due, pause, resume). The owner has no IPC experience, so keep the protocol minimal and well tested. Restrict the socket to the owner's user.
  - Not launched through systemd timers (ruled out by the owner). How it starts is OPEN (see section 11).
- **Slint UI**: asks what to back up, shows status, prompts for the password, triggers backups and exports. Calls the library directly for foreground work and listens to the daemon for schedule events. Keep it thin so the toolkit stays replaceable.
- The backup worker is written as a function with no resident state, so the daemon (or a later keychain worker) can invoke it without a rewrite.

### Backup pipeline

1. Scan the selected folders, applying exclusion rules.
2. Compare each file's path, size, mtime and inode with the previous snapshot. Only changed files are read.
3. Split changed files with FastCDC (suggested range roughly 512 KB to 8 MB, tune later).
4. For each chunk: compute a keyed hash of the plaintext (chunk ID). If that ID already exists in the store, skip it.
5. For new chunks: compress, then encrypt. The order matters: hash first, then compress, then encrypt, otherwise dedup breaks.
6. Group new chunks into pack files (avoids millions of tiny files).
7. Write an encrypted snapshot manifest: file list, metadata, ordered chunk IDs per file.

Dedup is a property of the chunk store and works across files, versions, renames and moves. Unchanged data costs no encryptions.

### Store layout

- Plain directory of immutable pack files plus encrypted manifests plus a small plaintext header.
- Plaintext header contains only: format version, scheme IDs, Argon2 parameters, salt, wrapped DEK.
- A snapshot is just a manifest, so taking one is nearly free.

### Pruning

Deleting old snapshots requires garbage collection of unreferenced chunks. Design for this from the start since it is the hardest part to add later. Repacking should copy ciphertext where possible rather than re-encrypt.

## 4. Cryptography

### Key hierarchy (DECIDED)

- Password goes through Argon2id to produce the KEK.
- KEK wraps the DEK. Changing the password only re-wraps the DEK and never touches data.
- Argon2id parameters are expensive (hundreds of MB, around a second per attempt) and stored in the header so they can be raised later.
- Encourage generated diceware passphrases. Optional recovery key using the owner's Shamir implementation.

### Nonce safety (RECOMMENDED)

Owner chose AES-GCM with random nonces. The 2^32 encryptions-per-key guidance is real: with 4 MiB chunks it is about 16 PiB, reachable only if almost all data is new on every run. Dedup makes the realistic count far lower (roughly 287 years at 1% daily churn on 15.6 TiB), but the cap is conservative for good reason, so a safeguard is wanted. The trigger is the number of encryptions, not elapsed time, so no UI warning or user-facing "rotate key" nag. Preferred approach:

- **Per-pack subkeys**: derive a subkey per pack with HKDF from the DEK and a random salt stored in the pack header. Each key encrypts only a few hundred chunks. No counter or state to corrupt.
- Alternative: derive the chunk key as HKDF(DEK, chunk_id). Each key then encrypts one distinct plaintext. No stored keys. Keep a random nonce in the header anyway.
- Alternative: key epochs with an automatic counter threshold (for example 2^30). New data uses a new DEK, old data stays under the old DEK, nothing is re-encrypted.
- XChaCha20-Poly1305 (192-bit nonces) remains an option if AES-GCM nonce handling is ever a concern.
- A manual "rotate key" button is only for the user's own reasons (suspected leak) and that case does require re-encryption.

### Rejected for v1: per-file intermediate KEK

The 3-layer idea (master KEK, per-file KEK, per-chunk DEK) was considered. With dedup it requires a wrapped key per chunk reference (roughly 200 MB extra manifest for a 15 TiB store) and complicates pruning. Its benefits (per-file sharing, crypto-shredding) are not requirements and dedup defeats shredding. Revisit only if single-file sharing becomes a goal.

### Other crypto rules (RECOMMENDED)

- Chunk IDs use a keyed hash with a key separate from the encryption key (prevents confirmation-of-known-file attacks, keeps dedup working).
- Bind chunk ID and pack header in as AEAD associated data so chunks and packs cannot be swapped or relabeled.
- Manifest is encrypted and authenticated. Leaving it plaintext would leak file names, structure, sizes and timestamps and allow undetected tampering.
- Use 256-bit keys throughout (quantum margin for symmetric crypto).
- Crypto agility: algorithm and scheme IDs plus a format version in the header and in every pack, so primitives can be migrated gradually.
- zeroize all secrets. Consider mlock and disabling core dumps while a key is resident.
- Password entry limit: the password passes through the UI toolkit's text-field buffer, which cannot be zeroized. Copy it into a zeroizing buffer immediately and clear the field. This is a known limit of the "zeroize everything" goal.

## 5. Unlock and automation model

### v1 (DECIDED)

- The daemon is key-free (section 3). No key is resident between runs, in the daemon or anywhere else.
- The user enters the password, the key is derived, the backup runs, the key is zeroized.
- Timed backups are driven by prompting the user on an interval. The daemon keeps the schedule and signals the UI. Prefer a "backup due" notification that opens the unlock dialog over a focus-stealing modal, use idle detection to avoid interrupting typing, and use hourly or daily prompts rather than 15 minutes. When unlocked, run everything pending.
- Known costs: no backups while the user is away, and prompt fatigue (people get trained to type passwords into unexpected dialogs).
- systemd timers are ruled out by the owner. The daemon is the scheduler, so the app does not rely on OS schedulers.

### Later options (OPEN, not committed)

1. **Short-lived worker spawned by the daemon**: the daemon (which already holds no secrets) launches a worker that fetches the DEK from the OS keychain, runs, zeroizes, exits. Weak spot: headless Linux without a Secret Service needs a fallback. This is the only path to unattended backups while keeping the "no long-lived key" rule.
2. **Write-only public-key mode**: the unattended worker holds only public keys, so it can encrypt but never decrypt. See section 6.

A daemon that itself holds the DEK or KEK in memory remains rejected (section 9).

## 6. Optional post-quantum write-only mode (OPEN)

Only needed if unattended backups are ever wanted without a decryption key in memory. The passphrase-only symmetric mode does not need any public-key crypto.

- Hybrid KEM: X25519 plus ML-KEM-1024. A hybrid means a bug in an unaudited implementation or a lattice break does not alone expose data.
- Each pack and manifest header carries a fresh encapsulation (about 1.6 KB for ML-KEM-1024 plus a 32-byte X25519 ephemeral key). Both shared secrets are combined with HKDF, binding both ciphertexts and both public keys, to derive the pack key. Every pack has a unique key, so there is no DEK in memory.
- Header scheme IDs, for example `passphrase-v1` and `hybrid-x25519-mlkem1024-v1`, let both modes share one format.
- Keys: Argon2id output goes through HKDF to produce the 64-byte ML-KEM seed and the X25519 secret. Public keys live in the repo header and on the worker. On restore, re-derive and check against the stored public keys (also a password check).
- Caveat: the public key acts as a password verifier, so offline guessing is possible with the header, as with the wrapped DEK in symmetric mode. Mitigate with a strong passphrase or by mixing a random recovery secret into the seed.
- Crate notes: RustCrypto `ml-kem` implements final FIPS 203 (formerly Kyber), is not wire-compatible with old Kyber drafts, is pre-1.0 (0.3.x, pin the version), and its own docs state it has never been independently audited. Private keys are best stored as the 64-byte seed. Avoid the deprecated expanded key format. Avoid older Kyber-named crates.
- A worker in this mode still needs a local metadata cache for change detection, since it cannot read previous manifests, and it holds the chunk-ID key.

## 7. Export and portability

- Owner's proposal: archive the store as a `.tar` for an external drive or cloud.
- Recommendation: keep the store as immutable pack files so rsync, rclone or a plain copy only move new data. Offer `.tar` as an optional convenience for one-off archives rather than the primary mechanism, because tarring the whole store each time recopies everything.
- Export of selected snapshots means copying the referenced packs plus index. Import is a safe merge because chunk names are content-derived. Repositories only share chunks if they share the same ID key.

## 8. Exclusion rules

- gitignore-style rule file using the `ignore` crate (ripgrep's implementation, supports nested rule files).
- Ship presets because a blank rules file is hostile to average users: developer (`target/`, `node_modules/`), gaming (Steam library, shader caches), system junk (caches, trash, temp).
- Honor `CACHEDIR.TAG` markers.
- Show users what was excluded and its total size so nothing is silently unprotected.
- Large files are handled by chunking, not exclusion.

## 9. Dropped ideas and why

- **Daemon (dropped 2026-11-08, re-added 2026-11-13)**: originally dropped to avoid IPC and a resident process. Now back as a key-free scheduler and notifier (section 3). The file watcher inside it stays optional: a watcher can miss events (for example inotify limits and overflow), so a scan is required anyway, and an incremental scan on metadata is cheap. A watcher would only improve latency and battery. If added, treat the watcher as a hint and the scan as the source of truth.
- **Long-lived key-holding daemon**: still rejected. It conflicts with the goal of keeping keys resident for as short a time as possible. The re-added daemon must not hold the DEK or KEK.
- **Classical-only public-key crypto**: would not survive a large quantum computer.
- **User-facing key rotation warning**: wrong mechanism, safety should be automatic.
- **Tauri UI**: webview and npm dependency chain conflict with the goal of avoiding the Electron-like ecosystem, and add attack surface to the component that handles the password. Replaced by Slint.

## 10. Suggested build order

1. Chunk store, encrypted manifests, timed snapshots (local disk target only).
2. Restore and verification (whole-repository integrity check without full decrypt where possible).
3. Pruning and garbage collection.
4. Export.
5. CLI polish, then the daemon (schedule, backup-due signal, minimal local IPC), then Slint UI (password prompt, folder picker, exclusion presets, notifications).
6. Optional: keychain worker spawned by the daemon, then write-only hybrid mode, then the file-watch hint in the daemon.

## 11. Open questions

- AES-GCM per-pack subkeys vs chunk-ID-derived keys vs epochs for nonce safety (recommendation: per-pack subkeys).
- Target chunk size range and pack size.
- Prompt interval default and idle-detection behavior.
- Whether to ship a recovery key (Shamir) in v1.
- Whether and when to add the keychain worker or write-only hybrid mode.
- Cloud or remote targets: out of scope for v1, local disk only.
- Daemon scope: confirm it stays key-free (scheduler and notifier only), or whether the owner wants it to do more.
- Daemon lifecycle: how it starts (user-session autostart entry vs launched by the UI on first run), how it stops, and what happens to schedules when it is not running.
- Whether the file-watch hint ships with the daemon or waits until after v1.

## 12. Licensing notes

- LiBak is GPLv3. Slint is offered under GPLv3, a royalty-free license, or a commercial license. The GPLv3 option applies because LiBak is itself GPLv3. Re-check the current terms on slint.dev before the first release.
- Every dependency must be GPLv3-compatible. The RustCrypto crates (MIT or Apache-2.0) are compatible. Run `cargo deny` with a license allowlist in CI so an incompatible dependency cannot slip in.
- Include the license text, a copyright header policy, and source availability for any distributed binaries.

## Change log

- 2026-11-08: initial plan.
- 2026-11-08: UI changed from Tauri to Slint. License set to GPLv3. Added password-buffer caveat and licensing notes.
- 2026-11-13: daemon re-added as a key-free scheduler and notifier with local IPC. Structure, architecture, unlock model, dropped ideas, build order and open questions updated to match. Key-holding daemon stays rejected.
