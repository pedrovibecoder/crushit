export type TaskCategory =
  | "frontend"
  | "backend"
  | "database"
  | "security"
  | "testing"
  | "devops"
  | "refactor"
  | "bug";

export type TaskStatus =
  | "backlog"
  | "ready"
  | "in-progress"
  | "needs-review"
  | "completed"
  | "blocked";

export type FocusStatus = "idle" | "running" | "paused" | "finished";

export interface Project {
  id: number;
  path: string;
  name: string;
  isGit: boolean;
  branch: string | null;
  createdAt: number;
  lastOpenedAt: number;
}

export interface ProjectInspection {
  path: string;
  name: string;
  exists: boolean;
  isGit: boolean;
  branch: string | null;
  manifests: string[];
  alreadyAdded: boolean;
}

export interface AcceptanceCriterion {
  id: number;
  taskId: number;
  text: string;
  isMet: boolean;
  position: number;
}

export interface Task {
  id: number;
  projectId: number;
  goalId: number | null;
  title: string;
  description: string | null;
  category: TaskCategory;
  status: TaskStatus;
  position: number;
  estimateMinutes: number | null;
  isAiGenerated: boolean;
  createdAt: number;
  updatedAt: number;
  completedAt: number | null;
  criteria: AcceptanceCriterion[];
  files: string[];
  dependsOn: number[];
  focusSeconds: number;
}

export interface NewTask {
  projectId: number;
  title: string;
  description?: string | null;
  category?: TaskCategory;
  status?: TaskStatus;
  estimateMinutes?: number | null;
  criteria?: string[];
  files?: string[];
}

export interface TaskPatch {
  title?: string;
  description?: string;
  category?: TaskCategory;
  status?: TaskStatus;
  estimateMinutes?: number;
  criteria?: string[];
  files?: string[];
  dependsOn?: number[];
}

export interface FocusSnapshot {
  status: FocusStatus;
  sessionId: number | null;
  taskId: number | null;
  durationSeconds: number;
  elapsedSeconds: number;
  remainingSeconds: number;
}

export interface Settings {
  focusMinutes: number;
  showTimerInMenuBar: boolean;
  hidePopupOnBlur: boolean;
  activeProjectId: number | null;
  agent: Agent;
  codexPath: string | null;
  codexModel: string | null;
  claudePath: string | null;
  claudeModel: string | null;
}

export interface SettingsPatch {
  focusMinutes?: number;
  showTimerInMenuBar?: boolean;
  hidePopupOnBlur?: boolean;
  activeProjectId?: number | null;
  agent?: Agent;
  codexPath?: string | null;
  codexModel?: string | null;
  claudePath?: string | null;
  claudeModel?: string | null;
}

export type GoalStatus =
  | "draft"
  | "analyzing"
  | "ready"
  | "failed"
  | "accepted"
  | "discarded";

export interface PlannedTask {
  title: string;
  description: string;
  category: TaskCategory;
  acceptanceCriteria: string[];
  relevantFiles: string[];
  /** 1-based positions of earlier tasks in the same plan. */
  dependsOn: number[];
  estimateMinutes: number;
}

export interface Plan {
  summary: string;
  existing: string[];
  missing: string[];
  tasks: PlannedTask[];
}

export interface Goal {
  id: number;
  projectId: number;
  title: string;
  prompt: string;
  status: GoalStatus;
  agent: Agent;
  codexThreadId: string | null;
  error: string | null;
  createdAt: number;
  updatedAt: number;
  plan: Plan | null;
}

export type AnalysisStatus = "idle" | "running" | "ready" | "failed";

export interface AnalysisStep {
  id: string;
  label: string;
  done: boolean;
}

export interface AnalysisSnapshot {
  status: AnalysisStatus;
  goalId: number | null;
  projectId: number | null;
  goalTitle: string;
  steps: AnalysisStep[];
  error: string | null;
  startedAt: number;
}

export interface ShortcutInfo {
  combo: string | null;
  problem: string | null;
}

export type Agent = "codex" | "claude-code";

export const AGENTS: Agent[] = ["codex", "claude-code"];

export const AGENT_LABELS: Record<Agent, string> = {
  codex: "Codex",
  "claude-code": "Claude Code",
};

export interface AgentStatus {
  agent: Agent;
  installed: boolean;
  path: string | null;
  version: string | null;
  signedIn: boolean;
  authMode: string | null;
  problem: string | null;
}

export interface AgentModel {
  id: string;
  displayName: string;
  description: string | null;
  isDefault: boolean;
}

export interface Bootstrap {
  settings: Settings;
  projects: Project[];
  activeProject: Project | null;
  tasks: Task[];
  focus: FocusSnapshot;
  analysis: AnalysisSnapshot;
  goal: Goal | null;
}

export const TASK_CATEGORIES: TaskCategory[] = [
  "frontend",
  "backend",
  "database",
  "security",
  "testing",
  "devops",
  "refactor",
  "bug",
];

export const CATEGORY_LABELS: Record<TaskCategory, string> = {
  frontend: "Frontend",
  backend: "Backend",
  database: "Database",
  security: "Security",
  testing: "Testing",
  devops: "DevOps",
  refactor: "Refactor",
  bug: "Bug",
};

/** A single dot of colour is enough to tell categories apart at small sizes. */
export const CATEGORY_DOTS: Record<TaskCategory, string> = {
  frontend: "#4a9bf5",
  backend: "#7b61ff",
  database: "#3fbf6a",
  security: "#e05656",
  testing: "#e2a32c",
  devops: "#20b1c4",
  refactor: "#8a8f9a",
  bug: "#e0609b",
};

export const STATUS_LABELS: Record<TaskStatus, string> = {
  backlog: "Backlog",
  ready: "Ready",
  "in-progress": "In Progress",
  "needs-review": "Needs Review",
  completed: "Completed",
  blocked: "Blocked",
};
