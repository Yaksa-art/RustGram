# RustGram

> Telegram Desktop, rewritten in Rust — one module at a time.

![Status](https://img.shields.io/badge/status-research%20%2F%20experimental-orange)
![License](https://img.shields.io/badge/license-GPLv3%20%2B%20OpenSSL%20exception-blue)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey)

RustGram is an experiment in migrating **Telegram Desktop** — roughly a million lines of
battle-tested C++ built over more than a decade — to **Rust**, without ever breaking the
client along the way.

It starts life as a fork of [telegramdesktop/tdesktop](https://github.com/telegramdesktop/tdesktop).
It ends life — if it gets that far — as a Rust application with none of its own C++ left
in the tree.

## Why

- **Memory safety where it matters most.** A messenger spends its whole day parsing
  untrusted data from the network: messages, stickers, documents, calls. That is exactly
  the workload Rust was built for.
- **A build that fits in one file.** tdesktop needs a sibling `Libraries/` tree of
  prebuilt dependencies, a specific Qt, a specific toolchain, and patience. Cargo
  dependency resolution would replace a small novel of build documentation.
- **Dependency hygiene.** The upstream tree vendors dozens of patched third-party
  libraries. Crates.io and cargo audit give us a real dependency story: versions,
  advisories, reproducible builds.
- **Because it's there.** The honest reason. It's the most interesting systems-programming
  mountain visible from here, and the view from the top should be spectacular.

This is not a criticism of tdesktop. It's a love letter written in a different language.

## Strategy: strangler fig, not forest fire

Big-bang rewrites of working software die alone in a branch. RustGram migrates the way a
strangler fig grows — around the living tree, module by module:

1. **The client stays shippable.** After every merge, the app builds and runs on all
   supported platforms. There is no point where "the rewrite" replaces "the original".
2. **Rust enters through a bridge.** [cxx-qt](https://github.com/KDAB/cxx-qt) lets Rust
   code live inside the Qt application and Qt types cross the boundary safely. New code
   is Rust; old code leaves one module at a time.
3. **Leaf modules first, UI last.** Pure logic with no Qt dependency (parsers, utilities,
   build tooling) moves first. Storage, protocol, and media follow. The UI shell — the
   largest and most Qt-entangled surface — moves last, if at all.
4. **Parity before deletion.** A module's Rust replacement ships behind the same
   interface, passes the same tests (and new ones), and runs in production before the
   C++ original is deleted. Golden-vector and differential tests keep the two honest.
5. **The .tl codegen goes first.** The scheme compiler that turns Telegram's TL definitions
   into C++ is a standalone build tool with deterministic output — the perfect first
   target. A Rust reimplementation that produces byte-identical output proves the whole
   pipeline with zero runtime risk.

## Status

| Phase | Scope | State |
|-------|-------|-------|
| 0 | Recon, toolchain, CI, first Rust frame on the stack | not started |
| 1 | Build tooling & leaf modules (codegen, parsers, utils) | not started |
| 2 | Storage layer (SQLite, local cache, export) | not started |
| 3 | Protocol stack (crypto, serialization, MTProto) | not started |
| 4 | Media pipeline (images, Lottie, codecs) | not started |
| 5 | UI shell (cxx-qt widgets, progressive replacement) | not started |
| 6 | Calls (tgcalls / webrtc strategy) | not started |

Current Rust share of the codebase: **0%**. That number has nowhere to go but up, and
tracking it is part of the fun — see [ROADMAP.md](ROADMAP.md) for entry and exit
criteria per phase, the honest risk register, and the explicit kill criteria.

## Building

Until the migration produces its own build story, RustGram builds exactly like upstream
tdesktop. Follow the [official build instructions](docs/building-msvc.md) for your
platform:

- **Windows** — Visual Studio 2022, `cmake --build out --config Debug --target Telegram`
- **Linux / Docker** — `Telegram/build/docker/centos_env/build_debug.sh`
- **macOS** — Xcode, see `docs/building-osx.md`

Debug builds only for development — Release is extremely heavy and unnecessary for
testing changes. When the first Rust workspace lands (Phase 0), a Rust toolchain
becomes a build prerequisite and this section grows a `cargo` story.

## Repository layout

```
Telegram/          # tdesktop source (C++, shrinking over time — in theory)
rust/              # Rust workspace: bridge crates + ported modules (Phase 0)
docs/              # build & platform documentation
.claude/ .agents/  # AI-assisted development tooling (see below)
```

## AI-assisted development

This repository is developed with AI agents in the loop, inheriting the upstream task
queue convention: discrete units of work live as records in a sibling `ai-tdesktop`
repository (`tasks/YYYY/MM/DD/<slug>/` with `task.md` + `state.yaml`), and commits
reference them by task id. Agents working in this tree should read `AGENTS.md` and
`CLAUDE.md` first — they define the task vocabulary, commit format, and build rules.

## Roadmap

The full plan — phases, decision gates, risks, and the metrics that tell us whether
this is working — lives in [ROADMAP.md](ROADMAP.md).

## License

GPLv3 with OpenSSL exception, inherited from and derivative of
[Telegram Desktop](https://github.com/telegramdesktop/tdesktop). A rewrite in another
language is still a derivative work; the license travels with it, as it should.

## Acknowledgments

- [telegramdesktop](https://github.com/telegramdesktop) and every contributor over the
  years — this project stands on their work and would be meaningless without it.
- [KDAB](https://github.com/KDAB) for cxx-qt, the bridge that makes this plan sane.
- [grammers](https://github.com/Lonami/grammers) — pure-Rust MTProto, and a
  proof that the protocol layer is portable.
- The Rust community, for building the toolchain this whole bet is placed on.
