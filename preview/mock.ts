// Stand-in for the Tauri runtime so the UI can be rendered in a browser for
// visual review. Preview-only: never imported by the app itself.
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
    category: "backend",
    status: "in-progress",
    position: 0,
    estimateMinutes: 45,
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
    focusSeconds: 1800,
  },
  {
    id: 2,
    projectId: 1,
    goalId: 1,
    title: "Audit and complete customer i18n coverage",
    description: null,
    category: "backend",
    status: "ready",
    position: 1,
    estimateMinutes: 30,
    isAiGenerated: true,
    createdAt: 0,
    updatedAt: 0,
    completedAt: null,
    criteria: [],
    files: [],
    dependsOn: [1],
    focusSeconds: 0,
  },
  {
    id: 3,
    projectId: 1,
    goalId: 1,
    title: "Verify service endpoints against api_spec.json",
    description: null,
    category: "security",
    status: "ready",
    position: 2,
    estimateMinutes: 20,
    isAiGenerated: true,
    createdAt: 0,
    updatedAt: 0,
    completedAt: null,
    criteria: [],
    files: [],
    dependsOn: [2],
    focusSeconds: 0,
  },
  {
    id: 4,
    projectId: 1,
    goalId: 1,
    title: "Create design-code doc and registry row for the customer screen",
    description: null,
    category: "frontend",
    status: "ready",
    position: 3,
    estimateMinutes: 25,
    isAiGenerated: true,
    createdAt: 0,
    updatedAt: 0,
    completedAt: null,
    criteria: [],
    files: [],
    dependsOn: [],
    focusSeconds: 0,
  },
  {
    id: 5,
    projectId: 1,
    goalId: null,
    title: "Add tests for customer table, modal, and page hook",
    description: null,
    category: "testing",
    status: "completed",
    position: 4,
    estimateMinutes: null,
    isAiGenerated: false,
    createdAt: 0,
    updatedAt: 0,
    completedAt: 1,
    criteria: [],
    files: [],
    dependsOn: [],
    focusSeconds: 0,
  },
];

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
      category: "backend",
      acceptanceCriteria: ["Generates a valid PDF", "Includes line items", "Handles a missing invoice"],
      relevantFiles: ["src/services/invoice-pdf.ts"],
      dependsOn: [],
      estimateMinutes: 45,
    },
    {
      title: "Audit and complete customer i18n coverage",
      description: "Serve the generated PDF over HTTP.",
      category: "backend",
      acceptanceCriteria: ["Returns application/pdf", "404s for unknown invoices"],
      relevantFiles: ["src/api/invoices/download.ts"],
      dependsOn: [1],
      estimateMinutes: 30,
    },
    {
      title: "Check invoice ownership",
      description: "Only the owning customer may download an invoice.",
      category: "security",
      acceptanceCriteria: ["Rejects another customer's invoice"],
      relevantFiles: ["src/middleware/auth.ts"],
      dependsOn: [2],
      estimateMinutes: 20,
    },
    {
      title: "Add a Download PDF button",
      description: "Surface the download on the invoice detail page.",
      category: "frontend",
      acceptanceCriteria: ["Button appears for paid invoices"],
      relevantFiles: ["src/pages/invoice.vue"],
      dependsOn: [2],
      estimateMinutes: 25,
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

export async function invoke(command: string): Promise<unknown> {
  switch (command) {
    case "bootstrap":
      return {
        settings: SETTINGS,
        projects: [PROJECT, { ...PROJECT, id: 2, name: "storefront", path: "/Users/dev/projects/storefront", branch: "main" }],
        activeProject: PROJECT,
        tasks: TASKS,
        focus: FOCUS,
        analysis: ANALYSIS,
        goal: GOAL,
      };
    case "list_tasks":
      return TASKS;
    case "get_settings":
      return SETTINGS;
    case "focus_snapshot":
      return FOCUS;
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
    default:
      return undefined;
  }
}

export async function listen(): Promise<() => void> {
  return () => {};
}

export function getCurrentWindow() {
  return { onFocusChanged: async () => () => {} };
}

export async function open(): Promise<string | null> {
  return null;
}

export async function revealItemInDir(): Promise<void> {}
