# CLAUDE.md — AionDB
## Distributed Bitemporal Database Engine in Rust · Project Mentor Instructions

> **Aion (Αἰών)** — Greek deity of eternity and cyclical time, distinct from Chronos (linear time).
> AionDB manages two orthogonal time axes (valid time × transaction time) across a
> strongly-consistent distributed cluster — purpose-built for financial-grade workloads.

---

## ⚡ Quick Reference (Claude Code Operational Section)

```
Project:    AionDB — distributed bitemporal database, financial-grade consistency
Language:   Rust (stable toolchain)
Workspace:  Cargo workspace — 5 crates (see map below)
Test cmd:   cargo nextest run
Lint cmd:   cargo clippy -- -D warnings
Format cmd: cargo fmt --check
Bench cmd:  cargo criterion
CI:         GitHub Actions — fmt → clippy → test → audit → deny
```

### Crate Map
```
aiondb/
├── crates/
│   ├── aiondb-core/        # storage engine, temporal model, MVCC, HLC, indexes
│   ├── aiondb-consensus/   # Raft implementation (stubbed Phase 0, built Phase 10)
│   ├── aiondb-server/      # axum HTTP server, query routing, cluster membership API
│   ├── aiondb-client/      # embedded Rust client library (crates.io)
│   └── aiondb-sql/         # SQL parser, lexer, AST, query planner
├── benchmarks/             # criterion benchmark suites
├── docs/
│   ├── adr/                # Architecture Decision Records (mandatory)
│   ├── grammar.ebnf        # Formal SQL dialect grammar
│   └── openapi.yaml        # REST API spec (OpenAPI 3.1)
└── deploy/
    ├── docker/
    ├── terraform/
    └── helm/               # includes cluster CRD and operator
```

### ⚠️ Distribution-Aware Design Rules (apply from Phase 1 onwards)
- WAL entries MUST include `term: u64` and `log_index: u64` — required for Raft
- ALL timestamps MUST use Hybrid Logical Clock (`HLC`) — never bare `SystemTime`
- Record identity MUST include `node_id` — wall-clock uniqueness is not sufficient
- `aiondb-consensus` crate MUST exist in workspace from Phase 0 (even if stubbed)
- No API that assumes single-node ownership of data — design for quorum from day one

### Non-negotiable Engineering Rules
- No `unwrap()` in library code — use `?` and typed errors (`thiserror`)
- All `unsafe` blocks require `// SAFETY:` comment explaining the invariant
- No `clone()` without an explanatory comment
- All public items need `///` doc comments
- Conventional commits: `feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `chore:`
- Tests must pass and clippy must be clean before any commit
- Every optimisation requires a benchmark proving the problem first

### Full mentor instructions: @docs/MENTOR.md

---

## 🛠️ Claude Code Setup Guide

### Step 1 — Install Claude Code

```bash
node --version          # must be 18+
npm install -g @anthropic-ai/claude-code
claude --version        # verify
```

### Step 2 — Authenticate

```bash
claude                  # opens browser auth flow on first run
# authenticate at console.anthropic.com
```

### Step 3 — Global Personal Preferences

Claude Code reads two files: a **global** `~/.claude/CLAUDE.md` (all projects) and this
**project** `CLAUDE.md`. Create your global file:

```bash
mkdir -p ~/.claude && touch ~/.claude/CLAUDE.md
```

Recommended `~/.claude/CLAUDE.md`:

```markdown
# Personal Preferences

## Engineering
- I am a senior Rust engineer. Do not over-explain basic concepts.
- Never write code I haven't asked for. Guide me to write it myself.
- Ask what I'm already thinking before giving architectural advice.

## Rust
- thiserror for library errors, anyhow for binary errors.
- cargo nextest not cargo test.
- cargo clippy -- -D warnings before declaring anything done.
- Prefer &str over String in function parameters.

## Git
- Feature branches: feat/short-description
- Write PR descriptions even when working solo.
- Squash merge to main.

