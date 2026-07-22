# AGENT.md — Project Genesis & Build Rules

**Phase 0 — Project Genesis & Build Rules**

This file is the persistent source of truth for every agent working in this
repository. Re-verify it exists at the repository root on every session start.
Do **not** begin Phase 1 until the user explicitly activates it.

---

## Multi-agent triad (always live)

Every coding session initializes and consults this triad. No single agent
overrides the others without an explicit Strategic Arbiter resolution.

### 1. Strategic Arbiter

- Owns architecture, module boundaries, long-term coherence, and phase gates.
- Breaks ties when Security and Performance conflict, or when two designs
  both satisfy local rules.
- Prefers clear domain seams, explicit naming, reversible decisions, and
  lower human cognitive load.
- Rejects “clever” structure that does not buy a type-system or product win.

### 2. Security Agent

- Hunts vulnerabilities: injection, auth gaps, SSRF via user-supplied URLs,
  secret leakage in logs/UI, unsafe path handling, privilege confusion across
  multi-user media servers.
- Demands fail-closed defaults where failure mode matters (mapping, allowlists,
  ignore lists, dry-run vs apply).
- Flags any change that weakens honest UI or silent-overwrite behavior.
- When it finds an issue under a critique phase: **fix or PASS** — no
  complaint-only findings.

### 3. Performance & Devil’s Advocate Agent

- Enforces **zero-cost abstractions**: prefer types, enums, and the compiler
  over runtime ceremony.
- Challenges unnecessary allocations, clones, locks held across `.await`,
  redundant network round-trips, and “sync everything again” when skip-if-equal
  is enough.
- Opposes complexity that does not buy correctness, latency, or clarity.
- When it finds an issue under a critique phase: **fix or PASS** — no
  complaint-only findings.

**Operating rule:** Surface dissent from each role when relevant. If roles
disagree, the Strategic Arbiter decides and records the rationale (commit
message or PR notes). **Security outranks Performance** when both cannot be
satisfied without compromise.

---

## Build rules (locked)

### Language and first principles

1. **Code strictly from first principles using only Rust** for application
   logic. Prefer the compiler’s type system to eliminate error classes
   (enums over stringly state, `Result`/`Option` over sentinels, newtypes at
   domain boundaries).
2. Do not introduce non-Rust application languages for core product logic.
   Embedded HTML/JS/CSS for the dashboard is allowed only as data/strings
   generated from Rust (e.g. maud / `const` fragments), not a parallel stack.

### RFC / protocol compliance

3. **Strict RFC compliance** across implementations that speak standards:
   HTTP (status codes, methods, caching headers where applicable), JSON
   shapes, WebSocket framing as used by Emby/Jellyfin, URI/URL handling, and
   sensible Content-Type / CSP for the dashboard.
4. Prefer documented Emby/Jellyfin API contracts over guesswork; fail clearly
   when upstream responses violate expectations.

### License

5. **Apache License 2.0** is the project license (explicit patent and
   trademark protection). Keep `LICENSE` at the repository root and
   `license = "Apache-2.0"` in `Cargo.toml`. Do not relicense without an
   explicit user decision.

### Cognitive load and file structure

6. **Hard 250-line limit per `.rs` file** (production and tests). If a file
   would exceed 250 lines, split it before landing the change.
7. **Split exclusively at logical function / domain boundaries** — never
   arbitrary mid-function cuts.
8. **Explicit, domain-specific file naming** (e.g. `force_played_pair.rs`,
   `poster_proxy.rs`, `url_safety.rs`). No `utils2.rs`, `misc.rs`, or
   `helpers2.rs`.

### Observability

9. **Structured logging** on critical paths (connect, auth failures, sync
   apply/skip, force phases, clear-watched, config save/reload, fatal
   errors). Use `tracing` with levels appropriate to noise (`debug` for
   hot loops, `info` for operator-visible events, `error` for failures).
   Never log API keys, tokens, or raw secrets.

### Testing and dead code

10. **Write unit and integration tests concurrently** as code is built —
    not as a late afterthought. Maximize leverage on matching, sync, config,
    force-skip, UserData, and URL safety. Reserve end-to-end tests for
    critical user paths (dashboard smoke, force start, health).
    For RFC/protocol surfaces (URL normalize, SSRF, path IDs, auth compare,
    force equality windows), use **property-based tests** (`proptest`) in
    addition to positive / negative / boundary unit cases.
11. **Eliminate dead code**: unused modules, stubs, empty re-exports,
    placeholder tests, lying UI controls.

### Quality bar (must pass locally before later phase gates)

12. Strictly pass:
    - `cargo fmt --all -- --check`
    - `cargo clippy --all-targets -- -D warnings`
    - `cargo test --all-targets`
    - `cargo audit` (document allowed advisories if any)
    - `cargo +nightly udeps --all-targets` when available

### Brand and org visual style

13. **Icons:** Follow [graphics/BRAND.md](graphics/BRAND.md). studio2201 app
    icons are square neon **line glyphs** on dark navy/charcoal, 2–3 colors
    (cyan/green/purple), **no text**. Canonical StateSync icon is
    `graphics/statesync_icon.jpg` only — never replace with character art.
14. **Headers/mascots:** Allowed for README banners only; must stay separate
    from the app icon. No embedded text in any image asset.
15. **Reference ground truth:** sibling org icons under
    `../pulse/assets/icon.png`, `../beam/assets/icon.png`, etc., when present
    on disk. Prefer matching those over inventing a new icon language.

### Product domain (StateSync)

- StateSync syncs **watched**, **resume**, and **favorites** across
  Emby/Jellyfin (and same-type pairs). It does not move media files.
- Prefer honest UI: no controls that do nothing. Prefer skip-if-equal and
  clear storytelling over silent rewrites.

---

## Phase gates

| Phase | This file’s stance |
|-------|--------------------|
| **0 (this document)** | Lock rules + triad + confirm local build → **halt**. Await explicit **Phase 1**. |
| **Later phases** | Only when the user explicitly activates them. Do not skip ahead. |

---

## Enforcement checklist (every Phase 0 run)

- [ ] `AGENT.md` present at repo root and current
- [ ] Multi-agent triad treated as session-active
- [ ] Apache-2.0 `LICENSE` + `Cargo.toml` license field
- [ ] Rust toolchain available; `cargo check` / project builds
- [ ] **Stop** — await explicit Phase 1 activation before feature work
