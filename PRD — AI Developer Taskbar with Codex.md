# Product Requirements Document
## AI Developer Taskbar — Codex-Only MVP

**Status:** Draft  
**Platform:** macOS first  
**Product Type:** Menu-bar developer productivity application  
**Frontend:** Vue 3 + TypeScript  
**Desktop Framework:** Tauri 2  
**Coding Agent:** OpenAI Codex only  
**Local Storage:** SQLite  
**Version:** MVP / V1

---

# 1. Product Summary

AI Developer Taskbar is a lightweight macOS menu-bar application that helps developers answer one simple question:

> **What should I work on next?**

Instead of manually breaking a development goal into tasks, the developer selects a local code repository and describes the outcome they want.

Example:

> Add Google authentication and allow users to connect multiple accounts.

The application asks Codex to analyze the existing repository, understand what already exists, identify what is missing, and generate an actionable development plan.

The developer can then start any generated task directly from the menu bar.

Codex performs the implementation inside the selected repository while the application displays its progress.

The product combines:

**Goal → Code Analysis → Tasks → Focus → Codex Execution → Verification**

---

# 2. Problem

Developers using AI coding tools still spend significant time deciding:

- what needs to be built;
- which files are relevant;
- what should happen first;
- which tasks depend on other tasks;
- whether a task is actually finished;
- what to work on next.

Traditional todo applications do not understand the codebase.

AI coding agents understand code but normally wait for the developer to tell them exactly what to do.

There is a missing layer between:

**Project intention**

and

**AI coding execution.**

The product fills that gap.

---

# 3. Product Vision

Create a developer-focused execution system where the developer describes the desired software outcome rather than manually maintaining the implementation checklist.

The application continuously understands:

**Current repository state**

↓

**Desired state**

↓

**Missing implementation**

↓

**Development tasks**

↓

**Current task**

↓

**Codex execution**

↓

**Code changes**

↓

**Task completion**

The long-term vision is:

> **Your codebase tells you what to do next.**

---

# 4. Target User

## Primary User

Solo developers and small software teams who:

- use AI heavily when coding;
- manage several features or projects;
- frequently use Codex;
- work primarily from Git repositories;
- want less overhead than Jira/Linear;
- want their todo list connected directly to their code.

## Example User

A developer is building a SaaS application.

Instead of manually creating:

- Create database table
- Add API
- Build UI
- Add validation
- Write tests

they enter:

> Allow customers to generate and download invoices as PDF.

The application analyzes the repository and creates those tasks automatically.

---

# 5. Product Principles

### Minimal

The application should feel closer to a timer or system utility than a traditional project-management platform.

### Code-aware

Tasks must be generated from evidence found inside the repository.

### Developer controlled

Codex may suggest, implement, and verify work, but the developer remains responsible for accepting plans and marking work complete.

### Focused

The main interface should emphasize the current task rather than showing a large project board.

### Local-first

Repository access and development execution should occur locally during the MVP.

---

# 6. Core User Experience

The normal application should not require a large window.

The primary UI lives in the macOS menu bar.

Example:

```text
WiFi     Search     ⚡ 24:31     Battery     08:42
                       ↑
                    Dev App
```

Clicking it opens:

```text
┌─────────────────────────────────┐
│ TODAY                           │
│                                 │
│ ▶ Invoice PDF Service    24:31  │
│   3 / 5 criteria                │
│                                 │
│ ○ Download API            30m   │
│ ○ Authorization test      20m   │
│                                 │
│ ──────────────────────────────  │
│                                 │
│ + Task             ✦ New Goal  │
└─────────────────────────────────┘
```

The developer should be able to understand their current work within approximately 2–3 seconds of opening the menu.

---

# 7. Primary Workflow

## Step 1 — Select Repository

The developer chooses a local Git repository.

```text
Select Project

/Users/user/projects/my-app

Detected

✓ Git repository
✓ main branch
✓ package.json

[ Use Project ]
```

The application stores the project path locally.

---

## Step 2 — Set Goal

From the menu bar:

```text
✦ NEW GOAL

What do you want to build?

┌──────────────────────────────────┐
│ Allow customers to download      │
│ invoices as PDF.                 │
└──────────────────────────────────┘

Project
my-app

[ Analyze Project ]
```

---

## Step 3 — Codex Analyzes Repository

Codex receives the project working directory and goal.

Analysis should initially run with **read-only repository access**.

Codex can work with sandbox policies including read-only and workspace-write modes; the product should use read-only during planning and workspace-write only after the user explicitly starts implementation.

UI:

```text
Analyzing project...

✓ Repository structure
✓ Database
✓ Existing API
◐ Frontend components
○ Tests
```

