# RustGram Roadmap

> The plan for moving a million lines of C++ to Rust without killing the patient.

This document defines the phases, decision gates, risks, and kill criteria for the
RustGram migration. It is a living document: every phase transition and every gate
decision updates it. If reality and this document disagree, reality wins and the
document gets rewritten.

Companion reading: [README.md](README.md) covers the why and the strategy in broad
strokes. This file is where the engineering honesty lives.

---

## Guiding invariants

These hold at every point in every phase. A change that violates one is rejected
regardless of how elegant it is.

1. **The client builds and runs after every merge.** No long-lived broken branch, no
   "rewrite" that replaces the original in one cutover. Strangler fig, not forest fire.
2. **Parity before deletion.** A C++ module is removed only after its Rust replacement
   has shipped, passed golden-vector and differential tests, and run in real usage.
3. **Debug-build velocity is sacred.** Upstream's rule (never build Release for dev)
   becomes ours with interest: if Rust integration slows the debug build beyond the
   budget, the integration is wrong, not the budget.
4. **Upstream mergeability is an asset with a price.** While it lasts, we track
   upstream tdesktop releases and measure merge cost. When drift exceeds value, we
   fork deliberately — as a recorded decision, not an accident.
5. **One bridge.** All Rust↔C++ traffic crosses through cxx-qt in a single, reviewed
   boundary crate. No ad-hoc `extern "C"` scattered through the tree.

---

## Phase overview

| # | Phase | Scale | Exit condition (summary) |
|---|-------|-------|--------------------------|
| 0 | Foundation | ~1–2 weeks | Rust crate compiled & linked into the shipping app; metrics baseline captured |
| 1 | Tooling & leaf modules | ~1–3 months | `.tl` codegen in Rust with byte-identical output; ≥3 leaf modules ported |
| 2 | Storage | ~3–6 months | Rust storage reads existing `tdata`, round-trips it, backs export |
| 3 | Protocol (MTProto) | ~6–12 months | Rust MTProto serves a live session: auth, updates, file transfer |
| 4 | Media pipeline | ~3–6 months | Images, animated stickers, video playback & streaming via Rust orchestration |
| 5 | UI shell | open-ended | Majority of new UI development happens in Rust; C++ UI is legacy maintenance |
| 6 | Calls | after 5 | Calling works through safe-Rust wrappers; pure-Rust WebRTC evaluated honestly |

Durations assume a small team working with AI-agent assistance. They are estimates
shaped like ranges precisely because point estimates would be lies.

---

## Phase 0 — Foundation

**Goal:** prove the pipeline, not the port. A Rust function runs inside the real app;
CI proves it on all three platforms.

**Entry criteria**
- Repository cloned; build instructions from upstream work unmodified (baseline builds green
  on Windows, Linux/Docker, macOS).

**Work items**
1. Rust workspace at `rust/` with the first crate: `rustgram-bridge` (cxx-qt).
2. CMake integration: the Telegram target builds `rustgram-bridge` via cargo and links
   the resulting static library. One entry point (`cmake/external/rust/`), reviewed
   like any other vendored dependency.
3. One real call: app startup logs a string produced by Rust (e.g. a build-fingerprint
   hash). Trivial, visible in every log — and proves the whole chain.
4. `rust-loc` script: counts Rust vs C++ LoC, emits the share metric.
5. CI: existing build matrix extended with the Rust toolchain; artifact caching so the
   debug build stays within the time budget.
6. ADR-0001 ("cxx-qt as the single bridge") written; `docs/adr/` process started.

**Exit criteria**
- Debug build green on all platforms **with** Rust linked in.
- CI time delta vs pre-Rust baseline ≤ +20%.
- The metrics baseline (build time, binary size, `rust-loc`) is recorded — future
  phases are judged against it.

---

## Phase 1 — Tooling & leaf modules

**Goal:** port code with deterministic output and no runtime risk. Build confidence,
tests, and tooling on easy targets.

