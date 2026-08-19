// Stand-in for the Tauri runtime so the UI can be rendered in a browser for
// visual review. Preview-only: never imported by the app itself.
const DAY_MS = 86400000;
const asDay = (offset: number) => {
  const date = new Date(Date.now() + offset * DAY_MS);
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
};
const TODAY = asDay(0);
const YESTERDAY = asDay(-1);

const PROJECT = {
  id: 1,
  path: "/Users/dev/projects/my-app",
  name: "my-app",
  isGit: true,
  branch: "feature/invoice-pdf",
  createdAt: 0,
  lastOpenedAt: 0,
};

const TASKS = [
  {
    id: 1,
    projectId: 1,
    goalId: 1,
    title: "Wire real permission key into customer menu entry",
    description: "Render an invoice to a PDF buffer.",
    category: "task",
    status: "in-progress",
    position: 0,
    estimateMinutes: 45,
    storyPoints: 5,
    plannedFor: TODAY,
    isAiGenerated: true,
    createdAt: 0,
    updatedAt: 0,
    completedAt: null,
    criteria: [
      { id: 1, taskId: 1, text: "Generate valid PDF", isMet: true, position: 0 },
      { id: 2, taskId: 1, text: "Include invoice items", isMet: true, position: 1 },
      { id: 3, taskId: 1, text: "Include customer information", isMet: true, position: 2 },
      { id: 4, taskId: 1, text: "Return PDF buffer", isMet: false, position: 3 },
      { id: 5, taskId: 1, text: "Handle generation errors", isMet: false, position: 4 },
    ],
    files: ["src/services/invoice.ts", "src/models/invoice.ts"],
    dependsOn: [],
    focusSeconds: 4500,
    focusSessions: 3,
  },
  {
    id: 2,
    projectId: 1,
    goalId: 1,
    title: "Audit and complete customer i18n coverage",
    description: null,
    category: "task",
    status: "ready",
    position: 1,
    estimateMinutes: 30,
    storyPoints: 3,
    plannedFor: TODAY,
    isAiGenerated: true,
    createdAt: 0,
    updatedAt: 0,
    completedAt: null,
    criteria: [],
    files: [],
    dependsOn: [1],
    focusSeconds: 0,
    focusSessions: 0,
  },
  {
    id: 3,
    projectId: 1,
    goalId: 1,
    title: "Verify service endpoints against api_spec.json",
    description: null,
    category: "bug",
    status: "ready",
    position: 2,
    estimateMinutes: 20,
    storyPoints: 2,
    plannedFor: YESTERDAY,
    isAiGenerated: true,
    createdAt: 0,
    updatedAt: 0,
    completedAt: null,
    criteria: [],
    files: [],
    dependsOn: [2],
    focusSeconds: 0,
    focusSessions: 0,
  },
  {
    id: 4,
    projectId: 1,
    goalId: 1,
    title: "Create design-code doc and registry row for the customer screen",
    description: null,
    category: "design",
    status: "ready",
    position: 3,
    estimateMinutes: 25,
    storyPoints: 3,
    plannedFor: TODAY,
    isAiGenerated: true,
    createdAt: 0,
    updatedAt: 0,
    completedAt: null,
    criteria: [],
    files: [],
    dependsOn: [],
    focusSeconds: 0,
    focusSessions: 0,
  },
  {
    id: 5,
    projectId: 1,
    goalId: null,
    title: "Add tests for customer table, modal, and page hook",
    description: null,
    category: "task",
    status: "completed",
    position: 4,
    estimateMinutes: null,
    storyPoints: 1,
    plannedFor: YESTERDAY,
    isAiGenerated: false,
    createdAt: 0,
    updatedAt: 0,
    completedAt: 1,
    criteria: [],
    files: [],
    dependsOn: [],
    focusSeconds: 0,
    focusSessions: 0,
  },
];

const CATEGORIES = [
  { id: 1, slug: "task", label: "Task", color: "#4a9bf5", position: 0 },
  { id: 2, slug: "bug", label: "Bug", color: "#e0609b", position: 1 },
  { id: 3, slug: "design", label: "Design", color: "#7b61ff", position: 2 },
];

/**
 * A year of finished work for the history graph. Generated from a fixed seed so
 * the picture is the same every time it is reviewed.
 */