---

# 8. AI Planning Output

Codex must return structured task information rather than a long conversational answer.

Example:

```text
GOAL

Invoice PDF Downloads


Existing

✓ Invoice database model
✓ Invoice detail page
✓ Authentication
✓ Invoice API


Missing

□ PDF generation
□ Download endpoint
□ Authorization validation
□ Download button
□ Tests
```

Generated plan:

```text
5 TASKS

1. Create invoice PDF service
   Backend

2. Add invoice download endpoint
   Backend
   Depends on #1

3. Add invoice ownership validation
   Security
   Depends on #2

4. Add Download PDF button
   Frontend
   Depends on #2

5. Add invoice PDF tests
   Testing
   Depends on #1–3
```

The developer must confirm:

**Add Plan**

before tasks are saved.

---

# 9. Task Structure

Each generated task should contain:

**Title**

Example:

> Create Invoice PDF Service

**Description**

Short explanation of what needs to change.

**Category**

Possible values:

- Frontend
- Backend
- Database
- Security
- Testing
- DevOps
- Refactor
- Bug

**Acceptance Criteria**

Example:

```text
□ Generate valid PDF
□ Include invoice items
□ Include customer information
□ Return PDF buffer
□ Handle generation errors
```

**Relevant Files**

Example:

```text
src/services/invoice.ts
src/models/invoice.ts
```

These are suggestions rather than guaranteed files.

**Dependencies**

Example:

```text
Blocked by Task #2
```

**Status**

```text
Backlog
Ready
In Progress
Needs Review
Completed
Blocked
```

---

# 10. Today View

The default menu-bar screen is the Today view.

```text
TODAY

2 / 5 COMPLETE

████████░░░░


▶ Create invoice PDF service

○ Add download endpoint

○ Add authorization test


+ Task

✦ New Goal
```

Users may manually add tasks alongside AI-generated tasks.

AI-generated tasks must display a small indicator:

```text
✦ AI
```

---

# 11. Focus Mode

Starting a task puts the application into focus mode.

Menu bar changes from:

```text
⚡
```

to:

```text
⚡ 24:31
```

Clicking shows:

```text
CREATE INVOICE PDF SERVICE

24:31

Acceptance Criteria

✓ Read invoice information
□ Generate PDF
□ Error handling
□ Tests

────────────────────────

Codex

○ Not started

[ Run with Codex ]
```

---

# 12. Run With Codex

The primary execution action is:

**Run with Codex**

No Claude, Gemini, Cursor, or other coding agents should be supported in V1.

When selected:

```text
Run task with Codex?

Project
my-app

Branch
feature/invoice-pdf

Task
Create Invoice PDF Service

Codex will be able to modify files
inside this workspace.

[ Cancel ]

[ Start Codex ]
```

Explicit confirmation is required before enabling write access.

---

# 13. Codex Execution

For rich integration, the desktop application should use the local **Codex App Server**.

OpenAI documents App Server specifically as the interface for embedding Codex into rich clients. It supports threads, turns, approvals, conversation history, streamed agent events, file-change events, and interruption.

Architecture:

```text
Vue UI
   │
   ▼
Tauri
   │
   ▼
Codex Integration Service
   │
   ▼
codex app-server
   │
   ▼
Local Repository
```

The Tauri backend can communicate with App Server using its default stdio JSONL transport.

---

# 14. Codex Thread Model

Each development task should have its own Codex thread.

Example:

```text
Project
└── Goal
    ├── Task #1
    │   └── Codex Thread A
    │
    ├── Task #2
    │   └── Codex Thread B
    │
    └── Task #3
        └── Codex Thread C
```

Store:

```text
task_id
codex_thread_id
started_at
last_activity
status
```

Codex supports starting and resuming stored threads, allowing a task to continue without starting from zero each time.

---

# 15. Live Codex Activity

While Codex is running:

```text
CODEX

● Working

04:32

✓ Read existing invoice service
✓ Inspected invoice model
◐ Creating PDF service

Files changed
2


[ View Activity ]

[ Stop ]
```

The interface should summarize events rather than expose every internal log line.

App Server emits thread, turn, item, tool-progress, and completion events that can drive this live UI.

---

# 16. Codex Approval UX

Codex may encounter an action requiring developer approval.

Example:

```text
CODEX NEEDS APPROVAL

Run command?

npm install pdf-lib

Reason
Required for PDF generation.

[ Reject ]

[ Allow Once ]
```

The application must never silently bypass Codex sandboxing or approval controls.

The CLI supports approval policies and explicitly warns that bypassing approvals and sandboxing should only be used in hardened environments.