**Work items**
1. **`.tl` scheme codegen in Rust.** The generator that turns
   `mtproto/scheme/*.tl` into C++ types is standalone, deterministic, and
   diff-testable. A Rust reimplementation must produce **byte-identical** output to
   the current generator. This single module de-risks the entire protocol phase: it
   forces us to understand every TL type before touching MTProto itself.
2. **Leaf modules**, selected by two filters: no Qt dependency, and low upstream churn.
   Candidates in rough order:
   - country codes / dial code parsing (`data/countries/`)
   - emoji suggestion trie (`chat_helpers/emoji_suggestions`)
   - langpack entity parsing (`langparser`)
   - URL/ Entities helpers that are pure string manipulation
   - `codegen/numbers`, `codegen/style` if the `.tl` port succeeds comfortably
3. **Differential test harness:** for each ported module, a runner that feeds the same
   inputs to C++ and Rust implementations and diffs outputs. Lives in
   `Telegram/SourceFiles/test/` per upstream convention; the golden vectors are
   committed.

**Exit criteria**
- `.tl` codegen: byte-identical on the full upstream scheme, wired into the build
  (Rust generator replacing the C++ one in the shipping pipeline).
- ≥3 leaf modules ported, each: differential tests green, C++ original deleted.
- Upstream merge cost measured: one real upstream release rebased, hours recorded.

**Kill check:** if byte-identical codegen proves infeasible or the differential
harness can't be made deterministic after honest effort, stop and re-plan — the
protocol phase depends on this rigor.

---

## Phase 2 — Storage

**Goal:** local data — settings, sessions, cache indices, export — served by Rust.

**Work items**
1. Map `storage/` + `main/` settings serialization: `QDataStream`-based sequential
   formats, the KV prefs facility, `tdata` layout (passcode-encrypted key file,
   map file, settings).
2. `rustgram-storage` crate: rusqlite-backed database layer mirroring the current
   schema, plus readers/writers for the binary formats.
3. **`tdata` compatibility is a hard requirement** for dev continuity: the Rust layer
   must read an existing profile created by the C++ client (so we can dogfood on real
   accounts) and write formats the same-version client can still read.
4. Port `export/` on top of the new storage layer — it's a pure consumer of stored
   data and the perfect first real customer.

**Exit criteria**
- App runs with storage served by Rust: settings persist, sessions resume, cache
  indexes intact.
- Round-trip test: C++ writes → Rust reads → Rust writes → C++ reads, byte-stable
  where the format demands it.
- Export produces identical output to the C++ exporter on a golden account snapshot.

---

## Phase 3 — Protocol (MTProto)

**Goal:** the beast. Network protocol stack in Rust, using our Phase-1 codegen.

