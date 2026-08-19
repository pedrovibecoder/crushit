# Blitzit

A macOS menu-bar taskbar for developers, built to the
[PRD](./PRD%20—%20AI%20Developer%20Taskbar%20with%20Codex.md).

**Phases 1 and 2 are implemented**: the Tauri + Vue shell, menu-bar tray and
popup, SQLite store, project selection, task management and the focus timer;
then Codex detection, auth status, app-server communication, read-only
repository analysis, and goal → plan → task generation. Running tasks *with*
Codex (phase 3) and verification (phase 4) are still to come.

## Running

```sh
npm install
npm run tauri:dev     # development, with hot reload
npm run tauri:build   # .app and .dmg in src-tauri/target/release/bundle
```

The app has no dock icon. Look for the lightning bolt in the menu bar; a left
click opens the popup beneath it, a right click gives Open and Quit.

**If the icon never appears**, the menu bar is full rather than the app being
broken. macOS silently hides status items it cannot fit, and on a laptop with a
notch the usable strip is small — an item whose slot lands under the notch is
dropped. Press **⌃⌥B** to open the popup instead; the live combination is shown
in Settings. To get the icon back, free a slot: quit another menu-bar app, turn
items off under System Settings → Control Center, or run a menu-bar manager.

You can check what macOS is actually doing with:

```sh
# every status item, including the ones being hidden
swift preview/windows.swift            # onScreen only
```

## Coding agents

Planning runs through **Codex** or **Claude Code**; pick one under Settings →
Coding agent. Whichever you choose needs to be installed and signed in
(`codex login` / `claude auth login`). Settings shows what Blitzit found for
each and lets you point at a specific binary when discovery misses it.

The PRD scopes V1 to Codex; Claude Code was added on request, so the two sit
behind one small surface (`agent.rs`) rather than being threaded through the
app.

Discovery deliberately picks the **newest** install rather than the first on
`PATH`. A stale `npm` copy easily shadows the one bundled with the ChatGPT
desktop app, and an old CLI rejects newer models with
"requires a newer version of Codex".

## Checks

```sh
npm run typecheck                          # vue-tsc over the frontend
cd src-tauri && cargo test                 # timer, tray, database, planning
cd src-tauri && cargo test -- --ignored    # live tests against your real agents
npm run preview:ui                         # render the popup in a browser
```

`npm run preview:ui` serves the popup on <http://localhost:1430> with the Tauri
runtime stubbed out (`preview/mock.ts`), which is the quickest way to look at a
screen without building the app. `?view=today|task|goal|projects|settings`
picks the screen and `?state=plan|analyzing|failed|nocodex` picks the Codex
situation.

The `--ignored` tests spawn the real CLIs; the planning ones start a model turn
and so consume account quota. The Claude one needs a repository to look at:
`BLITZIT_FIXTURE=/path/to/repo cargo test --test claude_live -- --ignored`.

## How it fits together

```
Vue 3 + Pinia (popup UI)
        │  invoke / events
Tauri 2 ├── tray + popup placement       src-tauri/src/{tray,popup}.rs
        ├── focus timer                  src-tauri/src/focus.rs
        ├── analysis state               src-tauri/src/analysis.rs
        ├── SQLite                       src-tauri/src/db.rs
        ├── plan shape + sanitising      src-tauri/src/plan.rs
        └── agent selection              src-tauri/src/agent.rs
                ├── Codex        src-tauri/src/codex/   (JSON-RPC app-server)
                └── Claude Code  src-tauri/src/claude/  (claude -p, stream-json)
                              │
                     the local repository
```

**Long-running work lives in the backend, not the webview.** The focus timer
and repository analysis both keep their state in Rust and emit events, so
closing the popup never stops a session or an analysis, and reopening shows the
run still in progress. The frontend only mirrors what the backend emits.

Database state lives at
`~/Library/Application Support/com.cleonart.blitzit/blitzit.sqlite3`.
Migrations run from `MIGRATIONS` in `db.rs`, tracked by `PRAGMA user_version` —
add a new entry rather than editing an existing one.

## Planning is read-only

The PRD requires analysis to run without write access. Each agent enforces that
differently, so each is pinned separately:

- **Codex** — the thread starts with `sandbox: "read-only"`, the turn re-states
  `sandboxPolicy: { type: "readOnly", networkAccess: false }`, and the client
  **declines** every approval request rather than leaving it unanswered
  (`codex/client.rs`). A live test asserts the thread comes back `readOnly`.
- **Claude Code** — `--permission-mode plan` is *not* sufficient on its own: it
  blocks the editing tools but still allows `Bash`, and a shell can write
  anywhere. (An early run did exactly that, writing under `~/.claude/plans`.)
  `Bash` is therefore on the `--disallowedTools` list alongside `Write`, `Edit`
  and `NotebookEdit`; analysis inspects the repository with `Read`, `Glob` and
  `Grep`. `AskUserQuestion` is refused too — nobody can answer it in a headless
  run. A live test compares `git status` before and after a run.

Plans are sanitised in `plan.rs` before storage: unknown categories fall back,
estimates are clamped, and `dependsOn` is filtered to backward references only,
so a plan can never describe a dependency cycle.

How the shape is requested differs by agent, deliberately:

- **Codex** uses `outputSchema` on the turn. The app server enforces it, and a
  rejected answer does not become the model's problem.
- **Claude Code** puts the shape in the prompt instead of using `--json-schema`.
  That flag is enforced by a `StructuredOutput` tool call, and a payload that
  trips its input limit sends the model into a loop: it retries with fewer
  fields, which then fails the schema's own `required` list, and it never
  converges. One observed run burned ten minutes that way on a single-file
  repository. `--max-turns` now bounds the conversation as a backstop, and the
  parser accepts JSON embedded in prose.

Nothing Codex proposes becomes a task until the developer presses **Add tasks**.

## Phase coverage

| PRD requirement | Where |
| --- | --- |
| FR-01 Project selection | `views/ProjectsView.vue`, `repo.rs` |
| FR-02 Goal creation | `views/GoalView.vue`, `db.rs` goals |
| FR-03 Repository analysis (read-only) | `codex/planning.rs` |
| FR-04 Plan generation | `codex/planning.rs` output schema |
| FR-05 Task editing | `views/TaskView.vue` |
| FR-06 Dependency tracking | `task_dependencies`, shown as "Blocked" |
| FR-07 Focus timer | `focus.rs`, `stores/focus.ts` |
| FR-14 Manual completion | Task screen footer |

FR-08 to FR-13 — running a task with Codex, live activity, approvals,
stop/resume, diffs and verification — are the next two phases. The Task screen
shows a disabled **Run with Codex** row so the shape is visible without
pretending the wiring exists.