---

# 17. Stop Execution

The developer must always be able to stop an active Codex task.

```text
[ Stop Codex ]
```

Stopping should:

1. interrupt the active turn;
2. preserve the Codex thread;
3. preserve code already written;
4. leave the task as In Progress.

The user may later choose:

```text
[ Continue with Codex ]
```

---

# 18. Task Completion

When Codex finishes:

```text
CODEX FINISHED

Files changed
4

src/services/invoice-pdf.ts
src/api/invoices/download.ts
tests/invoice-pdf.test.ts
package.json

[ Review Changes ]

[ Verify Task ]
```

The application should **not automatically mark the task complete**.

---

# 19. AI Verification

Verification runs another Codex turn using read-only access.

Prompt concept:

```text
Review the current repository against
the following task acceptance criteria.

Do not modify files.

Return evidence for every criterion.
```

Example result:

```text
TASK VERIFICATION

Create Invoice PDF Service

✓ Generates PDF
✓ Includes customer data
✓ Includes invoice items
✓ Handles missing invoice

⚠ No unit test found


4 / 5 criteria satisfied


Recommendation

Keep task open.

[ Keep Working ]

[ Complete Anyway ]
```

The developer retains final control.

---

# 20. Repository Changes

The application should monitor Git state.

Display:

```text
4 files changed

+182
-24
```

Available action:

**View Changes**

A basic diff viewer is sufficient for MVP.

Full GitHub integration is not required for V1.

---

# 21. Codex Authentication

During onboarding:

```text
CODEX

Checking Codex...

✓ Codex installed
✓ Signed in

[ Continue ]
```

If authentication is unavailable:

```text
Codex needs to be signed in.

[ Sign In ]
```

Codex supports local authentication through either ChatGPT sign-in or an OpenAI API key. The CLI exposes login and login-status flows that the desktop application can use for onboarding diagnostics.

The application must **never read or copy Codex authentication tokens directly**.

Authentication remains managed by Codex.

---

# 22. Application Architecture

```text
┌──────────────────────────────┐
│       macOS Menu Bar         │
└──────────────┬───────────────┘
               │
               ▼
┌──────────────────────────────┐
│ Vue 3 + TypeScript           │
│                              │
│ Today                        │
│ Goal                         │
│ Tasks                        │
│ Focus                        │
│ Codex Activity               │
└──────────────┬───────────────┘
               │
               ▼
┌──────────────────────────────┐
│ Tauri 2                      │
│                              │
│ Tray                         │
│ Windows                      │
│ File access                  │
│ Process management           │
│ Local IPC                    │
└──────────────┬───────────────┘
               │
         ┌─────┴──────┐
         ▼            ▼
      SQLite      Git CLI
         │
         ▼
┌──────────────────────────────┐
│ Codex Controller             │
│                              │
│ JSON-RPC / JSONL             │
└──────────────┬───────────────┘
               │
               ▼
       codex app-server
               │
               ▼
        Local Repository
```

---

# 23. Recommended Frontend Stack

```text
Vue 3
TypeScript
Vite
Pinia
Tailwind CSS
```

Pinia manages:

```text
Project state
Tasks
Goals
Current focus task
Timer
Codex execution state
Settings
```

---

# 24. Local Database

Use SQLite.

Core tables:

```text
projects
goals
tasks
task_dependencies
codex_threads
focus_sessions
task_verifications
settings
```

Example relationship:

```text
Project
   │
   └── Goal
        │
        ├── Task
        │    ├── Codex Thread
        │    ├── Focus Sessions
        │    └── Verification
        │
        └── Task
```

No cloud account is required for the MVP.

---

# 25. Menu-Bar States

Normal:

```text
⚡
```

Focus running:

```text
⚡ 24:31
```

Codex running:

```text
✦ Coding…
```

Codex waiting:

```text
! Approval
```

Task finished:

```text
✓ Done
```

After several seconds, completed states return to the normal icon.

---

# 26. Main Screens

The product should contain only five primary interfaces for V1:

### Today

Current and upcoming tasks.

### Goal

Create a goal and generate a plan.

### Task

Task details, criteria, dependencies, and Codex execution.

### Activity

Live Codex activity and changed files.

### Settings

Project configuration, Codex status, timer settings, and preferences.

A traditional project-management dashboard is explicitly not required for the MVP.

---

# 27. Functional Requirements

## FR-01 Project Selection

User can select a local Git repository.

## FR-02 Goal Creation

User can describe a desired software outcome.

## FR-03 Repository Analysis

Codex analyzes the repository without modifying files.

## FR-04 Plan Generation

Codex generates structured development tasks.