const HISTORY = (() => {
  const TITLES = [
    "L-SALES [Order] Register new by copy function",
    "Audit and complete customer i18n coverage",
    "Verify service endpoints against api_spec.json",
    "Wire real permission key into customer menu entry",
    "Backdated order recalculates gross correctly",
  ];
  const out: Array<Record<string, unknown>> = [];
  const today = Math.floor(Date.now() / 1000);
  let seed = 7;
  const random = () => {
    seed = (seed * 1103515245 + 12345) % 2147483648;
    return seed / 2147483648;
  };
  for (let back = 0; back < 365; back += 1) {
    const at = today - back * 86400;
    const weekday = new Date(at * 1000).getDay();
    const chance = weekday === 0 || weekday === 6 ? 0.16 : 0.62;
    if (random() > chance) continue;
    const count = 1 + Math.floor(random() * 4);
    for (let index = 0; index < count; index += 1) {
      out.push({
        id: 1000 + out.length,
        title: TITLES[Math.floor(random() * TITLES.length)],
        category: random() > 0.75 ? "bug" : random() > 0.5 ? "design" : "task",
        storyPoints: [1, 2, 3, 5, 8][Math.floor(random() * 5)],
        completedAt: at - Math.floor(random() * 28800),
      });
    }
  }
  return out;
})();

const SETTINGS = {
  focusMinutes: 25,
  showTimerInMenuBar: true,
  hidePopupOnBlur: true,
  activeProjectId: 1,
  agent: new URLSearchParams(location.search).get("agent") ?? "codex",
  codexPath: null,
  codexModel: null,
  claudePath: null,
  claudeModel: null,
  theme: new URLSearchParams(location.search).get("theme") ?? "light",
  slackConnected: new URLSearchParams(location.search).get("slack") !== null,
  notifications: true,
  sounds: true,
  launchAtLogin: false,
  focusBlockEnabled: new URLSearchParams(location.search).get("block") !== null,
  focusBlockSites: ["instagram.com", "facebook.com", "x.com", "twitter.com", "tiktok.com"],
  onboarded: new URLSearchParams(location.search).get("view") !== "onboarding",
};

const REST = {
  status: new URLSearchParams(location.search).get("rest") ? "running" : "idle",
  minutes: 10,
  durationSeconds: 600,
  elapsedSeconds: 190,
  remainingSeconds: 410,
};

const FOCUS = {
  status: "running",
  sessionId: 1,
  taskId: 1,
  durationSeconds: 1500,
  elapsedSeconds: 29,
  remainingSeconds: 1471,
};

// `?state=` picks which Codex situation the preview renders.
const STATE = new URLSearchParams(location.search).get("state") ?? "";

const PLAN = {
  summary:
    "Invoices are modelled and exposed over the API, but nothing renders them as a PDF or serves a download.",
  existing: [
    "Invoice model in src/models/invoice.ts",
    "Invoice detail page in src/pages/invoice.vue",
    "Session auth middleware in src/middleware/auth.ts",
  ],
  missing: ["PDF generation", "Download endpoint", "Ownership check", "Tests"],
  tasks: [
    {
      title: "Wire real permission key into customer menu entry",
      description: "Render an invoice model to a PDF buffer.",
      category: "task",
      acceptanceCriteria: ["Generates a valid PDF", "Includes line items", "Handles a missing invoice"],
      relevantFiles: ["src/services/invoice-pdf.ts"],
      dependsOn: [],
      estimateMinutes: 45,
      storyPoints: 5,
    },
    {
      title: "Audit and complete customer i18n coverage",
      description: "Serve the generated PDF over HTTP.",
      category: "task",
      acceptanceCriteria: ["Returns application/pdf", "404s for unknown invoices"],
      relevantFiles: ["src/api/invoices/download.ts"],
      dependsOn: [1],
      estimateMinutes: 30,
      storyPoints: 3,
    },
    {
      title: "Check invoice ownership",
      description: "Only the owning customer may download an invoice.",
      category: "bug",
      acceptanceCriteria: ["Rejects another customer's invoice"],
      relevantFiles: ["src/middleware/auth.ts"],
      dependsOn: [2],
      estimateMinutes: 20,
      storyPoints: 2,
    },
    {
      title: "Add a Download PDF button",
      description: "Surface the download on the invoice detail page.",
      category: "design",
      acceptanceCriteria: ["Button appears for paid invoices"],
      relevantFiles: ["src/pages/invoice.vue"],
      dependsOn: [2],
      estimateMinutes: 25,
      storyPoints: 3,
    },
  ],
};

const GOAL = {
  id: 1,
  projectId: 1,
  title: "Allow customers to download invoices as PDF",
  prompt: "Allow customers to download invoices as PDF.",
  status: STATE === "plan" ? "ready" : STATE === "failed" ? "failed" : "draft",
  agent: SETTINGS.agent,
  codexThreadId: "01a017c8-e344-7222-9061-610c601abe7e",
  error: STATE === "failed"
    ? "Codex usage limit reached. You've hit your usage limit. Try again at Aug 20th, 2026 11:36 AM."
    : null,
  createdAt: 0,
  updatedAt: 0,
  plan: STATE === "plan" ? PLAN : null,
};