**Decision gate (must be closed before this phase starts):** port tdesktop's own
`mtproto/` module versus adopting [grammers](https://github.com/Lonami/grammers).
The plan of record is **porting our own**, because:
- tdesktop's MTProto is entangled with its update model and session state; swapping
  the implementation changes behavior at the seams;
- grammers is bot/client-oriented and brilliant, but adopting it means adopting its
  update semantics wholesale;
- we already own byte-identical `.tl` codegen from Phase 1 — the marginal cost of
  porting is lower than the integration risk of swapping.

This decision is revisited once, at phase start, with the full research in hand.

**Work items (in order)**
1. Crypto primitives & RSA/DH key exchange — pure functions, aggressively unit-tested
   against fixed vectors from the MTProto spec.
2. Transports: TCP, obfuscated TCP, websocket; the connection/DC-switch machinery.
3. Serialization: generated TL types in Rust (the codegen now targets Rust too —
   dual-target generation, C++ and Rust, from the same `.tl`).
4. Session & update state: pts/qts sequencing, channel difference handling.
5. File upload/download pipeline (the part tdesktop's media streaming sits on).

**Safety rails**
- Live-DC testing uses **test accounts only**, never personal accounts. Ban risk from
  a broken client is real; the upstream AGENTS conventions and testing discipline
  exist for a reason.
- Recorded-session replay tests: captured byte streams are replayed against the Rust
  stack offline.

**Exit criteria**
- A full session through the Rust stack: auth, dialog load, message send/receive,
  media transfer, surviving a DC migration.
- Zero protocol-level regressions vs C++ in a two-week dogfood window.

---

## Phase 4 — Media pipeline

**Goal:** images, animated stickers, video, and streaming orchestrated by Rust.

**Honest scope note:** "in Rust" here means **safe-Rust orchestration over proven
codecs**. We do not rewrite FFmpeg. Decode/encode stays FFI (ffmpeg-next or
gstreamer-rs — decision recorded as an ADR); what we own is the pipeline: caching,
pre-decode sizing, streaming with seek, frame scheduling, GPU upload paths.

**Work items**
1. Image pipeline: `image` crate for decode/encode, thumbnailing, progressive JPEG,
   WebP/AVIF coverage parity.
2. Lottie stickers: rlottie via FFI behind a safe wrapper (pure-Rust Lottie players
   are not production-ready; we watch them, we don't bet on them).
3. Video: playback and network streaming with seek — the media streaming state
   machine is the actual hard part and it is squarely ours.
4. Animated emoji, custom emoji cache, sticker set management on the new pipeline.

**Exit criteria**
- Sticker playback and video streaming in daily use without frame-scheduling
  regressions (jank measured, not vibed).
- Media cache hit rate and memory profile at parity or better with baseline.

---

## Phase 5 — UI shell

**Goal:** invert the ratio. New UI development happens in Rust; C++ UI becomes
frozen legacy.

This phase is open-ended by design and is the one most likely to be re-scoped.
The end-state question — *Qt shell forever* vs *full Rust toolkit* (iced/egui/…)
— is explicitly **not** decided here. It gets its own gate when Phase 4 is done and
the ecosystem landscape is re-surveyed. The likely long-term answer is a hybrid:
Qt-hosted custom widgets whose logic and painting live in Rust, migrated
widget-family by widget-family.

**Work items**
1. Widget inventory: the Qt widget tree is cataloged with usage-frequency and
   entanglemetry (upstream churn × dependency degree) — the migration order falls
   out of this inventory, leaf widgets first.
2. Chat view internals first (it's where the app lives): message layout engine,
   text rendering via cosmic-text, the virtualized list. Hardest and highest-value.
3. Settings, dialogs, boxes — wide but shallow.
4. Theming: the `.style` system gets a Rust-side equivalent so ported widgets stop
   depending on generated C++ style headers.

**Exit criteria**
- Majority of new UI commits touch Rust, measured quarterly.
- A vertical slice (one complete screen) fully Rust: composed, themed, shipped.

---

## Phase 6 — Calls

**Goal:** calling without C++ at the boundary.

Pragmatic plan of record: tgcalls stays (it's a Telegram-maintained libwebrtc fork
with extra encryption — nobody should rewrite that), wrapped in a safe-Rust FFI
layer with the session/lifecycle state owned by Rust. Pure-Rust WebRTC
(webrtc-rs) is evaluated at phase start with fresh data; it is a stretch goal,
not a dependency.

**Exit criteria**
- One-on-one and group calls placed and received through the Rust wrapper layer,
  stable across a dogfood window.

---

## Decision gates

Five decisions that block code until they're made, each recorded as an ADR:

| # | Decision | When | Plan of record |
|---|----------|------|----------------|
| 1 | Bridge technology | Phase 0 | cxx-qt, single boundary crate |
| 2 | MTProto: port vs adopt | Phase 3 start | Port our own, byte-compat via Phase-1 codegen |
| 3 | UI end-state | After Phase 4 | Open — re-survey ecosystem then |
| 4 | `tdata` compatibility | Phase 2 | Hard requirement: must read existing profiles |
| 5 | Upstream tracking policy | Phase 1 exit | Track releases while merge cost < 4h; fork deliberately when it exceeds |

---

## Risk register

Ranked by expected pain. Every risk names its tripwire.

1. **Upstream velocity.** tdesktop moves fast; every upstream change to a migrated
   module is rework. *Tripwire:* merge cost > 4h per release two releases in a row →
   close gate 5 and fork deliberately.
2. **Bridge lifetime bugs.** Rust↔Qt object lifetimes are the classic footgun;
   use-after-free here is worse than the C++ status quo. *Tripwire:* any
   bridge-attributed crash in a shipped build → freeze migration of new modules
   until the ownership rules are hardened and fuzzed.
3. **Build complexity doubles before it halves.** CMake+cargo+Qt is three build
   systems in a trench coat. *Tripwire:* CI time budget (+20%) exceeded →
   integration gets fixed before any new porting.
4. **MTProto misimplementation → account bans.** A wrong client can trip server-side
   abuse detection. *Tripwire:* any unexpected AUTH_KEY_DUPLICATED / flood-ban on a
   test account → halt live testing, replay-test offline until explained.
5. **Hot-path performance regressions.** Chat-list rendering and media decode are
   latency-sensitive. *Tripwire:* benchmark gate in CI; a merged regression is a
   revert, not a "we'll optimize later."
6. **Multi-year energy budget.** This outlasts enthusiasm; phases must ship
   independent value. *Tripwire:* defined in kill criteria below.
7. **cxx-qt maturity gaps** for deep Qt Widgets integration. *Tripwire:* if a needed
   feature is missing upstream, we contribute it or fall back to a hand-written seam
   — never scatter raw FFI.
8. **Windows toolchain pain.** MSVC + Qt + Rust is the weakest-supported combo of
   the three platforms. *Mitigation:* Windows is in CI from Phase 0 day one.
9. **Codec/Lottie depth.** Pure-Rust replacements immature for video and Lottie.
   *Mitigation:* already scoped as FFI; watch, don't bet.
10. **License drift in dependency swaps.** GPLv3 travels with the code; new crates
    must be license-compatible. *Mitigation:* cargo-deny audit in CI.

---

## Kill criteria

Written now, while nobody is tired, so future-us can't rationalize:

- **Phase 1 gate:** if byte-identical `.tl` codegen and a deterministic differential
  harness can't be achieved, the methodology itself is broken — stop entirely.
- **Estimate blowout:** any two consecutive phases exceeding 3× their upper estimate →
  the remaining phases are re-planned or the project freezes at its current shipped
  value. Freezing with storage+protocol in Rust is a *success state*, not a failure.
- **Bridge velocity:** if for 3+ months the merge/PR velocity of Rust-touched modules
  is measurably worse than the C++ baseline, the strategy degrades to
  Rust-for-new-modules-only — recorded, not mourned.
- **The patient dies anyway:** if the app stops shipping green builds for a period
  measured in weeks (not days) due to migration complexity, unwind the last step,
  re-green, and shrink the step size.

The project is allowed to end early at any phase boundary with everything shipped so
far intact. The only unacceptable ending is a broken client and an unfinished rewrite.

---

## Metrics

What we measure, continuously, from Phase 0:

| Metric | Source | Budget / target |
|--------|--------|-----------------|
| Rust LoC share | `rust-loc` script | monotonically increasing; reported per phase |
| Debug build time delta vs baseline | CI | ≤ +20% during migration; aspiration −30% end-state |
| Upstream merge cost | per-release log | < 4h through Phase 2 |
| Differential test pass rate | per ported module | 100% before C++ deletion — no exceptions |
| Crash rate (bridge-attributed) | crash reporting | zero tolerance; any instance freezes new migration |
| Binary size delta | CI artifact | informational; no budget while migrating |

---

## Changelog

- **2026-09-11** — Initial roadmap drafted at project inception. Phases 0–6 defined,
  gates, risks, and kill criteria recorded before any code was written.
