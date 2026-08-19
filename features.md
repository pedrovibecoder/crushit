# Crushit — feature list

A macOS menu-bar task manager for developers, with a Pomodoro timer and a
built-in coding agent (Codex or Claude Code).

## Task list

- Task list per project
- Quick add — type a title, pick a category, done
- Task detail: title, description, category, time estimate, story points
- Acceptance criteria — a checklist per task, tick off as met (or generate them with AI)
- Relevant files — the repository files a task touches
- Task dependencies — a task can wait on another, and shows as **Blocked** until it clears
- Statuses: backlog, ready, in progress, needs review, completed, blocked
- Filters: All / Open / Done
- Mark complete with a confirmation step
- Delete with a confirmation step
- Completion celebration with confetti and a chime
- Progress bar — animated, one notch per task
- Progress counted two ways: tasks done and story points done

## Categories

- Custom categories — add your own
- Starts with **Task** and **Bug**
- Rename a category (tasks stay filed under it)
- Pick a colour for each from a palette
- Delete a category (its tasks move to the first one; the last one can't be deleted)
- Coloured dot per category everywhere it appears

## Story points

- Fibonacci scale: 1, 2, 3, 5, 8, 13
- Set manually on any task
- Estimated automatically by AI on generated and imported tasks
- Totalled per plan and per project

## Pomodoro timer

- Focus timer per task
- Presets: 15 / 25 / 45 / 60 minutes, plus a custom length
- Start, pause, resume, stop
- Time tracking per task — total time spent and number of sessions
- Runs in the background — keeps counting with the popup closed
- Live countdown in the menu bar (can be switched off)
- Alarm sound when a session ends (plays three times through the system)
- Notification when a session ends
- Unfinished session restored after restarting the app

## Break timer

- **I'm tired** button
- Break lengths: 5 / 10 / 20 / 30 minutes
- Full-screen rest view with a sleeping animation
- Automatically pauses a running focus session
- **I'm fresh now** — end the break early
- Break countdown in the menu bar
- Alarm and notification when the break is over

## AI — planning

- Works with Codex or Claude Code
- **AI task creation from a goal** — describe what you want, the agent reads your repository and proposes tasks
- Plan includes: summary, what already exists, what's missing, ordered tasks
- Every generated task comes with acceptance criteria, files, dependencies, a time estimate and story points
- Live progress steps while it analyses
- Runs read-only — cannot modify your code while planning
- Keeps running with the popup closed
- Stop an analysis at any point
- Review screen — accept the plan to create the tasks, or discard it

## AI — acceptance criteria

- **Generate acceptance criteria** button on any task you wrote by hand
- The agent reads your repository for context, then proposes the checkable statements that would settle the task
- Covers failure cases and tests, not just the happy path
- Suggestions are shown for review with a checkbox each — untick anything you don't want
- Add the ones you kept in one click, or discard the lot
- Criteria you already wrote are shown to the agent so it doesn't repeat them
- Read-only — suggesting never modifies your code
- Numbering, duplicates and rambling entries are stripped out automatically

## AI — import

- **Create tasks from a screenshot** — drop an image of your board or backlog
- Also supports CSV, Excel (.xlsx/.xls), Numbers, Markdown, TXT, JSON
- Drag and drop onto the window, or choose a file
- One task per item, keeping your original wording
- AI-estimated story points on every imported task
- Up to 40 tasks per import
- Review screen before anything is added

## AI — writing code

- Hand a task to the agent to implement
- Write access confirmed explicitly before it starts
- Live activity feed of what the agent is doing
- List of files changed so far
- Approval prompts — the agent stops and asks, and the menu bar shows it's waiting
- Stop a run at any point
- Resume the same conversation later

## AI — verification

- Checks finished work against the task's own acceptance criteria
- Per-criterion verdict with evidence from the code
- Overall recommendation
- Read-only — never modifies anything, never marks the task complete itself

## Git review

- Changed files in the repository with their state (added, modified, deleted, renamed, untracked)
- Insertion and deletion counts
- Per-file diff viewer with coloured additions, removals and context
- Works with staged and unstaged changes

## History and stats

- **GitHub-style contribution graph** — a year of finished tasks
- Shading scaled to your busiest day
- Current streak of consecutive days
- Story points completed over the year
- Click any day to see just that day's tasks
- Full history list grouped by date with completion times
- Weekly stats: time focused today, tasks done, sessions taken, average session length

## Projects

- Multiple projects, one active at a time
- Git repository detection
- Current branch shown, refreshed automatically and on demand
- Detects project manifests
- Picking a subfolder records the repository root
- Reveal the project in Finder
- First-run setup: choose a project, check the agent

## Coding agent setup

- Choose Codex or Claude Code
- Detects whether it's installed and signed in, and which auth mode
- Picks the newest install rather than the first on PATH
- Set a custom binary path
- Choose a model, or leave it to your own agent config

## App

- Lives in the macOS menu bar — no dock icon
- Menu-bar popup that sizes itself to its content
- Full desktop window with sidebar navigation
- Global hotkey (⌃⌥B) for when the menu bar is full
- Keyboard shortcuts: ⌘1–⌘4 screens, ⌘N new task, ⌘G new goal, ⌘W hide, Esc back
- Menu-bar status: focus countdown, break countdown, Coding…, Approval, Done
- Two themes: Light and GitHub Dark
- Notifications for sessions, runs, plans and approvals
- Sound on/off
- Close the popup when it loses focus (optional)
- Launch at login
- Right-click the menu-bar icon for Open and Quit

## Data

- Everything stored locally in a single SQLite file
- No account, no sign-up, no telemetry
- Agent credentials never read
- Database migrated forward on upgrade, never rebuilt