## Communication
- Concise. No preambles.
- Code review: 🟢🟡🔴📚 framework only.
- Decisions: ask what I'm thinking first.
```

### Step 4 — Initialise the Project

```bash
git clone git@github.com:YOUR_USERNAME/aiondb.git
cd aiondb
claude
/init           # let Claude Code scan and suggest — review before accepting
```

Verify Claude Code understands the project:
```
> What does AionDB do?
> What are the 5 crates and what does each one own?
> Why must timestamps use HLC instead of SystemTime?
> What non-negotiable rules apply before every commit?
```

### Step 5 — Project Structure for Claude Code

```
aiondb/
├── CLAUDE.md                      ← loaded automatically every session (this file)
├── docs/MENTOR.md                 ← full mentor instructions (@docs/MENTOR.md)
├── .claude/
│   ├── settings.json              ← permissions + hooks (commit this)
│   ├── settings.local.json        ← personal overrides (gitignore this)
│   ├── agents/
│   │   ├── mentor.md              ← architecture + decisions (/mentor)
│   │   └── reviewer.md            ← structured code review (/reviewer)
│   └── commands/
│       ├── adr.md                 ← /adr — create new ADR
│       └── phase-status.md        ← /phase-status — roadmap progress
└── .gitignore                     ← must include: .claude/settings.local.json
```

### Step 6 — `.claude/settings.json`

```json
{
  "model": "claude-sonnet-4-6",
  "thinkingMode": "auto",
  "permissions": {
    "allow": [
      "Bash(cargo:*)", "Bash(git:*)",
      "Bash(grep:*)", "Bash(find:*)",
      "Edit", "Read", "Write"
    ],
    "deny": ["Bash(rm -rf:*)"]
  },
  "hooks": {
    "PreToolUse": [{
      "matcher": "Edit|Write",
      "hooks": [{
        "type": "command",
        "command": "[ \"$(git branch --show-current)\" != \"main\" ] || { echo '{\"block\": true, \"message\": \"Cannot edit on main. Create a feature branch first.\"}' >&2; exit 2; }",
        "timeout": 5
      }]
    }],
    "PostToolUse": [{
      "matcher": "Edit|Write",
      "hooks": [{
        "type": "command",
        "command": "cargo fmt 2>/dev/null || true",
        "timeout": 30
      }]
    }]
  }
}
```

### Step 7 — Custom Agents

**`.claude/agents/mentor.md`**
```markdown
---
name: mentor
description: Staff engineer mentor for AionDB. Use for architecture decisions, distributed systems design, Raft/consensus questions, MVCC design, ADR writing, phase planning. Does NOT write production code.
model: claude-opus-4-6
---
You are a staff engineer mentor. Full instructions in @docs/MENTOR.md.
IMPORTANT: Guide, never code. Socratic questions before answers.
Code review uses 🟢🟡🔴📚 framework from @docs/MENTOR.md exactly.
Always ask what the engineer is already thinking before advising.
```

**`.claude/agents/reviewer.md`**
```markdown
---
name: reviewer
description: Senior Rust code reviewer for AionDB. Use after writing or modifying any Rust code. Checks correctness, safety, distribution-awareness, and test coverage.
model: claude-opus-4-6
---
You are a senior Rust code reviewer for AionDB.
1. Run: git diff HEAD
2. Apply 🟢🟡🔴📚 framework from @docs/MENTOR.md
3. Check: no unwrap() in lib code, all unsafe has SAFETY:, public items have /// docs
4. Check distribution rules: HLC not SystemTime, WAL entries have term+log_index
5. Run: cargo clippy -- -D warnings — report new warnings
6. Check: are there missing test cases for this change?
```

### Step 8 — Slash Commands

**`.claude/commands/adr.md`**
```markdown
---
description: Create a new Architecture Decision Record in docs/adr/
---
Create a new ADR for: $ARGUMENTS
Auto-number based on existing files in docs/adr/. Use template from @docs/MENTOR.md.
Status: Proposed. Leave Consequences and Alternatives for me to fill in.
```

**`.claude/commands/phase-status.md`**
```markdown
---
description: Show current phase progress and next 3 tasks
---
Read @docs/MENTOR.md. Scan task checkboxes to find current phase.
Show: phase name, % complete, next 3 unchecked tasks, any blocked dependencies.
```

### Step 9 — Daily Workflow

```bash
cd aiondb && claude

# Architecture + decisions:
/mentor I'm designing the WAL format for Phase 1. What must I get right for Raft later?

# Code review after writing:
/reviewer

# New ADR:
/adr Hybrid Logical Clocks vs wall clocks for distributed timestamp ordering

# Roadmap check:
/phase-status
```

### Step 10 — Claude.ai vs Claude Code

| Tool | Use For |
|------|---------|
| **Claude.ai (this chat)** | Deep architectural discussions, phase planning, long-form mentorship, CAP/consistency tradeoff exploration |
| **Claude Code (terminal)** | Active coding, diff-based code review, refactoring, file creation, running commands |

Start each phase in Claude.ai. Implement in Claude Code. Review with `/reviewer`.

---

## 🎯 Project Overview

**AionDB** is a distributed, strongly-consistent, bitemporal database engine in Rust.
Built to demonstrate the skills London's top-tier financial engineering teams look for.

**What this project proves:**
- Deep Rust: ownership, lifetimes, async, unsafe, procedural macros
- Database internals: WAL, MVCC, storage engines, query planning, temporal indexing
- Distributed systems: Raft consensus, linearisability, HLC, quorum reads/writes
- Temporal data modelling: bitemporal SQL:2011, financial-grade audit trail
- Production engineering: Docker → Terraform/AWS → Kubernetes cluster operator

**Full roadmap, phases, ADR templates, reading materials:** @docs/MENTOR.md

---

*AionDB — "Time is not a line. It is a plane. And it must be consistent across all nodes."*
*This CLAUDE.md is committed to git. Keep it lean. Delegate detail to @docs/MENTOR.md.*
