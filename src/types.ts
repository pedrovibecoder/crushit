/**
 * A category slug. Categories are rows the developer manages in Settings, so
 * this is any string the backend knows about rather than a fixed union.
 */
export type TaskCategory = string;

export interface Category {
  id: number;
  slug: TaskCategory;
  label: string;
  /** Hex colour for the dot that tells categories apart at small sizes. */
  color: string;
  position: number;
}

export interface CategoryPatch {
  label?: string;
  color?: string;
}

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
  /** Relative size on the Fibonacci scale, when one has been estimated. */
  storyPoints: number | null;
  /** The day this task sits on, as `YYYY-MM-DD` on the local clock. */
  plannedFor: string;
  isAiGenerated: boolean;
  createdAt: number;
  updatedAt: number;
  completedAt: number | null;
  criteria: AcceptanceCriterion[];
  files: string[];
  dependsOn: number[];
  focusSeconds: number;
  focusSessions: number;
}

/** One finished task, as the history graph needs it. */
export interface CompletedTask {
  id: number;
  title: string;
  category: TaskCategory;
  storyPoints: number | null;
  completedAt: number;
}

export interface NewTask {
  projectId: number;
  title: string;
  description?: string | null;
  category?: TaskCategory;
  status?: TaskStatus;
  estimateMinutes?: number | null;
  storyPoints?: number | null;
  plannedFor?: string;
  criteria?: string[];
  files?: string[];
}

export interface TaskPatch {
  title?: string;
  description?: string;
  category?: TaskCategory;
  status?: TaskStatus;
  estimateMinutes?: number;
  storyPoints?: number;
  plannedFor?: string;
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

export type RestStatus = "idle" | "running" | "finished";

export interface RestSnapshot {
  status: RestStatus;
  minutes: number;
  durationSeconds: number;
  elapsedSeconds: number;
  remainingSeconds: number;
}

/** What a break may be set to, matching the backend's own list. */
export const REST_CHOICES = [5, 10, 20, 30] as const;

export interface SlackAccount {
  userId: string;
  user: string;
  team: string;
}

export interface SlackMessage {
  user: string;
  author: string;
  text: string;
  ts: string;
  isMine: boolean;
}

export interface WaitingConversation {
  id: string;
  with: string;
  messages: SlackMessage[];
  waitingSeconds: number;
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
  /** Whether a Slack token is saved. The token itself never leaves the backend. */
  slackConnected: boolean;
  theme: Theme;
  notifications: boolean;
  sounds: boolean;
  launchAtLogin: boolean;
  /** Send blocked sites to a holding page while a session is running. */
  focusBlockEnabled: boolean;
  /** The sites that are shut during a session, as bare hosts. */
  focusBlockSites: string[];
  onboarded: boolean;
}

/** Whether macOS lets the app ask a browser what it is showing. */
export type BlockPermission = "unknown" | "granted" | "denied";

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
  theme?: Theme;
  notifications?: boolean;
  sounds?: boolean;
  launchAtLogin?: boolean;
  focusBlockEnabled?: boolean;
  /** Written as typed — the backend reads the list leniently. */
  focusBlockSites?: string;
  onboarded?: boolean;
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
  /** Relative size on the Fibonacci scale; 0 when the agent gave none. */
  storyPoints: number;
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

export type ExecutionStatus =
  | "idle"
  | "running"
  | "awaiting-approval"
  | "finished"
  | "failed";

export interface ActivityItem {
  id: string;
  label: string;
  kind: string;
  done: boolean;
}

export interface ApprovalRequest {
  id: number;
  title: string;
  command: string | null;
  reason: string | null;
}

export interface ExecutionSnapshot {
  status: ExecutionStatus;
  taskId: number | null;
  taskTitle: string;
  agent: Agent;
  threadId: string | null;
  activity: ActivityItem[];
  changedFiles: string[];
  approval: ApprovalRequest | null;
  summary: string | null;
  error: string | null;
  startedAt: number;
}

export interface CriterionVerdict {
  text: string;
  satisfied: boolean;
  evidence: string;
}

export interface Verification {
  summary: string;
  criteria: CriterionVerdict[];
  complete: boolean;
}

export type VerificationStatus = "idle" | "running" | "ready" | "failed";

export interface VerificationSnapshot {
  status: VerificationStatus;
  taskId: number | null;
  taskTitle: string;
  steps: AnalysisStep[];
  result: Verification | null;
  satisfied: number;
  total: number;
  recommendation: string;
  error: string | null;
  startedAt: number;
}

export interface ChangedFile {
  path: string;
  state: string;
  insertions: number;
  deletions: number;
}

export interface RepoChanges {
  files: ChangedFile[];
  insertions: number;
  deletions: number;
  isGit: boolean;
}

export interface Stats {
  focusTodaySeconds: number;
  tasksDoneWeek: number;
  sessionsWeek: number;
  averageSessionSeconds: number;
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
  categories: Category[];
  focus: FocusSnapshot;
  rest: RestSnapshot;
  analysis: AnalysisSnapshot;
  goal: Goal | null;
  execution: ExecutionSnapshot;
  verification: VerificationSnapshot;
}

/**
 * What a new category can be coloured. Fixed hexes rather than CSS variables:
 * the palette is now data the developer owns, so it has to survive in the
 * database and read the same in both themes.
 */
/**
 * The story-point scale. Fibonacci, because the gaps are the point: the
 * difference between 8 and 13 is a real judgement, between 8 and 9 is noise.
 */
export const STORY_POINTS = [1, 2, 3, 5, 8, 13] as const;

/** The file types that can be dropped on the window to become tasks. */
export const IMPORTABLE_EXTENSIONS = [
  "png", "jpg", "jpeg", "gif", "webp", "heic",
  "csv", "tsv", "xlsx", "xls", "numbers",
  "md", "txt", "json",
];

export function isImportable(path: string): boolean {
  const extension = path.split(".").pop()?.toLowerCase() ?? "";
  return IMPORTABLE_EXTENSIONS.includes(extension);
}

export const CATEGORY_COLORS = [
  "#4a9bf5",
  "#7b61ff",
  "#3fbf6a",
  "#e05656",
  "#e2a32c",
  "#20b1c4",
  "#8a8f9a",
  "#e0609b",
] as const;

/** Falls back for a task filed under a category that no longer exists. */
export const UNKNOWN_CATEGORY_COLOR = "#8a8f9a";

export type Theme = "light" | "github-dark";

export const THEMES: Theme[] = ["light", "github-dark"];

export const THEME_LABELS: Record<Theme, string> = {
  light: "Light",
  "github-dark": "GitHub Dark",
};

export const STATUS_LABELS: Record<TaskStatus, string> = {
  backlog: "Backlog",
  ready: "Ready",
  "in-progress": "In Progress",
  "needs-review": "Needs Review",
  completed: "Completed",
  blocked: "Blocked",
};