## FR-05 Task Editing

User can modify, delete, reorder, or manually create tasks.

## FR-06 Dependency Tracking

Tasks may depend on other tasks.

## FR-07 Focus Timer

A developer can start, pause, and stop a timer for a task.

## FR-08 Codex Execution

A task can be sent to Codex for implementation.

## FR-09 Live Status

Codex activity is displayed while work is running.

## FR-10 User Approval

Potentially sensitive commands may require user approval.

## FR-11 Stop / Continue

User can interrupt and later resume a Codex task.

## FR-12 Diff Detection

Changed repository files are displayed.

## FR-13 Verification

Codex compares the implementation against acceptance criteria.

## FR-14 Manual Completion

User controls final task completion.

---

# 28. Non-Functional Requirements

### Performance

Menu-bar popup should appear essentially immediately after click.

### Reliability

Closing the popup must not stop an active Codex task.

### Privacy

Local code must not be copied into the product's own cloud database during MVP.

### Recovery

Application restart must recover:

- projects;
- goals;
- tasks;
- timers;
- stored Codex thread IDs.

### Safety

Code-writing access must only be enabled after explicit user action.

---

# 29. MVP Non-Goals

Do **not** build the following in V1:

- Claude integration
- Gemini integration
- Cursor integration
- Jira
- Linear
- Slack
- team collaboration
- GitHub Issues sync
- automatic pull requests
- cloud synchronization
- mobile application
- Windows version
- multiple simultaneous coding agents
- autonomous task execution
- automatic deployment
- AI auto-merging
- advanced analytics
- vector database
- full code indexing platform

Keeping these out is important.

The first version should validate one workflow:

> **Goal → Plan → Task → Codex → Verify**

---

# 30. MVP Success Criteria

The MVP is successful if a developer can:

1. Install the application.
2. Connect a local repository.
3. Enter a development goal.
4. Receive useful code-aware tasks.
5. Start one task.
6. Allow Codex to implement it.
7. Observe Codex progress.
8. Review changed files.
9. Verify acceptance criteria.
10. Continue to the next task.

The full workflow should be possible without opening a traditional project-management application.

---

# 31. Key Product Metric

The most important metric is:

**Percentage of generated tasks that developers accept without major rewriting.**

Supporting metrics:

```text
Goals created
Tasks generated per goal
Tasks accepted
Tasks manually rewritten
Codex tasks started
Codex task completion rate
Verification pass rate
Average focus session
Time from goal → first implementation
```

A strong early signal would be users repeatedly trusting the application to answer:

> What should I work on next?

---

# 32. Development Phases

## Phase 1 — Shell

Build:

```text
Tauri
Vue
Menu bar
SQLite
Project selection
Task UI
Focus timer
```

No AI required initially.

---

## Phase 2 — Codex Planning

Build:

```text
Codex detection
Codex authentication status
App Server communication
Repository analysis
Goal → task generation
```

---

## Phase 3 — Codex Execution

Build:

```text
Run with Codex
Workspace write mode
Activity events
Stop
Resume
Approvals
```

---

## Phase 4 — Verification

Build:

```text
Git diff
Changed files
Acceptance criteria validation
Task completion flow
```

---

## Phase 5 — Polish

Build:

```text
Keyboard shortcuts
Notifications
Animations
Error recovery
Onboarding
Auto-start on login
Menu-bar timer
```

---

# 33. Suggested MVP Development Order

```text
01 Tauri + Vue setup

02 macOS menu-bar icon

03 popup window

04 SQLite database

05 project selector

06 manual tasks

07 focus timer

08 goal UI

09 detect Codex

10 launch Codex App Server

11 repository analysis

12 structured plan generation

13 generated task UI

14 run task with Codex

15 stream Codex activity

16 approvals

17 stop/resume task

18 detect Git changes

19 task verification

20 onboarding + polish
```

---

# 34. Core Product Differentiator

Traditional todo application:

```text
Developer
   ↓
Creates Tasks
   ↓
Completes Tasks
```

AI coding agent:

```text
Developer
   ↓
Writes Prompt
   ↓
Agent Codes
```

This product:

```text
Developer
   ↓
Sets Goal
   ↓
Codex Understands Codebase
   ↓
Creates Development Plan
   ↓
Developer Chooses Task
   ↓
Codex Implements
   ↓
Codex Verifies
   ↓
Next Task
```

The application therefore sits between **project planning** and **AI coding execution**.

---

# 35. Product Statement

> **Set the goal. Let your codebase create the plan. Focus on one task. Let Codex help execute it.**

The application should feel less like Jira and more like a small developer companion that is always available from the menu bar.