const ANALYSIS = {
  status: STATE === "analyzing" ? "running" : STATE === "failed" ? "failed" : "idle",
  goalId: 1,
  projectId: 1,
  goalTitle: "Allow customers to download invoices as PDF",
  steps:
    STATE === "analyzing"
      ? [
          { id: "1", label: "Running rg --files -g '*.ts'", done: true },
          { id: "2", label: "Working out what matters", done: true },
          { id: "3", label: "Running cat src/models/invoice.ts", done: true },
          { id: "4", label: "Writing the plan", done: false },
        ]
      : [],
  error: STATE === "failed"
    ? "Codex usage limit reached. You've hit your usage limit. Try again at Aug 20th, 2026 11:36 AM."
    : null,
  startedAt: 0,
};

const AGENT_STATUS = {
  agent: SETTINGS.agent,
  installed: STATE !== "nocodex",
  path: SETTINGS.agent === "codex" ? "/Applications/ChatGPT.app/Contents/Resources/codex" : "/Users/dev/.local/bin/claude",
  version: SETTINGS.agent === "codex" ? "0.148.0-alpha.15" : "2.1.170",
  signedIn: STATE !== "nocodex",
  authMode: SETTINGS.agent === "codex" ? "ChatGPT" : "claude.ai",
  problem: STATE === "nocodex" ? "Codex CLI not found. Install it, or set its path in Settings." : null,
};

const RUN = {
  status:
    STATE === "approval"
      ? "awaiting-approval"
      : STATE === "running"
        ? "running"
        : STATE === "ran"
          ? "finished"
          : "idle",
  taskId: 1,
  taskTitle: "Wire real permission key into customer menu entry",
  agent: SETTINGS.agent,
  threadId: "01a017c8-e344-7222-9061-610c601abe7e",
  activity: [
    { id: "1", label: "Reading permissions.ts", kind: "read", done: true },
    { id: "2", label: "Running npm test", kind: "command", done: true },
    { id: "3", label: "Editing customer-menu.ts", kind: "file", done: false },
  ],
  changedFiles: ["src/menu/customer.ts", "src/permissions/keys.ts"],
  approval:
    STATE === "approval"
      ? {
          id: 7,
          title: "Run command?",
          command: "npm install pdf-lib",
          reason: "Required for PDF generation.",
        }
      : null,
  summary:
    STATE === "ran"
      ? "Added the permission key and wired it into the customer menu entry. Tests pass."
      : null,
  error: null,
  startedAt: Math.floor(Date.now() / 1000) - 272,
};

const VERIFICATION = {
  status: STATE === "verified" ? "ready" : STATE === "verifying" ? "running" : "idle",
  taskId: 1,
  taskTitle: "Wire real permission key into customer menu entry",
  steps: [
    { id: "1", label: "Reading permissions.ts", done: true },
    { id: "2", label: "Searching for menu registration", done: false },
  ],
  result:
    STATE === "verified"
      ? {
          summary: "The key is wired up, but nothing covers it.",
          complete: false,
          criteria: [
            {
              text: "Menu entry uses the real permission key",
              satisfied: true,
              evidence: "src/menu/customer.ts line 42 reads PERMISSIONS.customer.view.",
            },
            {
              text: "Unauthorised users cannot see the entry",
              satisfied: true,
              evidence: "Guarded by hasPermission() in src/menu/index.ts.",
            },
            {
              text: "A test covers the permission check",
              satisfied: false,
              evidence: "No test references the customer menu entry.",
            },
          ],
        }
      : null,
  satisfied: 2,
  total: 3,
  recommendation: "Keep task open.",
  error: null,
  startedAt: Math.floor(Date.now() / 1000) - 40,
};

const CHANGES = {
  isGit: true,
  insertions: 182,
  deletions: 24,
  files: [
    { path: "src/menu/customer.ts", state: "modified", insertions: 46, deletions: 12 },
    { path: "src/permissions/keys.ts", state: "modified", insertions: 8, deletions: 2 },
    { path: "src/pages/customer/table.vue", state: "added", insertions: 118, deletions: 0 },
    { path: "src/legacy/customer-old.ts", state: "deleted", insertions: 0, deletions: 10 },
    { path: "notes.md", state: "untracked", insertions: 10, deletions: 0 },
  ],
};

const DIFF = `--- a/src/permissions/keys.ts
+++ b/src/permissions/keys.ts
@@ -12,7 +12,9 @@ export const PERMISSIONS = {
   invoice: {
     view: "invoice.view",
   },
-  customer: {},
+  customer: {
+    view: "customer.view",
+  },
 };`;

export async function invoke(command: string, args?: unknown): Promise<unknown> {
  switch (command) {
    case "bootstrap":
      return {
        settings: SETTINGS,
        projects: [PROJECT, { ...PROJECT, id: 2, name: "storefront", path: "/Users/dev/projects/storefront", branch: "main" }],
        activeProject: PROJECT,
        tasks: TASKS,
        categories: CATEGORIES,
        focus: FOCUS,
        rest: REST,
        analysis: ANALYSIS,
        goal: GOAL,
        execution: RUN,
        verification: VERIFICATION,
      };
    case "list_tasks":
      return TASKS;
    case "list_categories":
      return CATEGORIES;
    case "suggest_criteria":
      return [
        "User can request a password reset",
        "Reset email is sent",
        "Expired token shows an error",
        "Password can be changed",
        "Existing sessions are invalidated",
        "Tests cover success and failure cases",
      ];
    case "completed_tasks":
      return HISTORY;
    case "get_settings":
      return SETTINGS;
    // Enough of a write to review the screens that toggle something.
    case "update_settings":
      Object.assign(SETTINGS, (args as { patch?: Record<string, unknown> })?.patch ?? {});
      return SETTINGS;
    case "check_focus_block":
      return new URLSearchParams(location.search).get("blockPermission") ?? "granted";
    case "focus_snapshot":
      return FOCUS;
    case "rest_snapshot":
      return REST;
    case "slack_account":
      return { userId: "U1", user: "cleonart", team: "Maverick" };
    case "slack_waiting":
      return [
        {
          id: "D1",
          with: "Priya",
          waitingSeconds: 2400,
          messages: [
            { user: "U2", author: "Priya", text: "morning! quick one", ts: "1.0", isMine: false },
            {
              user: "U2",
              author: "Priya",
              text: "is the invoice PDF download done, or still in progress? asking for the release notes",
              ts: "2.0",
              isMine: false,
            },
          ],
        },
        {
          id: "D2",
          with: "Tom",
          waitingSeconds: 480,
          messages: [
            { user: "U3", author: "Tom", text: "can you take a look at the i18n task today?", ts: "3.0", isMine: false },
          ],
        },
      ];
    case "draft_slack_reply":
      return "Not quite — the PDF service is done and the download endpoint is in progress. Ownership check and the button are still open, so I would not put it in the release notes yet.";
    case "agent_status":
      return AGENT_STATUS;
    case "agent_models":
      return SETTINGS.agent === "codex"
        ? [
            { id: "gpt-5.5", displayName: "GPT-5.5", description: "Frontier model for complex coding.", isDefault: true },
            { id: "gpt-5.4", displayName: "gpt-5.4", description: "Strong model for everyday coding.", isDefault: false },
          ]
        : [
            { id: "opus", displayName: "Opus", description: "Deepest reasoning", isDefault: false },
            { id: "sonnet", displayName: "Sonnet", description: "Balanced for everyday work", isDefault: false },
            { id: "haiku", displayName: "Haiku", description: "Fastest", isDefault: false },
          ];
    case "popup_shortcut":
      return { combo: "\u2303\u2325B", problem: null };
    case "latest_goal":
      return GOAL;
    case "analysis_snapshot":
      return ANALYSIS;
    case "execution_snapshot":
      return RUN;
    case "task_has_thread":
      return STATE === "ran";
    case "verification_snapshot":
      return VERIFICATION;
    case "performance_stats":
      return {
        focusTodaySeconds: 7500,
        tasksDoneWeek: 6,
        sessionsWeek: 14,
        averageSessionSeconds: 1380,
      };
    case "repo_changes":
      return CHANGES;
    case "file_diff":
      return DIFF;
    default:
      return undefined;
  }
}

type Handler = (event: { payload: unknown }) => void;
const HANDLERS = new Map<string, Set<Handler>>();

export async function listen(event: string, handler: Handler): Promise<() => void> {
  const set = HANDLERS.get(event) ?? new Set<Handler>();
  set.add(handler);
  HANDLERS.set(event, set);
  return () => set.delete(handler);
}

function emit(event: string, payload: unknown) {
  for (const handler of HANDLERS.get(event) ?? []) handler({ payload });
}

/**
 * `?ticking=1` runs the focus clock, so the screens that change once a second
 * can be reviewed as they behave rather than as a still.
 */
if (new URLSearchParams(location.search).get("ticking")) {
  let remaining = FOCUS.remainingSeconds;
  let elapsed = FOCUS.elapsedSeconds;
  setInterval(() => {
    remaining -= 1;
    elapsed += 1;
    emit("focus:tick", { ...FOCUS, remainingSeconds: remaining, elapsedSeconds: elapsed });
  }, 300);
}

export function getCurrentWindow() {
  return {
    label: new URLSearchParams(location.search).get("surface") ?? "popup",
    onFocusChanged: async () => () => {},
  };
}

export async function open(): Promise<string | null> {
  return null;
}

export async function openUrl(url: string) {
  console.info("preview: would open", url);
}

export async function revealItemInDir(): Promise<void> {}
