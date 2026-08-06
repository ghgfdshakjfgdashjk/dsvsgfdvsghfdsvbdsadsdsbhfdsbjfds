import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getVersion } from "@tauri-apps/api/app";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

interface Profile {
  name: string;
  enabled: boolean;
  mode: string;
  bindEnabled: boolean;
  bindVk: number;
  button: string;
  delivery: string;
  targetMode: string;
  targetTitle: string;
  targetProcess: string;
  targetX: number;
  targetY: number;
  points: { x: number; y: number }[];
  rateMode: string;
  cpsMin: number;
  cpsMax: number;
  randomize: boolean;
  jitter: number;
  dutyEnabled: boolean;
  dutyCycle: number;
  precision: string;
  limitEnabled: boolean;
  limitCount: number;
  timeLimitEnabled: boolean;
  timeLimitSecs: number;
  startDelayEnabled: boolean;
  startDelayMs: number;
  filterEnabled: boolean;
  filterTitle: string;
  sequenceEnabled: boolean;
  sequence: string;
  burstEnabled: boolean;
  burstCount: number;
  burstPauseMs: number;
  pixelEnabled: boolean;
  pixelX: number;
  pixelY: number;
  pixelRgb: number;
  pixelTolerance: number;
  pixelStopOn: string;
  shakeEnabled: boolean;
  shakePx: number;
  shakeMs: number;
}

interface Preset {
  name: string;
  code: string;
}

interface Settings {
  profiles: Profile[];
  selected: number;
  panicVk: number;
  edgeGuardEnabled: boolean;
  edgeGuardPx: number;
  edgeGuardMode: string;
  edgeGuardChrome: boolean;
  theme: string;
  accentHue: number;
  accentSat: number;
  cursorStyle: string;
  cursorImage: string;
  cursorSize: number;
  windowWidth: number;
  windowHeight: number;
  alwaysOnTop: boolean;
  blurEnabled: boolean;
  acrylic: boolean;
  opacity: number;
  presets: Preset[];
}

interface TargetInfo {
  title: string;
  process: string;
  rawInput: boolean;
}

interface WindowEntry {
  title: string;
  process: string;
  rawInput: boolean;
}

interface ClickerStatus {
  name: string;
  active: boolean;
  clicks: number;
  cps: number;
  guarded: boolean;
  totalClicks: number;
  activeSeconds: number;
  target: string;
}

interface Status {
  clickers: ClickerStatus[];
  running: number;
  capturing: boolean;
  totalClicks: number;
  activeSeconds: number;
  cpuPercent: number;
}

const CPS_CEILING = 50000;
const SLIDER_STEPS = 1000;

const VK_NAMES: Record<number, string> = {
  0x01: "Mouse L",
  0x02: "Mouse R",
  0x04: "Mouse M",
  0x05: "Mouse 4",
  0x06: "Mouse 5",
  0x08: "Backspace",
  0x09: "Tab",
  0x0d: "Enter",
  0x10: "Shift",
  0x11: "Ctrl",
  0x12: "Alt",
  0x13: "Pause",
  0x14: "Caps",
  0x1b: "Esc",
  0x20: "Space",
  0x21: "Page Up",
  0x22: "Page Dn",
  0x23: "End",
  0x24: "Home",
  0x25: "Left",
  0x26: "Up",
  0x27: "Right",
  0x28: "Down",
  0x2c: "PrtScn",
  0x2d: "Insert",
  0x2e: "Delete",
  0x5b: "L Win",
  0x5c: "R Win",
  0x5d: "Menu",
  0x6a: "Num *",
  0x6b: "Num +",
  0x6d: "Num -",
  0x6e: "Num .",
  0x6f: "Num /",
  0x90: "NumLock",
  0x91: "ScrLock",
  0xa0: "L Shift",
  0xa1: "R Shift",
  0xa2: "L Ctrl",
  0xa3: "R Ctrl",
  0xa4: "L Alt",
  0xa5: "R Alt",
  0xba: ";",
  0xbb: "=",
  0xbc: ",",
  0xbd: "-",
  0xbe: ".",
  0xbf: "/",
  0xc0: "`",
  0xdb: "[",
  0xdc: "\\",
  0xdd: "]",
  0xde: "'",
};

for (let code = 0x30; code <= 0x39; code += 1) {
  VK_NAMES[code] = String.fromCharCode(code);
}
for (let code = 0x41; code <= 0x5a; code += 1) {
  VK_NAMES[code] = String.fromCharCode(code);
}
for (let n = 0; n <= 9; n += 1) {
  VK_NAMES[0x60 + n] = `Num ${n}`;
}
for (let n = 1; n <= 24; n += 1) {
  VK_NAMES[0x6f + n] = `F${n}`;
}

function vkLabel(vk: number): string {
  if (!vk) return "None";
  return VK_NAMES[vk] ?? `VK ${vk.toString(16).toUpperCase()}`;
}

const BUTTON_VK: Record<string, number> = {
  left: 0x01,
  right: 0x02,
  middle: 0x04,
  mouse4: 0x05,
  mouse5: 0x06,
};

function noAutofill(input: HTMLInputElement): HTMLInputElement {
  input.setAttribute("autocomplete", "off");
  input.setAttribute("autocorrect", "off");
  input.setAttribute("autocapitalize", "off");
  input.setAttribute("data-form-type", "other");
  input.spellcheck = false;
  return input;
}

function el<T extends HTMLElement>(id: string): T {
  const node = document.getElementById(id);
  if (!node) throw new Error(`missing element #${id}`);
  return node as T;
}

function paintRange(input: HTMLInputElement): void {
  const min = Number(input.min || 0);
  const max = Number(input.max || 100);
  const value = Number(input.value || 0);
  const pct = max > min ? ((value - min) / (max - min)) * 100 : 0;
  input.style.setProperty("--fill", `${pct}%`);
}

function sliderToCps(position: number): number {
  const ratio = Math.min(Math.max(position / SLIDER_STEPS, 0), 1);
  const value = Math.exp(ratio * Math.log(CPS_CEILING));

  const rounded = value < 10 ? Math.round(value * 100) / 100 : Math.round(value);
  return Math.min(CPS_CEILING, Math.max(0.01, rounded));
}

function cpsToSlider(cps: number): number {
  const clamped = Math.min(CPS_CEILING, Math.max(0.01, cps));
  return Math.round((Math.log(clamped) / Math.log(CPS_CEILING)) * SLIDER_STEPS);
}

function formatCps(value: number): string {
  if (value >= 100) return String(Math.round(value));
  return String(Math.round(value * 100) / 100);
}

const MIN_DELAY_MS = 1000 / CPS_CEILING;
const MAX_DELAY_MS = 1000 / 0.01;

function cpsToDelay(cps: number): number {
  return 1000 / Math.max(cps, 0.01);
}

function delayToCps(ms: number): number {
  return 1000 / Math.min(Math.max(ms, MIN_DELAY_MS), MAX_DELAY_MS);
}

function formatDelay(ms: number): string {
  if (ms >= 100) return String(Math.round(ms));
  if (ms >= 1) return String(Math.round(ms * 100) / 100);
  return String(Number(ms.toPrecision(2)));
}

function delayMode(): boolean {
  return profile.rateMode === "delay";
}

function positionToCps(position: number): number {
  return delayMode()
    ? sliderToCps(SLIDER_STEPS - position)
    : sliderToCps(position);
}

function cpsToPosition(cps: number): number {
  const base = cpsToSlider(cps);
  return delayMode() ? SLIDER_STEPS - base : base;
}

function formatCount(value: number): string {
  return Math.round(value).toLocaleString("en-US");
}

function trimNum(value: number): string {
  return String(Math.round(value * 100) / 100);
}

function wirePair(
  slider: HTMLInputElement,
  field: HTMLInputElement,
  apply: (value: number) => void,
  redraw: () => void,
): void {
  const commit = (raw: string, fallback: number) => {
    const parsed = Number(raw);
    apply(Number.isFinite(parsed) ? parsed : fallback);
    redraw();
    push();
  };

  slider.addEventListener("input", () => commit(slider.value, 0));
  field.addEventListener("change", () => commit(field.value, Number(slider.value)));
}

function formatDuration(seconds: number): string {
  const total = Math.floor(seconds);
  if (total < 60) return `${total}s`;

  const minutes = Math.floor(total / 60);
  if (minutes < 60) return `${minutes}m ${String(total % 60).padStart(2, "0")}s`;

  const hours = Math.floor(minutes / 60);
  return `${hours}h ${String(minutes % 60).padStart(2, "0")}m`;
}

type Step =
  | { kind: "move"; x: number; y: number }
  | { kind: "click"; button: string; count: number }
  | { kind: "key"; vk: number }
  | { kind: "text"; value: string }
  | { kind: "wait"; ms: number }
  | { kind: "scroll"; amount: number };

interface Automation {
  bindEnabled: boolean;
  bindVk: number;
  repeat: number;
  stepDelayMs: number;
  steps: Step[];
}

interface AutomationStatus {
  running: boolean;
  step: number;
  pass: number;
}

interface TweakState {
  id: string;
  label: string;
  detail: string;
  optimised: boolean;
  readable: boolean;
}

interface AdminTweakState {
  id: string;
  label: string;
  detail: string;
  optimised: boolean;
  readable: boolean;
  reboot: boolean;
}

interface CleanupState {
  id: string;
  label: string;
  detail: string;
  destructive: boolean;
}

interface Optimizations {
  tweaks: TweakState[];
  powerPlan: string;
  admin: AdminTweakState[];
  cleanups: CleanupState[];
}

interface Fisher {
  bindEnabled: boolean;
  bindVk: number;
  types: boolean[];
  colors: number[];
  tolerance: number;
  castButton: string;
  castDelayMs: number;
  recastDelayMs: number;
  rejectVk: number;
  rejectDelayMs: number;
  biteTimeoutSecs: number;
  fightTimeoutSecs: number;
  deadzone: number;
  searchTop: number;
  searchBottom: number;
  searchLeft: number;
  searchRight: number;
}

interface FisherStatus {
  running: boolean;
  phase: string;
  detail: string;
  caught: number[];
  rejected: number;
  missed: number;
  barFound: boolean;
  log: string[];
}

const FISH_KINDS = [
  { key: "iron", label: "Iron", note: "grey" },
  { key: "special", label: "Special", note: "red" },
  { key: "emerald", label: "Emerald", note: "green" },
  { key: "diamond", label: "Diamond", note: "blue" },
  { key: "gold", label: "Gold", note: "yellow" },
];

interface Gumdrop {
  bindEnabled: boolean;
  bindVk: number;
  gumdropSlot: number;
  pickaxeSlot: number;
  swordSlot: number;
  keyHoldMs: number;
  clickHoldMs: number;
  afterGumdropMs: number;
  placeWaitMs: number;
  afterPickaxeMs: number;
  afterBreakMs: number;
}

interface GumdropStatus {
  busy: boolean;
  runs: number;
}

interface Skywars {
  bindEnabled: boolean;
  bindVk: number;
  clickHoldMs: number;
  settleMs: number;
  betweenMs: number;
  clicksPerItem: number;
  retryGapMs: number;
  restoreCursor: boolean;
}

interface Davey {
  bindEnabled: boolean;
  bindVk: number;
  holdVk: number;
  holdMs: number;
  pickaxeSlot: number;
  keyHoldMs: number;
  burstCps: number;
  burstMs: number;
  burstDuty: number;
}

interface Overlay {
  enabled: boolean;
  position: string;
  x: number;
  y: number;
  onlyInWindows: boolean;
  windows: string[];
}

interface Crossbow {
  bindEnabled: boolean;
  bindVk: number;
  crossbowSlot: number;
  swordSlot: number;
  tacticalEnabled: boolean;
  tacticalSlot: number;
  secondSwitchMs: number;
  keyHoldMs: number;
  afterSwitchMs: number;
  clickHoldMs: number;
  afterClickMs: number;
}

interface CrossbowStatus {
  busy: boolean;
  runs: number;
}

interface DaveyStatus {
  busy: boolean;
  runs: number;
}

interface SkywarsStatus {
  busy: boolean;
  runs: number;
  taken: number;
  note: string;
}

type CaptureTarget =
  | { kind: "bind" }
  | { kind: "panic" }
  | { kind: "autoBind" }
  | { kind: "fisherBind" }
  | { kind: "dropBind" }
  | { kind: "skyBind" }
  | { kind: "dvyBind" }
  | { kind: "dvyHoldKey" }
  | { kind: "bowBind" }
  | { kind: "stepKey"; index: number }
  | { kind: "position"; index: number }
  | { kind: "clickPoint" }
  | { kind: "extraPoint" }
  | { kind: "pixel" };

let settings!: Settings;

let profile!: Profile;
let automation!: Automation;
let fisher!: Fisher;
let fisherPushTimer: number | undefined;
let gumdrop!: Gumdrop;
let dropPushTimer: number | undefined;
let skywars!: Skywars;
let davey!: Davey;
let crossbow!: Crossbow;
let overlay!: Overlay;
let skyPushTimer: number | undefined;
let capturing: CaptureTarget | null = null;
let pushTimer: number | undefined;
let autoPushTimer: number | undefined;

const titleDot = el<HTMLSpanElement>("titleDot");
const heroCard = document.querySelector<HTMLDivElement>(".hero")!;
const cpsGraph = el<HTMLCanvasElement>("cpsGraph");
const heroStatus = el<HTMLSpanElement>("heroStatus");
const heroSub = el<HTMLSpanElement>("heroSub");
const powerBtn = el<HTMLButtonElement>("btnPower");
const powerLabel = el<HTMLSpanElement>("powerLabel");

const modeGroup = el<HTMLDivElement>("modeGroup");
const modeHint = el<HTMLParagraphElement>("modeHint");
const buttonGroup = el<HTMLDivElement>("buttonGroup");
const buttonHint = el<HTMLParagraphElement>("buttonHint");
const deliveryDirect = el<HTMLInputElement>("deliveryDirect");
const deliveryHint = el<HTMLParagraphElement>("deliveryHint");
const targetRow = el<HTMLDivElement>("targetRow");
const targetName = el<HTMLSpanElement>("targetName");
const targetModeWrap = el<HTMLDivElement>("targetModeWrap");
const targetModeGroup = el<HTMLDivElement>("targetModeGroup");
const targetModeHint = el<HTMLParagraphElement>("targetModeHint");
const pinnedWrap = el<HTMLDivElement>("pinnedWrap");
const windowList = el<HTMLDivElement>("windowList");
const refreshWindowsBtn = el<HTMLButtonElement>("btnRefreshWindows");
const pickPointBtn = el<HTMLButtonElement>("btnPickPoint");
const resetPointBtn = el<HTMLButtonElement>("btnResetPoint");
const pinnedPointLabel = el<HTMLSpanElement>("pinnedPointLabel");
const rawInputWarn = el<HTMLDivElement>("rawInputWarn");
const rawInputName = el<HTMLElement>("rawInputName");
const precisionGroup = el<HTMLDivElement>("precisionGroup");
const precisionHint = el<HTMLParagraphElement>("precisionHint");
const conflictWarn = el<HTMLDivElement>("conflictWarn");

const bindToggle = el<HTMLInputElement>("bindEnabled");
const bindWrap = el<HTMLDivElement>("bindWrap");
const bindHint = el<HTMLParagraphElement>("bindHint");
const bindBtn = el<HTMLButtonElement>("btnBind");
const bindLabel = el<HTMLSpanElement>("bindLabel");
const panicBtn = el<HTMLButtonElement>("btnPanic");
const panicLabel = el<HTMLSpanElement>("panicLabel");
const clearPanicBtn = el<HTMLButtonElement>("btnClearPanic");

const cpsLabel = el<HTMLLabelElement>("cpsLabel");
const rateModeGroup = el<HTMLDivElement>("rateModeGroup");
const cpsChips = el<HTMLDivElement>("cpsChips");
const cpsMaxSlider = el<HTMLInputElement>("cpsMaxSlider");
const cpsMaxInput = el<HTMLInputElement>("cpsMaxInput");
const cpsMinWrap = el<HTMLDivElement>("cpsMinWrap");
const minLabel = el<HTMLLabelElement>("minLabel");
const cpsMinSlider = el<HTMLInputElement>("cpsMinSlider");
const cpsMinInput = el<HTMLInputElement>("cpsMinInput");

const randomizeToggle = el<HTMLInputElement>("randomize");
const jitterSlider = el<HTMLInputElement>("jitter");
const jitterValue = el<HTMLInputElement>("jitterInput");
const shakeToggle = el<HTMLInputElement>("shakeEnabled");
const shakeWrap = el<HTMLDivElement>("shakeWrap");
const shakePx = el<HTMLInputElement>("shakePx");
const shakeMs = el<HTMLInputElement>("shakeMs");

const dutyToggle = el<HTMLInputElement>("dutyEnabled");
const dutyWrap = el<HTMLDivElement>("dutyWrap");
const dutySlider = el<HTMLInputElement>("dutyCycle");
const dutyValue = el<HTMLInputElement>("dutyInput");
const dutyMs = el<HTMLSpanElement>("dutyMs");
const dutyWarn = el<HTMLDivElement>("dutyWarn");

const sequenceToggle = el<HTMLInputElement>("sequenceEnabled");
const sequenceWrap = el<HTMLDivElement>("sequenceWrap");
const sequenceInput = el<HTMLInputElement>("sequence");
const sequencePreview = el<HTMLDivElement>("sequencePreview");

const limitToggle = el<HTMLInputElement>("limitEnabled");
const limitWrap = el<HTMLDivElement>("limitWrap");
const limitInput = el<HTMLInputElement>("limitCount");
const addPointBtn = el<HTMLButtonElement>("btnAddPoint");
const pointList = el<HTMLDivElement>("pointList");
const pointHint = el<HTMLParagraphElement>("pointHint");

const burstToggle = el<HTMLInputElement>("burstEnabled");
const burstWrap = el<HTMLDivElement>("burstWrap");
const burstCount = el<HTMLInputElement>("burstCount");
const burstPause = el<HTMLInputElement>("burstPause");
const burstHint = el<HTMLParagraphElement>("burstHint");

const pixelToggle = el<HTMLInputElement>("pixelEnabled");
const pixelWrap = el<HTMLDivElement>("pixelWrap");
const pixelSwatch = el<HTMLDivElement>("pixelSwatch");
const pixelStopGroup = el<HTMLDivElement>("pixelStopGroup");
const pixelTolerance = el<HTMLInputElement>("pixelTolerance");
const pixelHint = el<HTMLParagraphElement>("pixelHint");
const pickPixelBtn = el<HTMLButtonElement>("btnPickPixel");
const resamplePixelBtn = el<HTMLButtonElement>("btnResamplePixel");

const timeLimitToggle = el<HTMLInputElement>("timeLimitEnabled");
const timeLimitWrap = el<HTMLDivElement>("timeLimitWrap");
const timeLimitInput = el<HTMLInputElement>("timeLimitInput");
const timeLimitHint = el<HTMLParagraphElement>("timeLimitHint");
const startDelayToggle = el<HTMLInputElement>("startDelayEnabled");
const startDelayWrap = el<HTMLDivElement>("startDelayWrap");
const startDelayInput = el<HTMLInputElement>("startDelayInput");
const filterToggle = el<HTMLInputElement>("filterEnabled");
const filterWrap = el<HTMLDivElement>("filterWrap");
const filterInput = el<HTMLInputElement>("filterTitle");
const edgeGuardToggle = el<HTMLInputElement>("edgeGuardEnabled");
const edgeGuardWrap = el<HTMLDivElement>("edgeGuardWrap");
const edgeGuardSlider = el<HTMLInputElement>("edgeGuardPx");
const edgeGuardValue = el<HTMLInputElement>("edgeGuardInput");
const edgeGuardMode = el<HTMLDivElement>("edgeGuardMode");
const edgeGuardHint = el<HTMLParagraphElement>("edgeGuardHint");
const edgeGuardChrome = el<HTMLInputElement>("edgeGuardChrome");

const statTotalClicks = el<HTMLSpanElement>("statTotalClicks");
const statActiveTime = el<HTMLSpanElement>("statActiveTime");
const statCpu = el<HTMLSpanElement>("statCpu");
const creditVersion = el<HTMLElement>("creditVersion");
const resetStatsBtn = el<HTMLButtonElement>("btnResetStats");

const navIndicator = el<HTMLSpanElement>("navIndicator");
const swatchRow = el<HTMLDivElement>("swatchRow");
const accentSlider = el<HTMLInputElement>("accentHue");
const accentValue = el<HTMLInputElement>("accentInput");
const accentSatSlider = el<HTMLInputElement>("accentSat");
const accentSatValue = el<HTMLInputElement>("accentSatInput");
const themeGroup = el<HTMLDivElement>("themeGroup");
const themeHint = el<HTMLParagraphElement>("themeHint");
const blurToggle = el<HTMLInputElement>("blurEnabled");
const acrylicToggle = el<HTMLInputElement>("acrylic");
const opacitySlider = el<HTMLInputElement>("opacity");
const opacityValue = el<HTMLSpanElement>("opacityValue");
const pinBtn = el<HTMLButtonElement>("btnPin");
const windowSizeHint = el<HTMLParagraphElement>("windowSizeHint");
const resetWindowBtn = el<HTMLButtonElement>("btnResetWindow");

const fisherHero = el<HTMLDivElement>("fisherHero");
const fisherStatusText = el<HTMLSpanElement>("fisherStatus");
const fisherSub = el<HTMLSpanElement>("fisherSub");
const fisherRunBtn = el<HTMLButtonElement>("btnFisherRun");
const fisherRunLabel = el<HTMLSpanElement>("fisherRunLabel");
const fishGrid = el<HTMLDivElement>("fishGrid");
const fisherBindToggle = el<HTMLInputElement>("fisherBindEnabled");
const fisherBindWrap = el<HTMLDivElement>("fisherBindWrap");
const fisherBindBtn = el<HTMLButtonElement>("btnFisherBind");
const fisherBindLabel = el<HTMLSpanElement>("fisherBindLabel");
const fisherResetBtn = el<HTMLButtonElement>("btnFisherReset");
const fisherTrace = el<HTMLPreElement>("fisherTrace");
const fisherEntry = el<HTMLButtonElement>("fisherEntry");
const fisherEntryNote = el<HTMLSpanElement>("fisherEntryNote");
const fisherEntryDot = el<HTMLSpanElement>("fisherEntryDot");
const fisherBody = el<HTMLDivElement>("fisherBody");
const fisherCopyBtn = el<HTMLButtonElement>("btnFisherCopy");
const dropEntry = el<HTMLButtonElement>("dropEntry");
const dropEntryNote = el<HTMLSpanElement>("dropEntryNote");
const dropEntryDot = el<HTMLSpanElement>("dropEntryDot");
const dropBody = el<HTMLDivElement>("dropBody");
const dropGumSlot = el<HTMLInputElement>("dropGumSlot");
const dropPickSlot = el<HTMLInputElement>("dropPickSlot");
const dropSwordSlot = el<HTMLInputElement>("dropSwordSlot");
const dropBindToggle = el<HTMLInputElement>("dropBindEnabled");
const dropBindWrap = el<HTMLDivElement>("dropBindWrap");
const dropBindBtn = el<HTMLButtonElement>("btnDropBind");
const dropBindLabel = el<HTMLSpanElement>("dropBindLabel");
const dropKeyHold = el<HTMLInputElement>("dropKeyHold");
const dropAfterGum = el<HTMLInputElement>("dropAfterGum");
const dropAfterPick = el<HTMLInputElement>("dropAfterPick");
const dropAfterBreak = el<HTMLInputElement>("dropAfterBreak");
const dropClickHold = el<HTMLInputElement>("dropClickHold");
const dropWait = el<HTMLInputElement>("dropWait");
const dropTotal = el<HTMLParagraphElement>("dropTotal");
const dropRuns = el<HTMLParagraphElement>("dropRuns");
const dropRunBtn = el<HTMLButtonElement>("btnDropRun");
const skyEntry = el<HTMLButtonElement>("skyEntry");
const skyEntryNote = el<HTMLSpanElement>("skyEntryNote");
const skyEntryDot = el<HTMLSpanElement>("skyEntryDot");
const skyBody = el<HTMLDivElement>("skyBody");

const dvyEntry = el<HTMLButtonElement>("dvyEntry");
const dvyEntryNote = el<HTMLSpanElement>("dvyEntryNote");
const dvyEntryDot = el<HTMLSpanElement>("dvyEntryDot");
const dvyBody = el<HTMLDivElement>("dvyBody");
const dvyBindToggle = el<HTMLInputElement>("dvyBindEnabled");
const dvyBindWrap = el<HTMLDivElement>("dvyBindWrap");
const dvyBindBtn = el<HTMLButtonElement>("btnDvyBind");
const dvyBindLabel = el<HTMLSpanElement>("dvyBindLabel");
const dvyHoldKeyBtn = el<HTMLButtonElement>("btnDvyHoldKey");
const dvyHoldLabel = el<HTMLSpanElement>("dvyHoldLabel");
const dvyPickSlot = el<HTMLInputElement>("dvyPickSlot");
const dvyHoldMs = el<HTMLInputElement>("dvyHoldMs");
const dvyKeyHold = el<HTMLInputElement>("dvyKeyHold");
const dvyBurstMs = el<HTMLInputElement>("dvyBurstMs");
const dvyBurstCps = el<HTMLInputElement>("dvyBurstCps");
const dvyBurstDuty = el<HTMLInputElement>("dvyBurstDuty");
const dvyRuns = el<HTMLElement>("dvyRuns");
const dvyRunBtn = el<HTMLButtonElement>("btnDvyRun");

const bowEntry = el<HTMLButtonElement>("bowEntry");
const bowEntryNote = el<HTMLSpanElement>("bowEntryNote");
const bowEntryDot = el<HTMLSpanElement>("bowEntryDot");
const bowBody = el<HTMLDivElement>("bowBody");
const bowBindToggle = el<HTMLInputElement>("bowBindEnabled");
const bowBindWrap = el<HTMLDivElement>("bowBindWrap");
const bowBindBtn = el<HTMLButtonElement>("btnBowBind");
const bowBindLabel = el<HTMLSpanElement>("bowBindLabel");
const bowSummary = el<HTMLParagraphElement>("bowSummary");
const bowTactical = el<HTMLInputElement>("bowTactical");
const bowTacticalWrap = el<HTMLDivElement>("bowTacticalWrap");
const bowTacticalSlot = el<HTMLInputElement>("bowTacticalSlot");
const bowSecondSwitch = el<HTMLInputElement>("bowSecondSwitch");
const bowSlot = el<HTMLInputElement>("bowSlot");
const bowSwordSlot = el<HTMLInputElement>("bowSwordSlot");
const bowKeyHold = el<HTMLInputElement>("bowKeyHold");
const bowAfterSwitch = el<HTMLInputElement>("bowAfterSwitch");
const bowClickHold = el<HTMLInputElement>("bowClickHold");
const bowAfterClick = el<HTMLInputElement>("bowAfterClick");
const bowRuns = el<HTMLElement>("bowRuns");
const bowRunBtn = el<HTMLButtonElement>("btnBowRun");

const overlayToggle = el<HTMLInputElement>("overlayEnabled");
const overlayWrap = el<HTMLDivElement>("overlayWrap");
const overlaySpots = el<HTMLDivElement>("overlaySpots");
const overlayXY = el<HTMLDivElement>("overlayXY");
const overlayX = el<HTMLInputElement>("overlayX");
const overlayY = el<HTMLInputElement>("overlayY");
const overlayOnlyIn = el<HTMLInputElement>("overlayOnlyIn");
const overlayNamesWrap = el<HTMLDivElement>("overlayNamesWrap");
const overlayNames = el<HTMLInputElement>("overlayNames");

const copyAllBtn = el<HTMLButtonElement>("btnCopyAll");
const copySettingsBtn = el<HTMLButtonElement>("btnCopySettings");
const copyClickerBtn = el<HTMLButtonElement>("btnCopyClicker");
const shareBox = el<HTMLInputElement>("shareBox");
const shareNote = el<HTMLElement>("shareNote");
const importCodeBtn = el<HTMLButtonElement>("btnImportCode");
const presetList = el<HTMLDivElement>("presetList");
const presetName = el<HTMLInputElement>("presetName");
const savePresetBtn = el<HTMLButtonElement>("btnSavePreset");

const skyBindToggle = el<HTMLInputElement>("skyBindEnabled");
const skyBindWrap = el<HTMLDivElement>("skyBindWrap");
const skyBindBtn = el<HTMLButtonElement>("btnSkyBind");
const skyBindLabel = el<HTMLSpanElement>("skyBindLabel");
const skyRestore = el<HTMLInputElement>("skyRestore");
const skySettle = el<HTMLInputElement>("skySettle");
const skyClickHold = el<HTMLInputElement>("skyClickHold");
const skyBetween = el<HTMLInputElement>("skyBetween");
const skyClicks = el<HTMLInputElement>("skyClicks");
const skyRetryGap = el<HTMLInputElement>("skyRetryGap");
const skyNote = el<HTMLParagraphElement>("skyNote");
const skyRunBtn = el<HTMLButtonElement>("btnSkyRun");

const autoHero = el<HTMLDivElement>("autoHero");
const autoStatus = el<HTMLSpanElement>("autoStatus");
const autoSub = el<HTMLSpanElement>("autoSub");
const autoRunBtn = el<HTMLButtonElement>("btnAutoRun");
const recordBtn = el<HTMLButtonElement>("btnRecord");
const recordMoves = el<HTMLInputElement>("recordMoves");
const recordHint = el<HTMLParagraphElement>("recordHint");
const autoRunLabel = el<HTMLSpanElement>("autoRunLabel");
const autoBindToggle = el<HTMLInputElement>("autoBindEnabled");
const autoBindWrap = el<HTMLDivElement>("autoBindWrap");
const autoBindHint = el<HTMLParagraphElement>("autoBindHint");
const autoBindBtn = el<HTMLButtonElement>("btnAutoBind");
const autoBindLabel = el<HTMLSpanElement>("autoBindLabel");
const autoRepeat = el<HTMLInputElement>("autoRepeat");
const autoLoopBtn = el<HTMLButtonElement>("btnAutoLoop");
const autoDelay = el<HTMLInputElement>("autoDelay");
const autoDelayValue = el<HTMLInputElement>("autoDelayInput");
const stepList = el<HTMLDivElement>("stepList");
const autoClearBtn = el<HTMLButtonElement>("btnAutoClear");

const updateBar = el<HTMLDivElement>("updateBar");
const updateText = el<HTMLSpanElement>("updateText");
const updateNotes = el<HTMLSpanElement>("updateNotes");
const updateActions = el<HTMLDivElement>("updateActions");
const updateTrack = el<HTMLDivElement>("updateTrack");
const updateFill = el<HTMLSpanElement>("updateFill");
const updateNowBtn = el<HTMLButtonElement>("btnUpdateNow");
const updateLaterBtn = el<HTMLButtonElement>("btnUpdateLater");
const appVersion = el<HTMLSpanElement>("appVersion");

const suggestKindGroup = el<HTMLDivElement>("suggestKindGroup");
const suggestTitle = el<HTMLInputElement>("suggestTitle");
const suggestBody = el<HTMLTextAreaElement>("suggestBody");
const suggestCount = el<HTMLSpanElement>("suggestCount");
const suggestHint = el<HTMLParagraphElement>("suggestHint");
const suggestVersion = el<HTMLSpanElement>("suggestVersion");
const suggestSendBtn = el<HTMLButtonElement>("btnSuggestSend");
const suggestCopyBtn = el<HTMLButtonElement>("btnSuggestCopy");
const suggestBrowseBtn = el<HTMLButtonElement>("btnSuggestBrowse");
const updateStatus = el<HTMLParagraphElement>("updateStatus");
const checkUpdateBtn = el<HTMLButtonElement>("btnCheckUpdate");

const profileTabs = el<HTMLDivElement>("profileTabs");
const profileName = el<HTMLInputElement>("profileName");
const addProfileBtn = el<HTMLButtonElement>("btnAddProfile");
const deleteProfileBtn = el<HTMLButtonElement>("btnDeleteProfile");
const profileEnabled = el<HTMLInputElement>("profileEnabled");
const profileEnabledHint = el<HTMLParagraphElement>("profileEnabledHint");

const cursorStyleGroup = el<HTMLDivElement>("cursorStyleGroup");
const cursorSizeWrap = el<HTMLDivElement>("cursorSizeWrap");
const cursorSizeSlider = el<HTMLInputElement>("cursorSize");
const cursorSizeValue = el<HTMLInputElement>("cursorSizeInput");
const cursorHint = el<HTMLParagraphElement>("cursorHint");
const cursorUploadWrap = el<HTMLDivElement>("cursorUploadWrap");
const cursorPreview = el<HTMLDivElement>("cursorPreview");
const cursorFile = el<HTMLInputElement>("cursorFile");
const cursorPickBtn = el<HTMLButtonElement>("btnCursorPick");
const cursorClearBtn = el<HTMLButtonElement>("btnCursorClear");
const cursorUploadHint = el<HTMLParagraphElement>("cursorUploadHint");

const tweakList = el<HTMLDivElement>("tweakList");
const adminList = el<HTMLDivElement>("adminList");
const cleanupList = el<HTMLDivElement>("cleanupList");
const optimizeAllBtn = el<HTMLButtonElement>("btnOptimizeAll");
const powerPlanGroup = el<HTMLDivElement>("powerPlanGroup");
const powerPlanHint = el<HTMLParagraphElement>("powerPlanHint");

const statePill = el<HTMLSpanElement>("statePill");
const statCps = el<HTMLElement>("statCps");
const statCpsLabel = el<HTMLElement>("statCpsLabel");
const statClicks = el<HTMLElement>("statClicks");
const statBind = el<HTMLElement>("statBind");
const statDelivery = el<HTMLElement>("statDelivery");
const cpsWarn = el<HTMLDivElement>("cpsWarn");
const sideCps = el<HTMLSpanElement>("sideCps");
const sideCpsLabel = el<HTMLSpanElement>("sideCpsLabel");
const resetBtn = el<HTMLButtonElement>("btnReset");

function push(): void {
  window.clearTimeout(pushTimer);
  pushTimer = window.setTimeout(() => {

    void invoke<Settings>("apply_settings", { settings }).catch(() => {

    });
  }, 110);
}

function reveal(node: HTMLElement, show: boolean): void {
  node.classList.toggle("open", show);
}

function positionSegment(group: HTMLElement): void {
  const indicator = group.querySelector<HTMLSpanElement>(".seg-indicator");
  const active = group.querySelector<HTMLButtonElement>(".seg.active");

  if (!indicator || !active || active.offsetWidth === 0) return;

  indicator.style.width = `${active.offsetWidth}px`;
  indicator.style.transform = `translateX(${active.offsetLeft}px)`;
  indicator.classList.add("ready");
}

function positionAllSegments(): void {
  document.querySelectorAll<HTMLElement>(".segmented").forEach(positionSegment);
}

function setSegment(group: HTMLElement, value: string): void {
  group.querySelectorAll<HTMLButtonElement>(".seg").forEach((seg) => {
    seg.classList.toggle("active", seg.dataset.value === value);
  });
  positionSegment(group);
}

function renderCps(): void {
  const solo = !profile.randomize;
  const delay = delayMode();

  setSegment(rateModeGroup, profile.rateMode);

  if (delay) {
    cpsLabel.textContent = solo ? "Delay between clicks" : "Shortest delay";
    minLabel.textContent = "Longest delay";
  } else {
    cpsLabel.textContent = solo ? "Clicks per second" : "Maximum";
    minLabel.textContent = "Minimum";
  }
  reveal(cpsMinWrap, !solo);

  for (const input of [cpsMaxInput, cpsMinInput]) {
    input.min = delay ? String(MIN_DELAY_MS) : "0.01";
    input.max = delay ? String(MAX_DELAY_MS) : String(CPS_CEILING);
    input.step = delay ? "1" : "0.1";
  }

  cpsMaxInput.value = delay
    ? formatDelay(cpsToDelay(profile.cpsMax))
    : formatCps(profile.cpsMax);
  cpsMinInput.value = delay
    ? formatDelay(cpsToDelay(profile.cpsMin))
    : formatCps(profile.cpsMin);

  cpsMaxSlider.value = String(cpsToPosition(profile.cpsMax));
  cpsMinSlider.value = String(cpsToPosition(profile.cpsMin));
  paintRange(cpsMaxSlider);
  paintRange(cpsMinSlider);

  cpsChips.querySelectorAll<HTMLButtonElement>(".chip").forEach((chip) => {
    const cps = Number(chip.dataset.cps) || 1;
    chip.textContent = delay
      ? `${formatDelay(cpsToDelay(cps))} ms`
      : cps >= 1000
        ? `${cps / 1000}k`
        : String(cps);
  });

  reveal(cpsWarn, profile.delivery === "system" && profile.cpsMax > 1500);
}

function renderConflict(): void {

  reveal(
    conflictWarn,
    profile.delivery !== "window" &&
      !profile.sequenceEnabled &&
      profile.bindVk === BUTTON_VK[profile.button],
  );
}

function renderDuty(): void {
  const on = profile.dutyEnabled;
  const percent = profile.dutyCycle;

  dutyToggle.checked = on;
  reveal(dutyWrap, on);
  reveal(dutyWarn, on);

  dutyValue.value = trimNum(percent);
  dutySlider.value = String(percent);
  paintRange(dutySlider);

  if (!on) {
    dutyMs.textContent = "instant";
    return;
  }
  const periodMs = 1000 / Math.max(profile.cpsMax, 0.1);
  const holdMs = (periodMs * percent) / 100;
  dutyMs.textContent = holdMs >= 1 ? `${holdMs.toFixed(1)} ms` : `${(holdMs * 1000).toFixed(0)} µs`;
}

const DELIVERY_HINTS: Record<string, string> = {
  system:
    "Injects into the global input stream. Works everywhere, but every event queues " +
    "behind your real mouse — past a few thousand CPS that shows up as cursor lag and stutter.",
  window:
    "Posts messages straight to whichever window is under the cursor when you activate. " +
    "Your real mouse is untouched and nothing queues, so high rates stay smooth — but " +
    "games that read raw input will ignore it.",
};

let targetPoll: number | undefined;

function renderDelivery(): void {
  const direct = profile.delivery === "window";
  deliveryDirect.checked = direct;
  deliveryHint.textContent = DELIVERY_HINTS[profile.delivery] ?? "";
  renderTargetMode();
  reveal(targetRow, direct);
  statDelivery.textContent = direct ? "Window" : "System";
  renderCps();

  window.clearInterval(targetPoll);

  if (!direct || profile.targetMode !== "cursor") {
    reveal(rawInputWarn, false);
    reveal(targetRow, false);
    return;
  }

  const tick = () => {

    if (!drawingWorthwhile()) return;

    void invoke<TargetInfo>("peek_target")
      .then((info) => {
        const named = info.title.trim();
        text(targetName, named || "untitled window");
        targetName.classList.toggle("empty", !named);

        text(rawInputName, named || info.process || "This app");
        reveal(rawInputWarn, info.rawInput);
      })
      .catch(() => {

      });
  };
  tick();
  targetPoll = window.setInterval(tick, 600);
}

const TARGET_MODE_HINTS: Record<string, string> = {
  cursor:
    "Locks onto whatever the pointer is over the moment you start, then stays there.",
  focused:
    "Follows whichever window you're tabbed into, so clicks move with you.",
  pinned:
    "One chosen window, clicked whether or not it's in front — you can work elsewhere meanwhile.",
};

let knownWindows: WindowEntry[] = [];

function renderWindowList(): void {
  windowList.innerHTML = "";

  if (knownWindows.length === 0) {
    const empty = document.createElement("div");
    empty.className = "step-empty";
    empty.textContent = "No windows found — try Refresh list.";
    windowList.append(empty);
    return;
  }

  knownWindows.forEach((entry) => {
    const chosen =
      entry.title === profile.targetTitle && entry.process === profile.targetProcess;

    const item = document.createElement("button");
    item.type = "button";
    item.className = chosen ? "window-item active" : "window-item";

    const title = document.createElement("span");
    title.className = "window-item-title";
    title.textContent = entry.title;

    const proc = document.createElement("span");
    proc.className = "window-item-proc";
    proc.textContent = entry.process;

    item.append(title);
    if (entry.rawInput) {
      const flag = document.createElement("span");
      flag.className = "raw-flag";
      flag.textContent = "RAW";
      item.append(flag);
    }
    item.append(proc);

    item.addEventListener("click", () => {
      profile.targetTitle = entry.title;
      profile.targetProcess = entry.process;

      profile.targetX = -1;
      profile.targetY = -1;
      renderTargetMode();
      push();
    });

    windowList.append(item);
  });
}

function refreshWindows(): void {
  void invoke<WindowEntry[]>("list_windows")
    .then((found) => {
      knownWindows = found;
      renderWindowList();
    })
    .catch(() => {
      knownWindows = [];
      renderWindowList();
    });
}

function renderTargetMode(): void {
  const direct = profile.delivery === "window";
  reveal(targetModeWrap, direct);
  if (!direct) {
    reveal(pinnedWrap, false);
    return;
  }

  setSegment(targetModeGroup, profile.targetMode);
  targetModeHint.textContent = TARGET_MODE_HINTS[profile.targetMode] ?? "";

  const pinned = profile.targetMode === "pinned";
  reveal(pinnedWrap, pinned);

  if (pinned) {

    const custom = profile.targetX >= 0 && profile.targetY >= 0;
    pinnedPointLabel.textContent = custom
      ? `Clicks at ${Math.round(profile.targetX)}, ${Math.round(profile.targetY)}`
      : "Clicks the centre";
    resetPointBtn.hidden = !custom;
    renderPoints();
    renderWindowList();
  }
}

function renderPoints(): void {
  pointList.replaceChildren();

  profile.points.forEach((point, index) => {
    const row = document.createElement("div");
    row.className = "point-row";

    const label = document.createElement("span");
    label.className = "target-label";
    label.textContent = `${index + 2}. ${Math.round(point.x)}, ${Math.round(point.y)}`;

    const remove = document.createElement("button");
    remove.type = "button";
    remove.className = "ghost-btn danger-btn";
    remove.textContent = "Remove";
    remove.addEventListener("click", () => {
      profile.points.splice(index, 1);
      renderTargetMode();
      push();
    });

    row.append(label, remove);
    pointList.append(row);
  });

  pointHint.textContent = profile.points.length
    ? `Clicks cycle through ${profile.points.length + 1} spots in order, one per click.`
    : "Every click lands on the one spot. Add more to work a row of buttons in turn.";
}

const PIXEL_STOP_HINTS: Record<string, string> = {
  change: "Stops as soon as the pixel stops matching the colour shown.",
  match: "Stops as soon as the pixel becomes the colour shown.",
};

function renderPixel(): void {
  pixelToggle.checked = profile.pixelEnabled;
  reveal(pixelWrap, profile.pixelEnabled);

  const rgb = profile.pixelRgb & 0xffffff;
  const hex = `#${rgb.toString(16).padStart(6, "0")}`;
  pixelSwatch.style.background = hex;
  pixelSwatch.textContent = hex.toUpperCase();

  setSegment(pixelStopGroup, profile.pixelStopOn);
  pixelTolerance.value = trimNum(profile.pixelTolerance);

  const picked = profile.pixelX !== 0 || profile.pixelY !== 0;
  pixelHint.textContent = picked
    ? `Watching ${Math.round(profile.pixelX)}, ${Math.round(profile.pixelY)}. ` +
      (PIXEL_STOP_HINTS[profile.pixelStopOn] ?? "")
    : "Pick a pixel first — nothing is being watched yet.";
}

function renderBurst(): void {
  burstToggle.checked = profile.burstEnabled;
  reveal(burstWrap, profile.burstEnabled);
  burstCount.value = trimNum(profile.burstCount);
  burstPause.value = trimNum(profile.burstPauseMs);

  const rate = Math.max(profile.cpsMax, 0.01);
  const clicking = profile.burstCount / rate;
  const cycle = clicking + profile.burstPauseMs / 1000;
  burstHint.textContent =
    `${trimNum(profile.burstCount)} clicks every ${cycle.toFixed(2)}s — ` +
    `about ${(profile.burstCount / cycle).toFixed(1)} clicks a second on average.`;
}

function renderSequencePreview(): void {
  const text = profile.sequence;
  if (!text.trim()) {
    sequencePreview.innerHTML =
      '<span class="seq-empty">Empty — nothing will be sent while this is on.</span>';
    return;
  }
  void invoke<string[]>("describe_sequence", { text })
    .then((steps) => {
      if (steps.length === 0) {
        sequencePreview.innerHTML =
          '<span class="seq-empty">Nothing recognised — nothing will be sent.</span>';
        return;
      }
      sequencePreview.innerHTML = "";
      steps.forEach((label) => {
        const chip = document.createElement("span");
        const isMouse = /^(LMB|RMB|MMB|MB4|MB5)[↓↑]?$/.test(label);
        const isTiming = /(ms|s)$|^scroll/.test(label);
        chip.className = isMouse
          ? "seq-step mouse"
          : isTiming
            ? "seq-step timing"
            : "seq-step";
        chip.textContent = label;
        sequencePreview.appendChild(chip);
      });
    })
    .catch(() => {
      sequencePreview.innerHTML = "";
    });
}

function renderBind(): void {
  const on = profile.bindEnabled;
  bindToggle.checked = on;
  reveal(bindWrap, on);
  bindHint.textContent = on
    ? "Any key or mouse button. Esc cancels."
    : "No hotkey. Start and stop from the button above.";

  const label = on ? vkLabel(profile.bindVk) : "—";
  statBind.textContent = label;
  bindLabel.textContent = vkLabel(profile.bindVk);
  heroSub.textContent = on
    ? `Press ${vkLabel(profile.bindVk)} to start`
    : "Press Start to begin";
}

function renderModeHint(): void {
  const hold = profile.mode === "hold";
  modeHint.textContent = hold
    ? "Clicks only while the bind is physically held down."
    : "Tap the bind to start, tap again to stop.";

  powerBtn.disabled = profile.enabled === false || (hold && profile.bindEnabled);

  powerBtn.removeAttribute("title");
}

let pendingUpdate: Update | null = null;

function summariseNotes(body: string): string {
  const parts = body
    .split(/\r?\n/)
    .map((line) => line.replace(/^\s*[-*]\s+/, "").trim())
    .filter((line) => line.length > 0 && !line.startsWith("#"));

  const joined = parts.join(" · ").replace(/[`*_]/g, "");
  return joined.length > 220 ? `${joined.slice(0, 219)}…` : joined;
}

async function checkForUpdate(manual: boolean): Promise<void> {
  if (manual) updateStatus.textContent = "Checking…";

  try {
    const update = await check();
    if (!update) {
      updateStatus.textContent = manual
        ? "You're on the latest version."
        : "Checked at launch.";
      return;
    }

    pendingUpdate = update;
    updateText.innerHTML = `Version <b>${update.version}</b> is available — you're on ${update.currentVersion}`;
    updateStatus.textContent = `Version ${update.version} is available.`;

    const notes = summariseNotes(update.body ?? "");
    updateNotes.textContent = notes;
    updateNotes.hidden = notes.length === 0;

    reveal(updateBar, true);
  } catch (error) {
    updateStatus.textContent = manual ? `Couldn't check: ${error}` : "Checked at launch.";
  }
}

async function installUpdate(): Promise<void> {
  if (!pendingUpdate) return;

  updateNowBtn.disabled = true;
  updateLaterBtn.disabled = true;
  updateActions.style.opacity = "0.6";
  updateTrack.hidden = false;

  let total = 0;
  let received = 0;

  try {
    await pendingUpdate.downloadAndInstall((event) => {
      switch (event.event) {
        case "Started":
          total = event.data.contentLength ?? 0;
          updateText.textContent = "Downloading…";
          break;
        case "Progress": {
          received += event.data.chunkLength;

          if (total > 0) {
            const pct = Math.min(100, (received / total) * 100);
            updateFill.style.width = `${pct}%`;
            updateText.textContent = `Downloading… ${Math.round(pct)}%`;
          } else {
            updateText.textContent = `Downloading… ${(received / 1_048_576).toFixed(1)} MB`;
          }
          break;
        }
        case "Finished":
          updateFill.style.width = "100%";
          updateText.textContent = "Installing…";
          break;
      }
    });

    updateText.textContent = "Restarting…";
    await relaunch();
  } catch (error) {
    updateText.textContent = `Update failed: ${error}`;
    updateTrack.hidden = true;
    updateNowBtn.disabled = false;
    updateLaterBtn.disabled = false;
    updateActions.style.opacity = "";
  }
}

function wireUpdates(): void {
  updateNowBtn.addEventListener("click", () => void installUpdate());
  updateLaterBtn.addEventListener("click", () => reveal(updateBar, false));
  checkUpdateBtn.addEventListener("click", () => void checkForUpdate(true));
}

const DOT_CURSOR =
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAYAAAAf8/9hAAABw0lEQVR42m3TsU4UYRQF4G9nd2dnF3AFBaFQKXwB34BEX4DC1lgYY2HnAxjewcLERGNsfAcbGjF20tmYGDFqIaCA7MLsztjcn4zGSW7mT/6c899z7rktf3+tqKxxhjqqapzPAOnL0EYH3fhncTdFiUlUFaXTAHdRYIAZ9OO+xgl+4xgjnAauajfAfczjEi4jD8ICs5iLjqtG1anNHoZYwSruYa0hbxMbQZxIpqg6obuI168k8OD5W8qupbzt053riWwDixiHrEkWOgdYwH2s5S/fO55cUA6v+dpZUjx+Izp6FB3MhOwsOZ+HxrXhiy3Vbq3oXFQeTJxOe8Zzy1aevdOQVcTDrTTvdjCaPRwZzs6rp5VWr0e/T10bdbv/5uVsfMmQU7h6cmD8/YOsKNX1MZMjjnZk9X6ToAxMnQhOcIjNrYfrFvOx0ZdteXfEzrZBf2rv9o00jZnIQ9nMQfLiI5Z+vn61urp+S16NzQ/YvXszgZ9iBz8SSSvM6OM8lmOUD/6Tgyf4jG/Yj0SWaWFSEs9FHhbinEerh9gL4K8ERtVqmNmJRKZ96IWsaXg0auxBmZap9c9o2o2NTN7UQTJJ8U1g+AOtKZGFOXsisgAAAABJRU5ErkJggg==";

const CURSOR_HINTS: Record<string, string> = {
  image: "The dot, at its original 16px.",
  ring: "A hollow ring — keeps whatever is underneath visible.",
  cross: "Thin crosshair for lining up on small targets.",
  arrow: "A pointer in your accent colour.",
  custom: "Your own image, scaled to the size below.",
  system: "Windows decides, same as any other app.",
};

const MAX_CURSOR_PX = 128;

async function loadCursorImage(file: File): Promise<void> {
  const readAsDataUrl = () =>
    new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result));
      reader.onerror = () => reject(reader.error);
      reader.readAsDataURL(file);
    });

  try {
    const source = await readAsDataUrl();

    const image = new Image();
    image.src = source;
    await image.decode();

    const width = image.naturalWidth || MAX_CURSOR_PX;
    const height = image.naturalHeight || MAX_CURSOR_PX;
    const scale = Math.min(1, MAX_CURSOR_PX / Math.max(width, height));

    const canvas = document.createElement("canvas");
    canvas.width = Math.max(1, Math.round(width * scale));
    canvas.height = Math.max(1, Math.round(height * scale));

    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("no canvas context");
    ctx.drawImage(image, 0, 0, canvas.width, canvas.height);

    settings.cursorImage = canvas.toDataURL("image/png");
    settings.cursorStyle = "custom";
    cursorUploadHint.textContent = `Loaded ${file.name} — ${canvas.width}x${canvas.height}.`;
    renderCursor();
    push();
  } catch {
    cursorUploadHint.textContent =
      "Couldn't read that image. PNG, WebP, GIF and JPEG all work.";
  }
}

interface CursorArt {
  url: string;
  hotspotX: number;
  hotspotY: number;
  size: number;
}

function cursorArt(): CursorArt | null {
  const size = Math.round(settings.cursorSize);
  const hue = Math.round(settings.accentHue) % 360;
  const accent = `hsl(${hue} ${Math.round(90 * (settings.accentSat / 100))}% 58%)`;
  const outline = "rgba(0,0,0,.55)";
  const half = Math.round(size / 2);

  const svg = (body: string) =>
    `data:image/svg+xml,${encodeURIComponent(
      `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 32 32">${body}</svg>`,
    )}`;

  switch (settings.cursorStyle) {
    case "system":
      return null;

    case "custom":

      if (!settings.cursorImage) return null;
      return {
        url: settings.cursorImage,
        hotspotX: half,
        hotspotY: half,
        size,
      };

    case "ring":
      return {
        url: svg(
          `<circle cx="16" cy="16" r="9" fill="none" stroke="${outline}" stroke-width="5"/>` +
            `<circle cx="16" cy="16" r="9" fill="none" stroke="${accent}" stroke-width="3"/>` +
            `<circle cx="16" cy="16" r="1.6" fill="${accent}"/>`,
        ),
        hotspotX: half,
        hotspotY: half,
        size,
      };

    case "cross":
      return {
        url: svg(
          `<path d="M16 3v10M16 19v10M3 16h10M19 16h10" stroke="${outline}" stroke-width="5" stroke-linecap="round"/>` +
            `<path d="M16 3v10M16 19v10M3 16h10M19 16h10" stroke="${accent}" stroke-width="2.4" stroke-linecap="round"/>`,
        ),
        hotspotX: half,
        hotspotY: half,
        size,
      };

    case "arrow":
      return {
        url: svg(
          `<path d="M5 2 26 15 16 16.6 11.6 26Z" fill="${outline}" stroke="${outline}" stroke-width="3" stroke-linejoin="round"/>` +
            `<path d="M5 2 26 15 16 16.6 11.6 26Z" fill="${accent}"/>`,
        ),

        hotspotX: Math.round((5 / 32) * size),
        hotspotY: Math.round((2 / 32) * size),
        size,
      };

    default:

      return { url: DOT_CURSOR, hotspotX: 8, hotspotY: 8, size: 16 };
  }
}

let nativeCursorTimer: number | undefined;

function pushNativeCursor(art: CursorArt | null): void {
  window.clearTimeout(nativeCursorTimer);

  if (!art) {
    void invoke("clear_native_cursor").catch(() => {});
    return;
  }

  nativeCursorTimer = window.setTimeout(() => {
    const image = new Image();
    image.src = art.url;
    void image
      .decode()
      .then(() => {
        const canvas = document.createElement("canvas");
        canvas.width = art.size;
        canvas.height = art.size;
        const ctx = canvas.getContext("2d");
        if (!ctx) return;

        ctx.drawImage(image, 0, 0, art.size, art.size);
        const { data } = ctx.getImageData(0, 0, art.size, art.size);

        void invoke("set_native_cursor", {
          width: art.size,
          height: art.size,
          hotspotX: art.hotspotX,
          hotspotY: art.hotspotY,
          rgba: Array.from(data),
        }).catch(() => {});
      })
      .catch(() => {

      });
  }, 120);
}

function renderCursor(): void {
  const art = cursorArt();
  const value = art
    ? `url("${art.url}") ${art.hotspotX} ${art.hotspotY}, auto`
    : "auto";

  document.documentElement.style.setProperty("--cursor-dot", value);
  pushNativeCursor(art);

  setSegment(cursorStyleGroup, settings.cursorStyle);
  cursorHint.textContent = CURSOR_HINTS[settings.cursorStyle] ?? "";

  const custom = settings.cursorStyle === "custom";
  reveal(cursorUploadWrap, custom);
  cursorPreview.style.backgroundImage = settings.cursorImage
    ? `url("${settings.cursorImage}")`
    : "none";
  cursorPreview.classList.toggle("empty", !settings.cursorImage);
  cursorClearBtn.hidden = !settings.cursorImage;

  const sizeable = !["image", "system"].includes(settings.cursorStyle);
  reveal(cursorSizeWrap, sizeable);
  const size = Math.round(settings.cursorSize);
  cursorSizeSlider.value = String(size);
  cursorSizeValue.value = String(size);
  paintRange(cursorSizeSlider);
}

let deleteArmed = false;
let deleteTimer: number | undefined;

function armDelete(): void {
  deleteArmed = true;
  deleteProfileBtn.textContent = `Delete "${profile.name}"?`;
  deleteProfileBtn.classList.add("confirming");
  window.clearTimeout(deleteTimer);
  deleteTimer = window.setTimeout(disarmDelete, 4000);
}

function disarmDelete(): void {
  deleteArmed = false;
  deleteProfileBtn.textContent = "Delete";
  deleteProfileBtn.classList.remove("confirming");
  window.clearTimeout(deleteTimer);
}

function selectProfile(index: number): void {

  disarmDelete();

  resetGraph();
  settings.selected = Math.min(Math.max(index, 0), settings.profiles.length - 1);
  profile = settings.profiles[settings.selected]!;
  renderAll();
  requestAnimationFrame(positionAllSegments);
}

function renderProfileTabs(running: boolean[] = []): void {
  profileTabs.innerHTML = "";

  settings.profiles.forEach((entry, index) => {
    const tab = document.createElement("button");
    tab.type = "button";
    const classes = ["profile-tab"];
    if (index === settings.selected) classes.push("active");
    if (entry.enabled === false) classes.push("off");
    tab.className = classes.join(" ");

    const dot = document.createElement("span");
    dot.className = running[index] ? "dot live" : "dot";

    const label = document.createElement("span");
    label.textContent = entry.name;

    tab.append(dot, label);
    tab.addEventListener("click", () => {
      if (index === settings.selected) return;
      selectProfile(index);
      push();
    });
    profileTabs.append(tab);
  });

  profileName.value = profile.name;
  profileEnabled.checked = profile.enabled !== false;
  profileEnabledHint.hidden = profile.enabled !== false;

  deleteProfileBtn.disabled = settings.profiles.length <= 1;
}

const THEME_HINTS: Record<string, string> = {
  gradient:
    "Ambient colour drawn from your accent, light or dark to match Windows.",
  "gradient-dark":
    "Ambient colour drawn from your accent, always dark whatever Windows is set to.",
  "gradient-light":
    "Ambient colour drawn from your accent, always light whatever Windows is set to.",
  dark: "Flat dark. No gradient, no ambient colour.",
  light: "Flat light. No gradient, no ambient colour.",
};

const systemPrefersLight = window.matchMedia("(prefers-color-scheme: light)");

// Light or dark, whoever gets to decide it.
function resolvedThemeIsLight(): boolean {
  if (settings.theme === "light" || settings.theme === "gradient-light") {
    return true;
  }
  if (settings.theme === "dark" || settings.theme === "gradient-dark") {
    return false;
  }
  // plain "gradient" is the only one that hands the choice to Windows
  return systemPrefersLight.matches;
}

function renderAccent(): void {
  const hue = Math.round(settings.accentHue) % 360;
  document.documentElement.style.setProperty("--acc-h", String(hue));
  accentSlider.value = String(hue);
  accentValue.value = String(hue);

  const sat = Math.round(settings.accentSat);
  document.documentElement.style.setProperty("--sat", (sat / 100).toFixed(3));
  accentSatSlider.value = String(sat);
  accentSatValue.value = String(sat);
  paintRange(accentSatSlider);

  renderCursor();

  swatchRow.querySelectorAll<HTMLButtonElement>(".swatch").forEach((swatch) => {
    swatch.classList.toggle("active", Number(swatch.dataset.hue) === hue);
  });
}

function renderTheme(): void {
  setSegment(themeGroup, settings.theme);
  themeHint.textContent = THEME_HINTS[settings.theme] ?? "";

  const light = resolvedThemeIsLight();
  document.documentElement.dataset.theme = light ? "light" : "dark";

  document.documentElement.dataset.surface = settings.theme.startsWith(
    "gradient",
  )
    ? "gradient"
    : "flat";

  void invoke("set_theme_tint", { light }).catch(() => {

  });
}

systemPrefersLight.addEventListener("change", () => {
  if (settings?.theme === "gradient") renderTheme();
});

let sizeSaveTimer = 0;

function rememberWindowSize(): void {
  windowSizeHint.textContent = `${window.innerWidth} x ${window.innerHeight}`;

  window.clearTimeout(sizeSaveTimer);
  sizeSaveTimer = window.setTimeout(() => {
    settings.windowWidth = window.innerWidth;
    settings.windowHeight = window.innerHeight;
    push();
  }, 400);
}

function renderAppearance(): void {
  document.documentElement.style.setProperty(
    "--panel-alpha",
    settings.opacity.toFixed(2),
  );
  opacityValue.textContent = String(Math.round(settings.opacity * 100));
  opacitySlider.value = String(Math.round(settings.opacity * 100));
  paintRange(opacitySlider);
}

function renderTimeLimit(): void {
  timeLimitToggle.checked = profile.timeLimitEnabled;
  reveal(timeLimitWrap, profile.timeLimitEnabled);
  timeLimitInput.value = trimNum(profile.timeLimitSecs);

  const spelled =
    profile.timeLimitSecs >= 60 ? `That's ${formatDuration(profile.timeLimitSecs)}. ` : "";
  timeLimitHint.textContent =
    `${spelled}Counted from the first click, so a start delay doesn't eat into it.`;
}

function renderAll(): void {
  setSegment(modeGroup, profile.mode);
  setSegment(buttonGroup, profile.button);
  setSegment(precisionGroup, profile.precision);

  panicLabel.textContent = vkLabel(settings.panicVk);
  renderBind();

  randomizeToggle.checked = profile.randomize;

  jitterSlider.value = String(profile.jitter);
  jitterValue.value = trimNum(profile.jitter);
  paintRange(jitterSlider);

  shakeToggle.checked = profile.shakeEnabled;
  shakeWrap.classList.toggle("open", profile.shakeEnabled);
  shakePx.value = trimNum(profile.shakePx);
  shakeMs.value = trimNum(profile.shakeMs);

  sequenceToggle.checked = profile.sequenceEnabled;
  reveal(sequenceWrap, profile.sequenceEnabled);
  sequenceInput.value = profile.sequence;

  buttonHint.hidden = !profile.sequenceEnabled;
  buttonGroup.style.opacity = profile.sequenceEnabled ? "0.45" : "";

  precisionHint.textContent =
    profile.precision === "max"
      ? "Spin-driven. Sub-millisecond accuracy, but pegs a full core while active — expect fan noise, and possibly coil whine or audio stutter."
      : "Sleep-driven. Barely touches the CPU. Batch sizing corrects for any timing slop, so the average rate is just as accurate.";

  limitToggle.checked = profile.limitEnabled;
  reveal(limitWrap, profile.limitEnabled);
  limitInput.value = String(profile.limitCount);

  renderTimeLimit();
  renderBurst();
  renderPixel();

  startDelayToggle.checked = profile.startDelayEnabled;
  reveal(startDelayWrap, profile.startDelayEnabled);
  startDelayInput.value = trimNum(profile.startDelayMs);

  filterToggle.checked = profile.filterEnabled;
  reveal(filterWrap, profile.filterEnabled);
  filterInput.value = profile.filterTitle;

  edgeGuardToggle.checked = settings.edgeGuardEnabled;
  reveal(edgeGuardWrap, settings.edgeGuardEnabled);
  edgeGuardSlider.value = String(settings.edgeGuardPx);
  edgeGuardValue.value = trimNum(settings.edgeGuardPx);
  paintRange(edgeGuardSlider);
  setSegment(edgeGuardMode, settings.edgeGuardMode);
  edgeGuardChrome.checked = settings.edgeGuardChrome;
  edgeGuardHint.textContent =
    settings.edgeGuardMode === "screen"
      ? "Only the outer bounds of the desktop, across all monitors. Stops far less often."
      : "Every visible window's border, which means it trips often — try 4–6px.";

  blurToggle.checked = settings.blurEnabled;
  acrylicToggle.checked = settings.acrylic;

  pinBtn.classList.toggle("live", settings.alwaysOnTop);
  pinBtn.setAttribute("aria-pressed", String(settings.alwaysOnTop));

  renderProfileTabs();
  renderCps();
  renderDuty();
  renderDelivery();
  renderConflict();
  renderModeHint();
  renderAccent();
  renderTheme();
  renderCursor();
  renderAppearance();
  renderSequencePreview();
}

let clickersRunning = 0;
let macroRunning = false;

function syncTitleDot(): void {
  titleDot.classList.toggle("live", clickersRunning > 0 || macroRunning);
}

const GRAPH_SAMPLES = 120;
const rateHistory: number[] = [];

function resetGraph(): void {
  rateHistory.length = 0;
}

function pushRateSample(cps: number): void {
  rateHistory.push(cps);
  if (rateHistory.length > GRAPH_SAMPLES) rateHistory.shift();
}

function drawGraph(): void {

  const ratio = window.devicePixelRatio || 1;
  const width = cpsGraph.clientWidth;
  const height = cpsGraph.clientHeight;
  if (width === 0 || height === 0) return;

  if (cpsGraph.width !== Math.round(width * ratio)) {
    cpsGraph.width = Math.round(width * ratio);
    cpsGraph.height = Math.round(height * ratio);
  }

  const ctx = cpsGraph.getContext("2d");
  if (!ctx) return;

  ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
  ctx.clearRect(0, 0, width, height);

  const peak = Math.max(...rateHistory, 1);
  const pad = 4;
  const usable = height - pad * 2;
  const step = width / Math.max(GRAPH_SAMPLES - 1, 1);

  const offset = width - (rateHistory.length - 1) * step;

  const styles = getComputedStyle(document.documentElement);
  const hue = styles.getPropertyValue("--acc-h").trim() || "222";
  const sat = Number(styles.getPropertyValue("--sat").trim()) || 1;

  const point = (index: number): [number, number] => [
    offset + index * step,
    height - pad - (rateHistory[index]! / peak) * usable,
  ];

  if (rateHistory.length > 1) {
    ctx.beginPath();
    ctx.moveTo(...point(0));
    for (let i = 1; i < rateHistory.length; i += 1) ctx.lineTo(...point(i));

    const area = ctx.createLinearGradient(0, 0, 0, height);
    area.addColorStop(0, `hsl(${hue} ${100 * sat}% 62% / 0.34)`);
    area.addColorStop(1, `hsl(${hue} ${100 * sat}% 62% / 0)`);

    ctx.lineTo(offset + (rateHistory.length - 1) * step, height);
    ctx.lineTo(offset, height);
    ctx.closePath();
    ctx.fillStyle = area;
    ctx.fill();

    ctx.beginPath();
    ctx.moveTo(...point(0));
    for (let i = 1; i < rateHistory.length; i += 1) ctx.lineTo(...point(i));
    ctx.strokeStyle = `hsl(${hue} ${100 * sat}% 70%)`;
    ctx.lineWidth = 1.6;
    ctx.lineJoin = "round";
    ctx.stroke();
  }

  ctx.fillStyle = styles.getPropertyValue("--text-faint").trim() || "#6d80a8";
  ctx.font = "10px Inter, Segoe UI, system-ui, sans-serif";
  ctx.textBaseline = "top";
  const label = peak >= 100 ? Math.round(peak).toString() : peak.toFixed(1);
  ctx.fillText(`peak ${label}`, 6, 4);
}

function renderActive(active: boolean): void {

  if (active) clickersRunning = Math.max(clickersRunning, 1);
  syncTitleDot();

  heroCard.classList.toggle("live", active);
  powerBtn.classList.toggle("live", active);
  statePill.classList.toggle("live", active);

  heroStatus.textContent = active ? "Clicking" : "Idle";
  statePill.textContent = active ? "ACTIVE" : "Idle";
  powerLabel.textContent = active ? "Stop" : "Start";
  if (!profile.bindEnabled) {
    heroSub.textContent = active ? "Press Stop to end" : "Press Start to begin";
  } else {
    heroSub.textContent = active
      ? `Press ${vkLabel(profile.bindVk)} to stop`
      : `Press ${vkLabel(profile.bindVk)} to start`;
  }
}

function text(node: HTMLElement, value: string): void {
  if (node.textContent !== value) node.textContent = value;
}

let graphWasActive = false;

/// The clicker that ran most recently. The readout keeps showing its
/// numbers after it stops, instead of dropping back to a profile that
/// never ran and reading zero.
let lastRun: number | null = null;

let windowFocused = true;

function drawingWorthwhile(): boolean {
  return windowFocused && !document.hidden;
}

function setDormant(): void {
  document.body.classList.toggle("dormant", !drawingWorthwhile());
}

function clickerPanelVisible(): boolean {
  return document.querySelector('.panel[data-panel="clicker"]')?.classList.contains("active") ?? false;
}

function renderStatus(status: Status): void {

  const running = status.clickers.map((c) => c.active);
  const dots = profileTabs.querySelectorAll<HTMLSpanElement>(".dot");
  dots.forEach((dot, index) => dot.classList.toggle("live", running[index] === true));

  clickersRunning = status.running;
  syncTitleDot();

  text(statTotalClicks, formatCount(status.totalClicks));
  text(statActiveTime, formatDuration(status.activeSeconds));
  text(statCpu, `${status.cpuPercent.toFixed(1)}%`);

  const mine = status.clickers[settings.selected];
  if (!mine) return;

  const live = mine.active
    ? undefined
    : status.clickers.find((c) => c.active && c.guarded === false) ??
      status.clickers.find((c) => c.active);

  if (mine.active) {
    lastRun = settings.selected;
  } else if (live) {
    lastRun = status.clickers.indexOf(live);
  }

  // nothing is running: fall back to whichever clicker went last
  const stale =
    !mine.active && !live && lastRun !== null && lastRun !== settings.selected
      ? status.clickers[lastRun]
      : undefined;

  const other = live ?? stale;
  const shown = other ?? mine;

  const cps = shown.cps >= 100 ? formatCount(shown.cps) : shown.cps.toFixed(1);
  text(statCps, cps);
  text(sideCps, cps);
  text(statClicks, formatCount(shown.clicks));

  if (shown.active && !graphWasActive) resetGraph();
  graphWasActive = shown.active;

  pushRateSample(shown.cps);

  if (drawingWorthwhile() && clickerPanelVisible()) drawGraph();

  const borrowed = other ? other.name.trim() || "Clicker" : "";
  text(statCpsLabel, borrowed || "Live");
  text(sideCpsLabel, borrowed ? `${borrowed} CPS` : "Live CPS");
  statCps.classList.toggle("borrowed", Boolean(other));

  renderActive(mine.active);

  if (mine.active) {
    heroCard.classList.toggle("guarded", mine.guarded);
    text(heroStatus, mine.guarded ? "Held" : "Clicking");
    text(statePill, mine.guarded ? "GUARDED" : "ACTIVE");
    statePill.classList.toggle("live", !mine.guarded);
    if (mine.guarded) {
      text(heroSub, "Cursor is near a window edge or button");
    }
  } else {
    heroCard.classList.remove("guarded");

    if (status.running > 0) {
      text(
        statePill,
        status.running === 1 && borrowed
          ? `${borrowed.toUpperCase()} RUNNING`
          : `${status.running} RUNNING`,
      );
      statePill.classList.add("live");
      text(
        heroSub,
        status.running === 1 && borrowed
          ? `${borrowed} is clicking — switch tabs to control it`
          : `${status.running} other clickers are running`,
      );
    }
  }

  if (mine.active && profile.delivery === "window") {
    const locked = mine.target.trim();
    targetName.textContent = locked || "untitled window";
    targetName.classList.toggle("empty", !locked);
  }
}

const REPO_URL = "https://github.com/Boots3453/Syntax";

const SUGGEST_KINDS: Record<string, { label: string; prefix: string }> = {
  feature: { label: "Feature", prefix: "Feature" },
  improvement: { label: "Improvement", prefix: "Improvement" },
  bug: { label: "Bug", prefix: "Bug" },
};

let suggestKind = "feature";

function suggestionText(): string {
  const kind = SUGGEST_KINDS[suggestKind]?.label ?? "Suggestion";
  const version = appVersion.textContent?.trim() || "unknown";

  return [
    suggestBody.value.trim(),
    "",
    "---",
    `Type: ${kind}`,
    `Version: ${version}`,
  ].join("\n");
}

function renderSuggest(): void {
  const length = suggestBody.value.length;
  suggestCount.textContent = String(length);

  const ready = suggestTitle.value.trim().length > 0;
  suggestSendBtn.classList.toggle("disabled", !ready);
  suggestCopyBtn.disabled = !ready;

  suggestVersion.textContent = appVersion.textContent ?? "—";
}

function suggestionUrl(): string {
  const kind = SUGGEST_KINDS[suggestKind]?.prefix ?? "Suggestion";
  const title = `[${kind}] ${suggestTitle.value.trim()}`;

  const params = new URLSearchParams({
    title: title.slice(0, 140),
    body: suggestionText().slice(0, 5000),
    labels: suggestKind === "bug" ? "bug" : "suggestion",
  });

  return `${REPO_URL}/issues/new?${params.toString()}`;
}

async function copySuggestion(): Promise<boolean> {
  const text = `${suggestTitle.value.trim()}\n\n${suggestionText()}`;

  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {

    const scratch = document.createElement("textarea");
    scratch.value = text;
    scratch.style.position = "fixed";
    scratch.style.opacity = "0";
    document.body.append(scratch);
    scratch.select();
    const ok = document.execCommand("copy");
    scratch.remove();
    return ok;
  }
}

function wireSuggest(): void {
  suggestKindGroup.querySelectorAll<HTMLButtonElement>(".seg").forEach((seg) => {
    seg.addEventListener("click", () => {
      suggestKind = seg.dataset.value ?? "feature";
      setSegment(suggestKindGroup, suggestKind);
      renderSuggest();
    });
  });

  suggestTitle.addEventListener("input", renderSuggest);
  suggestBody.addEventListener("input", renderSuggest);

  suggestSendBtn.addEventListener("click", () => {
    if (suggestTitle.value.trim().length === 0) {
      suggestHint.textContent = "Add a one-line summary first.";
      suggestTitle.focus();
      return;
    }

    void invoke("open_repo_url", { url: suggestionUrl() })
      .then(() => {
        suggestHint.textContent =
          "Opened in your browser. Nothing is posted until you press Submit there.";
      })
      .catch(() => {
        suggestHint.textContent =
          "Couldn't open your browser. Use Copy instead and paste it into GitHub.";
      });
  });

  suggestCopyBtn.addEventListener("click", () => {
    void copySuggestion().then((ok) => {
      suggestHint.textContent = ok
        ? "Copied to your clipboard."
        : "Couldn't reach the clipboard — select the text and copy it manually.";
    });
  });

  suggestBrowseBtn.addEventListener("click", () => {
    void invoke("open_repo_url", { url: `${REPO_URL}/issues` }).catch(() => {});
  });
}

const POWER_PLAN_HINTS: Record<string, string> = {
  balanced:
    "Windows parks cores and drops clock speed when idle, so the first few milliseconds of a burst run slower than the rest.",
  high: "The CPU stays off its low-power states, which removes the ramp-up at the start of a burst. Costs a little battery.",
  other:
    "You're on a custom or manufacturer power plan. Picking one of these replaces it.",
  unknown: "Couldn't read the active power plan from Windows.",
};

let optimizations: Optimizations = {
  tweaks: [],
  powerPlan: "unknown",
  admin: [],
  cleanups: [],
};

function renderOptimizations(): void {
  tweakList.replaceChildren();

  for (const tweak of optimizations.tweaks) {
    const row = document.createElement("div");
    row.className = tweak.readable ? "tweak" : "tweak unreadable";

    const text = document.createElement("div");
    const name = document.createElement("span");
    name.className = "tweak-name";
    name.textContent = tweak.label;
    const detail = document.createElement("p");
    detail.className = "hint tight";
    detail.textContent = tweak.readable
      ? tweak.detail
      : "Windows wouldn't report this setting, so it can't be changed from here.";
    text.append(name, detail);

    const toggle = document.createElement("label");
    toggle.className = "switch";
    const input = document.createElement("input");
    input.type = "checkbox";
    input.checked = tweak.optimised;
    input.disabled = !tweak.readable;
    const track = document.createElement("span");
    track.className = "track";
    toggle.append(input, track);

    input.addEventListener("change", () => {
      void applyTweak(tweak.id, input.checked);
    });

    row.append(text, toggle);
    tweakList.append(row);
  }

  renderAdmin();
  renderCleanups();

  setSegment(powerPlanGroup, optimizations.powerPlan);
  powerPlanHint.textContent = POWER_PLAN_HINTS[optimizations.powerPlan] ?? "";

  const anyOff = optimizations.tweaks.some((t) => t.readable && !t.optimised);
  optimizeAllBtn.disabled = !anyOff;
  optimizeAllBtn.textContent = anyOff ? "Apply all" : "All applied";
}

function renderAdmin(): void {
  adminList.replaceChildren();

  for (const tweak of optimizations.admin) {
    const row = document.createElement("div");
    row.className = "tweak";

    const text = document.createElement("div");
    const name = document.createElement("span");
    name.className = "tweak-name";
    name.textContent = tweak.label;

    const detail = document.createElement("p");
    detail.className = "hint tight";
    detail.textContent = tweak.reboot
      ? `${tweak.detail} Takes effect after a restart.`
      : tweak.detail;

    text.append(name, detail);

    const run = (optimised: boolean, note: string) => {
      void invoke("set_admin_tweak", { id: tweak.id, optimised })
        .then(() => {
          detail.textContent = note;

          if (tweak.readable) window.setTimeout(() => void refreshOptimizations(), 1500);
        })
        .catch((error) => {
          detail.textContent = String(error);
        });
    };

    if (tweak.readable) {
      const toggle = document.createElement("label");
      toggle.className = "switch";
      const input = document.createElement("input");
      input.type = "checkbox";
      input.checked = tweak.optimised;
      const track = document.createElement("span");
      track.className = "track";
      toggle.append(input, track);

      input.addEventListener("change", () => {
        const wanted = input.checked;

        input.checked = tweak.optimised;
        run(wanted, "Applied. Re-reading…");
      });

      row.append(text, toggle);
    } else {

      const actions = document.createElement("div");
      actions.className = "chip-row";

      const apply = document.createElement("button");
      apply.type = "button";
      apply.className = "ghost-btn";
      apply.textContent = "Apply";
      apply.addEventListener("click", () => run(true, "Applied."));

      const undo = document.createElement("button");
      undo.type = "button";
      undo.className = "ghost-btn";
      undo.textContent = "Undo";
      undo.addEventListener("click", () => run(false, "Reverted."));

      actions.append(apply, undo);
      row.append(text, actions);
    }

    adminList.append(row);
  }
}

function renderCleanups(): void {
  cleanupList.replaceChildren();

  for (const job of optimizations.cleanups) {
    const row = document.createElement("div");
    row.className = "tweak";

    const text = document.createElement("div");
    const name = document.createElement("span");
    name.className = "tweak-name";
    name.textContent = job.label;
    const detail = document.createElement("p");
    detail.className = "hint tight";
    detail.textContent = job.detail;
    text.append(name, detail);

    const run = document.createElement("button");
    run.type = "button";
    run.className = job.destructive ? "ghost-btn danger-btn" : "ghost-btn";
    run.textContent = "Run";
    run.addEventListener("click", () => {
      void invoke("run_cleanup", { id: job.id })
        .then(() => {
          detail.textContent = "Started in an elevated window.";
        })
        .catch((error) => {
          detail.textContent = String(error);
        });
    });

    row.append(text, run);
    cleanupList.append(row);
  }
}

async function refreshOptimizations(): Promise<void> {
  try {
    optimizations = await invoke<Optimizations>("get_optimizations");
    renderOptimizations();
  } catch {

  }
}

async function applyTweak(id: string, optimised: boolean): Promise<void> {
  try {

    optimizations = await invoke<Optimizations>("set_optimization", { id, optimised });
  } catch {
    await refreshOptimizations();
    return;
  }
  renderOptimizations();
}

function wireOptimize(): void {
  optimizeAllBtn.addEventListener("click", async () => {
    optimizeAllBtn.disabled = true;
    for (const tweak of optimizations.tweaks) {
      if (tweak.readable && !tweak.optimised) {
        await applyTweak(tweak.id, true);
      }
    }
  });

  powerPlanGroup.querySelectorAll<HTMLButtonElement>(".seg").forEach((seg) => {
    seg.addEventListener("click", async () => {
      const plan = seg.dataset.value;
      if (!plan) return;
      try {
        optimizations = await invoke<Optimizations>("set_power_plan", { plan });
        renderOptimizations();
      } catch {
        await refreshOptimizations();
      }
    });
  });

  document.querySelectorAll<HTMLButtonElement>("[data-launch]").forEach((button) => {
    button.addEventListener("click", () => {
      void invoke("launch_tool", { target: button.dataset.launch }).catch(() => {});
    });
  });

  el<HTMLButtonElement>("btnWinutil").addEventListener("click", () => {
    void invoke("launch_tool", { target: "winutil" }).catch(() => {});
  });
}

const MOUSE_CYCLE = ["left", "right", "middle", "mouse4", "mouse5"];
const MOUSE_LABELS: Record<string, string> = {
  left: "Left",
  right: "Right",
  middle: "Middle",
  mouse4: "Mouse 4",
  mouse5: "Mouse 5",
};

let runningStep = -1;

function pushAutomation(): void {
  window.clearTimeout(autoPushTimer);
  autoPushTimer = window.setTimeout(() => {

    void invoke<Automation>("apply_automation", { automation }).catch(() => {

    });
  }, 140);
}

function newStep(kind: Step["kind"]): Step {
  switch (kind) {
    case "move": {
      return { kind: "move", x: 0, y: 0 };
    }
    case "click":
      return { kind: "click", button: "left", count: 1 };
    case "key":
      return { kind: "key", vk: 0x20 };
    case "text":
      return { kind: "text", value: "" };
    case "wait":
      return { kind: "wait", ms: 500 };
    case "scroll":
      return { kind: "scroll", amount: -3 };
  }
}

function numberField(
  value: number,
  onChange: (next: number) => void,
  width = "74px",
): HTMLInputElement {
  const input = noAutofill(document.createElement("input"));
  input.type = "number";

  input.step = "any";
  input.value = trimNum(value);
  input.style.width = width;
  input.addEventListener("change", () => {
    onChange(Number(input.value) || 0);
    pushAutomation();
  });
  return input;
}

function unitLabel(text: string): HTMLSpanElement {
  const span = document.createElement("span");
  span.className = "unit";
  span.textContent = text;
  return span;
}

function buildStepFields(step: Step, index: number): HTMLDivElement {
  const fields = document.createElement("div");
  fields.className = "step-fields";

  switch (step.kind) {
    case "move": {
      fields.append(
        unitLabel("x"),
        numberField(step.x, (next) => (step.x = next)),
        unitLabel("y"),
        numberField(step.y, (next) => (step.y = next)),
      );
      const pick = document.createElement("button");
      pick.className = "ghost-btn";
      pick.type = "button";
      const picking = capturing?.kind === "position" && capturing.index === index;
      pick.textContent = picking ? "Click anywhere…" : "Pick";
      pick.addEventListener("click", () => {
        if (capturing) {
          void invoke("cancel_capture");
          endCapture();
          return;
        }
        beginCapture({ kind: "position", index });
      });
      fields.append(pick);
      break;
    }

    case "click": {
      const button = document.createElement("button");
      button.className = "ghost-btn";
      button.type = "button";
      button.textContent = MOUSE_LABELS[step.button] ?? "Left";
      button.addEventListener("click", () => {
        const next = (MOUSE_CYCLE.indexOf(step.button) + 1) % MOUSE_CYCLE.length;
        step.button = MOUSE_CYCLE[next] ?? "left";
        button.textContent = MOUSE_LABELS[step.button] ?? "Left";
        pushAutomation();
      });
      fields.append(
        button,
        unitLabel("×"),
        numberField(step.count, (next) => (step.count = Math.max(1, next)), "62px"),
      );
      break;
    }

    case "key": {
      const cap = document.createElement("button");
      cap.className = "keybind";
      cap.type = "button";
      cap.style.padding = "3px 7px";
      const listening = capturing?.kind === "stepKey" && capturing.index === index;
      cap.innerHTML = `<span class="keycap">${
        listening ? "Press a key…" : vkLabel(step.vk)
      }</span>`;
      cap.addEventListener("click", () => {
        if (capturing) {
          void invoke("cancel_capture");
          endCapture();
          return;
        }
        beginCapture({ kind: "stepKey", index });
      });
      fields.append(cap);
      break;
    }

    case "text": {
      const input = noAutofill(document.createElement("input"));
      input.type = "text";
      input.value = step.value;
      input.placeholder = "text to type, or {ENTER}";
      input.addEventListener("input", () => {
        step.value = input.value;
        pushAutomation();
      });
      fields.append(input);
      break;
    }

    case "wait": {
      fields.append(
        numberField(step.ms, (next) => (step.ms = Math.max(0, next)), "88px"),
        unitLabel("ms"),

      );
      break;
    }

    case "scroll": {
      fields.append(
        numberField(step.amount, (next) => (step.amount = next), "70px"),
        unitLabel("notches — negative scrolls down"),
      );
      break;
    }
  }

  return fields;
}

function stepButton(glyph: string, title: string, onClick: () => void): HTMLButtonElement {
  const button = document.createElement("button");
  button.className = title === "Remove" ? "step-btn remove" : "step-btn";
  button.type = "button";
  button.textContent = glyph;
  button.setAttribute("aria-label", title);
  button.addEventListener("click", onClick);
  return button;
}

function renderSteps(): void {
  stepList.innerHTML = "";

  if (automation.steps.length === 0) {
    const empty = document.createElement("div");
    empty.className = "step-empty";
    empty.textContent = "No steps yet — add one below.";
    stepList.append(empty);
    return;
  }

  automation.steps.forEach((step, index) => {
    const row = document.createElement("div");
    row.className = index === runningStep ? "step-row running" : "step-row";

    const number = document.createElement("span");
    number.className = "step-index";
    number.textContent = String(index + 1);

    const kind = document.createElement("span");
    kind.className = "step-kind";
    kind.textContent = step.kind === "text" ? "type" : step.kind;

    const actions = document.createElement("div");
    actions.className = "step-actions";
    actions.append(
      stepButton("↑", "Move up", () => {
        if (index === 0) return;
        const [moved] = automation.steps.splice(index, 1);
        if (moved) automation.steps.splice(index - 1, 0, moved);
        renderSteps();
        pushAutomation();
      }),
      stepButton("↓", "Move down", () => {
        if (index >= automation.steps.length - 1) return;
        const [moved] = automation.steps.splice(index, 1);
        if (moved) automation.steps.splice(index + 1, 0, moved);
        renderSteps();
        pushAutomation();
      }),
      stepButton("✕", "Remove", () => {
        automation.steps.splice(index, 1);
        renderSteps();
        pushAutomation();
      }),
    );

    row.append(number, kind, buildStepFields(step, index), actions);
    stepList.append(row);
  });
}

function renderAutomation(): void {
  autoBindToggle.checked = automation.bindEnabled;
  reveal(autoBindWrap, automation.bindEnabled);
  autoBindLabel.textContent = vkLabel(automation.bindVk);
  autoBindHint.textContent = automation.bindEnabled
    ? "Separate from the clicker's bind — they never share a key."
    : "No hotkey. Run the macro from the button above.";

  const loops = automation.repeat === 0;
  autoRepeat.value = String(automation.repeat);
  autoRepeat.disabled = loops;
  autoLoopBtn.textContent = loops ? "Looping — click to limit" : "Loop forever";

  autoDelay.value = String(automation.stepDelayMs);
  autoDelayValue.value = trimNum(automation.stepDelayMs);
  paintRange(autoDelay);

  autoSub.textContent = automation.bindEnabled
    ? `Press ${vkLabel(automation.bindVk)} to run`
    : "Press Run to start";

  renderSteps();
}

function pushFisher(): void {
  window.clearTimeout(fisherPushTimer);
  fisherPushTimer = window.setTimeout(() => {
    void invoke<Fisher>("apply_fisher", { config: fisher }).catch(() => {});
  }, 140);
}

function hex(color: number): string {
  return `#${(color & 0xffffff).toString(16).padStart(6, "0")}`;
}

let fishGridBuilt = false;

function buildFishGrid(): void {
  fishGrid.textContent = "";

  FISH_KINDS.forEach((kind, index) => {
    const row = document.createElement("div");
    row.className = "fish-row";

    const swatch = document.createElement("span");
    swatch.className = "fish-swatch";
    swatch.id = `fishColor${index}`;

    const name = document.createElement("span");
    name.className = "fish-name";
    name.textContent = kind.label;

    const note = document.createElement("span");
    note.className = "fish-note";
    note.textContent = kind.note;

    const count = document.createElement("span");
    count.className = "fish-count";
    count.id = `fishCount${index}`;
    count.textContent = "0";

    const toggle = document.createElement("label");
    toggle.className = "switch";
    const box = noAutofill(document.createElement("input"));
    box.type = "checkbox";
    box.id = `fishOn${index}`;
    box.addEventListener("change", () => {
      fisher.types[index] = box.checked;
      pushFisher();
    });
    const track = document.createElement("span");
    track.className = "track";
    toggle.append(box, track);

    row.append(swatch, name, note, count, toggle);
    fishGrid.append(row);
  });

  fishGridBuilt = true;
}

function renderFisher(): void {
  if (!fishGridBuilt) buildFishGrid();

  FISH_KINDS.forEach((_, index) => {
    const swatch = document.getElementById(`fishColor${index}`);
    const box = document.getElementById(`fishOn${index}`) as HTMLInputElement | null;
    if (swatch) swatch.style.background = hex(fisher.colors[index] ?? 0);
    if (box) box.checked = fisher.types[index] !== false;
  });

  fisherBindToggle.checked = fisher.bindEnabled;
  fisherBindWrap.classList.toggle("open", fisher.bindEnabled);
  text(fisherBindLabel, vkLabel(fisher.bindVk));

  text(
    fisherSub,
    fisher.bindEnabled ? `Press ${vkLabel(fisher.bindVk)} to run` : "Press Run to start",
  );
}

function renderFisherStatus(status: FisherStatus): void {
  fisherHero.classList.toggle("live", status.running);
  fisherRunBtn.classList.toggle("live", status.running);
  fisherEntryDot.classList.toggle("live", status.running);

  const caught = status.caught.reduce((sum, n) => sum + n, 0);
  text(
    fisherEntryNote,
    status.running
      ? `${status.phase} — ${caught} caught`
      : caught > 0
        ? `Stopped — ${caught} caught`
        : "Stopped",
  );
  text(fisherStatusText, status.running ? status.phase : "Stopped");
  text(fisherRunLabel, status.running ? "Stop" : "Run");

  FISH_KINDS.forEach((_, index) => {
    const count = document.getElementById(`fishCount${index}`);
    if (count) text(count, String(status.caught[index] ?? 0));
  });

  const trace = (status.log ?? []).join("\n");
  text(fisherTrace, trace || "Nothing recorded yet.");

  if (status.running) {
    const extra = status.rejected > 0 ? ` — ${status.rejected} cancelled` : "";
    text(fisherSub, `${status.detail || "working"}${extra}`);
  } else {
    text(
      fisherSub,
      fisher.bindEnabled ? `Press ${vkLabel(fisher.bindVk)} to run` : "Press Run to start",
    );
  }
}

function wireFisher(): void {
  fisherEntry.addEventListener("click", () => {
    const open = !fisherBody.classList.contains("open");
    fisherBody.classList.toggle("open", open);
    fisherEntry.classList.toggle("open", open);
    fisherEntry.setAttribute("aria-expanded", String(open));
  });

  fisherRunBtn.addEventListener("click", () => {
    void invoke<boolean>("toggle_fisher");
  });

  fisherResetBtn.addEventListener("click", () => {
    void invoke("reset_fisher_counts");
  });

  fisherCopyBtn.addEventListener("click", () => {
    const text = fisherTrace.textContent ?? "";
    void navigator.clipboard.writeText(text).then(
      () => {
        fisherCopyBtn.textContent = "Copied";
        window.setTimeout(() => (fisherCopyBtn.textContent = "Copy"), 1400);
      },
      () => {
        fisherCopyBtn.textContent = "Select it";
      },
    );
  });

  fisherBindToggle.addEventListener("change", () => {
    fisher.bindEnabled = fisherBindToggle.checked;
    renderFisher();
  renderGumdrop();
  renderSkywars();
    pushFisher();
  });

  fisherBindBtn.addEventListener("click", () => {
    if (capturing) {
      void invoke("cancel_capture");
      endCapture();
      return;
    }
    beginCapture({ kind: "fisherBind" });
  });

}

function pushGumdrop(): void {
  window.clearTimeout(dropPushTimer);
  dropPushTimer = window.setTimeout(() => {
    void invoke<Gumdrop>("apply_gumdrop", { config: gumdrop }).catch(() => {});
  }, 140);
}

function renderGumdrop(): void {
  dropGumSlot.value = String(gumdrop.gumdropSlot);
  dropPickSlot.value = String(gumdrop.pickaxeSlot);
  dropSwordSlot.value = String(gumdrop.swordSlot);
  dropKeyHold.value = String(gumdrop.keyHoldMs);
  dropAfterGum.value = String(gumdrop.afterGumdropMs);
  dropAfterPick.value = String(gumdrop.afterPickaxeMs);
  dropAfterBreak.value = String(gumdrop.afterBreakMs);
  dropClickHold.value = String(gumdrop.clickHoldMs);
  dropWait.value = String(gumdrop.placeWaitMs);

  const total =
    gumdrop.keyHoldMs * 3 +
    gumdrop.clickHoldMs * 2 +
    gumdrop.afterGumdropMs +
    gumdrop.placeWaitMs +
    gumdrop.afterPickaxeMs +
    gumdrop.afterBreakMs;
  text(dropTotal, `One run takes about ${total} ms.`);

  dropBindToggle.checked = gumdrop.bindEnabled;
  dropBindWrap.classList.toggle("open", gumdrop.bindEnabled);
  text(dropBindLabel, vkLabel(gumdrop.bindVk));

  text(
    dropEntryNote,
    gumdrop.bindEnabled ? `Press ${vkLabel(gumdrop.bindVk)} to run once` : "Ready",
  );
}

function renderGumdropStatus(status: GumdropStatus): void {
  dropEntryDot.classList.toggle("live", status.busy);
  text(dropRuns, status.runs === 0 ? "Not run yet" : `Run ${status.runs} times`);
}

function wireGumdrop(): void {
  dropEntry.addEventListener("click", () => {
    const open = !dropBody.classList.contains("open");
    dropBody.classList.toggle("open", open);
    dropEntry.classList.toggle("open", open);
    dropEntry.setAttribute("aria-expanded", String(open));
  });

  dropRunBtn.addEventListener("click", () => {
    void invoke("fire_gumdrop");
  });

  dropBindToggle.addEventListener("change", () => {
    gumdrop.bindEnabled = dropBindToggle.checked;
    renderGumdrop();
  renderSkywars();
    pushGumdrop();
  });

  dropBindBtn.addEventListener("click", () => {
    if (capturing) {
      void invoke("cancel_capture");
      endCapture();
      return;
    }
    beginCapture({ kind: "dropBind" });
  });

  const slot = (input: HTMLInputElement, apply: (value: number) => void) => {
    input.addEventListener("change", () => {
      apply(Math.min(9, Math.max(1, Math.round(Number(input.value) || 1))));
      renderGumdrop();
  renderSkywars();
      pushGumdrop();
    });
  };

  slot(dropGumSlot, (v) => (gumdrop.gumdropSlot = v));
  slot(dropPickSlot, (v) => (gumdrop.pickaxeSlot = v));
  slot(dropSwordSlot, (v) => (gumdrop.swordSlot = v));

  const timing = (input: HTMLInputElement, cap: number, apply: (value: number) => void) => {
    input.addEventListener("change", () => {
      apply(Math.min(cap, Math.max(0, Math.round(Number(input.value) || 0))));
      renderGumdrop();
  renderSkywars();
      pushGumdrop();
    });
  };

  timing(dropKeyHold, 2000, (v) => (gumdrop.keyHoldMs = v));
  timing(dropAfterGum, 5000, (v) => (gumdrop.afterGumdropMs = v));
  timing(dropAfterPick, 5000, (v) => (gumdrop.afterPickaxeMs = v));
  timing(dropAfterBreak, 5000, (v) => (gumdrop.afterBreakMs = v));
  timing(dropClickHold, 2000, (v) => (gumdrop.clickHoldMs = v));
  timing(dropWait, 5000, (v) => (gumdrop.placeWaitMs = v));
}

function pushSkywars(): void {
  window.clearTimeout(skyPushTimer);
  skyPushTimer = window.setTimeout(() => {
    void invoke<Skywars>("apply_skywars", { config: skywars }).catch(() => {});
  }, 140);
}

function renderSkywars(): void {
  skySettle.value = String(skywars.settleMs);
  skyClickHold.value = String(skywars.clickHoldMs);
  skyBetween.value = String(skywars.betweenMs);
  skyClicks.value = String(skywars.clicksPerItem);
  skyRetryGap.value = String(skywars.retryGapMs);
  skyRestore.checked = skywars.restoreCursor;

  skyBindToggle.checked = skywars.bindEnabled;
  skyBindWrap.classList.toggle("open", skywars.bindEnabled);
  text(skyBindLabel, vkLabel(skywars.bindVk));

  text(
    skyEntryNote,
    skywars.bindEnabled ? `Press ${vkLabel(skywars.bindVk)} to loot a chest` : "Ready",
  );
}

function renderSkywarsStatus(status: SkywarsStatus): void {
  skyEntryDot.classList.toggle("live", status.busy);
  text(skyNote, status.note);
}

function pushDavey(): void {
  void invoke<Davey>("apply_davey", { config: davey }).catch(() => {});
}

function renderDavey(): void {
  dvyPickSlot.value = String(davey.pickaxeSlot);
  dvyHoldMs.value = String(davey.holdMs);
  dvyKeyHold.value = String(davey.keyHoldMs);
  dvyBurstMs.value = String(davey.burstMs);
  dvyBurstCps.value = String(davey.burstCps);
  dvyBurstDuty.value = String(davey.burstDuty);

  text(dvyHoldLabel, vkLabel(davey.holdVk));

  dvyBindToggle.checked = davey.bindEnabled;
  dvyBindWrap.classList.toggle("open", davey.bindEnabled);
  text(dvyBindLabel, vkLabel(davey.bindVk));

  text(
    dvyEntryNote,
    davey.bindEnabled ? `Press ${vkLabel(davey.bindVk)} to run` : "Ready",
  );
}

function renderDaveyStatus(status: DaveyStatus): void {
  dvyEntryDot.classList.toggle("live", status.busy);
  text(dvyRuns, status.runs === 0 ? "Not run yet" : `Run ${status.runs} times`);
}

function wireDavey(): void {
  dvyEntry.addEventListener("click", () => {
    const open = !dvyBody.classList.contains("open");
    dvyBody.classList.toggle("open", open);
    dvyEntry.classList.toggle("open", open);
    dvyEntry.setAttribute("aria-expanded", String(open));
  });

  dvyRunBtn.addEventListener("click", () => {
    void invoke("fire_davey");
  });

  dvyBindToggle.addEventListener("change", () => {
    davey.bindEnabled = dvyBindToggle.checked;
    renderDavey();
    pushDavey();
  });

  const rebind = (button: HTMLButtonElement, target: CaptureTarget) => {
    button.addEventListener("click", () => {
      if (capturing) {
        void invoke("cancel_capture");
        endCapture();
        return;
      }
      beginCapture(target);
    });
  };

  rebind(dvyBindBtn, { kind: "dvyBind" });
  rebind(dvyHoldKeyBtn, { kind: "dvyHoldKey" });

  const number = (
    input: HTMLInputElement,
    low: number,
    high: number,
    apply: (value: number) => void,
  ) => {
    input.addEventListener("change", () => {
      apply(Math.min(high, Math.max(low, Math.round(Number(input.value) || 0))));
      renderDavey();
      pushDavey();
    });
  };

  number(dvyPickSlot, 1, 9, (v) => (davey.pickaxeSlot = v));
  number(dvyHoldMs, 0, 10000, (v) => (davey.holdMs = v));
  number(dvyKeyHold, 0, 2000, (v) => (davey.keyHoldMs = v));
  number(dvyBurstMs, 0, 10000, (v) => (davey.burstMs = v));
  number(dvyBurstCps, 1, 50000, (v) => (davey.burstCps = v));
  number(dvyBurstDuty, 5, 95, (v) => (davey.burstDuty = v));
}

function renderPresets(list: Preset[]): void {
  presetList.replaceChildren();

  list.forEach((preset, index) => {
    const row = document.createElement("div");
    row.className = "preset";

    const name = document.createElement("span");
    name.className = "preset-name";
    name.textContent = preset.name;

    const load = document.createElement("button");
    load.className = "chip";
    load.type = "button";
    load.textContent = "Load";
    load.addEventListener("click", () => void applyCode(preset.code));

    // same two-step as deleting a clicker: the first press arms it, and it
    // disarms itself if you walk away
    const drop = document.createElement("button");
    drop.className = "ghost-btn danger-btn";
    drop.type = "button";
    drop.textContent = "Delete";

    let armed = false;
    let timer: number | undefined;

    const disarm = () => {
      armed = false;
      drop.textContent = "Delete";
      drop.classList.remove("confirming");
      window.clearTimeout(timer);
    };

    drop.addEventListener("click", () => {
      if (!armed) {
        armed = true;
        drop.textContent = "Sure?";
        drop.classList.add("confirming");
        window.clearTimeout(timer);
        timer = window.setTimeout(disarm, 4000);
        return;
      }

      disarm();
      void invoke<Preset[]>("delete_preset", { index })
        .then(renderPresets)
        .catch(() => {});
    });

    drop.addEventListener("mouseleave", () => {
      if (armed) {
        window.clearTimeout(timer);
        timer = window.setTimeout(disarm, 1200);
      }
    });

    row.append(name, load, drop);
    presetList.append(row);
  });
}

/// Everything reads its settings at boot, so the simplest way to make a whole
/// new configuration take hold everywhere is to boot again.
async function applyCode(code: string): Promise<void> {
  try {
    await invoke("import_code", { code });
    location.reload();
  } catch (why) {
    text(shareNote, String(why));
  }
}

async function copyCode(scope: string, button: HTMLButtonElement): Promise<void> {
  const was = button.textContent ?? "";
  try {
    const code = await invoke<string>("export_code", { scope });
    await navigator.clipboard.writeText(code);
    button.textContent = "Copied";
  } catch {
    button.textContent = "Could not copy";
  }
  window.setTimeout(() => {
    button.textContent = was;
  }, 1400);
}

function wireSharing(): void {
  copyAllBtn.addEventListener("click", () => void copyCode("all", copyAllBtn));
  copySettingsBtn.addEventListener("click", () =>
    void copyCode("settings", copySettingsBtn),
  );
  copyClickerBtn.addEventListener("click", () => void copyCode("clicker", copyClickerBtn));

  shareBox.addEventListener("input", () => {
    const code = shareBox.value.trim();
    if (code.length === 0) {
      text(shareNote, "Paste a code to see what is in it.");
      return;
    }

    void invoke<string>("describe_code", { code })
      .then((what) => text(shareNote, `This code has ${what}.`))
      .catch((why) => text(shareNote, String(why)));
  });

  importCodeBtn.addEventListener("click", () => {
    const code = shareBox.value.trim();
    if (code.length === 0) {
      text(shareNote, "Nothing pasted yet.");
      return;
    }
    void applyCode(code);
  });

  savePresetBtn.addEventListener("click", () => {
    void invoke<Preset[]>("save_preset", { name: presetName.value })
      .then((list) => {
        presetName.value = "";
        renderPresets(list);
      })
      .catch(() => {});
  });
}

function pushOverlay(): void {
  void invoke<Overlay>("apply_overlay", { config: overlay }).catch(() => {});
}

function renderOverlay(): void {
  overlayToggle.checked = overlay.enabled;
  overlayWrap.classList.toggle("open", overlay.enabled);
  overlayXY.classList.toggle("open", overlay.position === "custom");

  overlayX.value = String(overlay.x);
  overlayY.value = String(overlay.y);

  overlayOnlyIn.checked = overlay.onlyInWindows;
  overlayNamesWrap.classList.toggle("open", overlay.onlyInWindows);
  if (document.activeElement !== overlayNames) {
    overlayNames.value = overlay.windows.join(", ");
  }

  overlaySpots.querySelectorAll<HTMLButtonElement>("[data-spot]").forEach((chip) => {
    chip.classList.toggle("on", chip.dataset.spot === overlay.position);
  });
}

function wireOverlay(): void {
  overlayToggle.addEventListener("change", () => {
    overlay.enabled = overlayToggle.checked;
    renderOverlay();
    pushOverlay();
  });

  overlaySpots.querySelectorAll<HTMLButtonElement>("[data-spot]").forEach((chip) => {
    chip.addEventListener("click", () => {
      overlay.position = chip.dataset.spot ?? "top-right";
      renderOverlay();
      pushOverlay();
    });
  });

  const spot = (input: HTMLInputElement, apply: (value: number) => void) => {
    input.addEventListener("change", () => {
      apply(Math.min(32000, Math.max(-32000, Math.round(Number(input.value) || 0))));
      renderOverlay();
      pushOverlay();
    });
  };

  spot(overlayX, (v) => (overlay.x = v));
  spot(overlayY, (v) => (overlay.y = v));

  overlayOnlyIn.addEventListener("change", () => {
    overlay.onlyInWindows = overlayOnlyIn.checked;
    renderOverlay();
    pushOverlay();
  });

  overlayNames.addEventListener("change", () => {
    overlay.windows = overlayNames.value
      .split(",")
      .map((name) => name.trim())
      .filter((name) => name.length > 0);
    renderOverlay();
    pushOverlay();
  });
}

function pushCrossbow(): void {
  void invoke<Crossbow>("apply_crossbow", { config: crossbow }).catch(() => {});
}

function renderCrossbow(): void {
  bowSlot.value = String(crossbow.crossbowSlot);
  bowSwordSlot.value = String(crossbow.swordSlot);
  bowKeyHold.value = String(crossbow.keyHoldMs);

  bowTactical.checked = crossbow.tacticalEnabled;
  bowTacticalWrap.classList.toggle("open", crossbow.tacticalEnabled);
  bowTacticalSlot.value = String(crossbow.tacticalSlot);
  bowSecondSwitch.value = String(crossbow.secondSwitchMs);

  text(
    bowSummary,
    crossbow.tacticalEnabled
      ? "Runs once per press: tactical crossbow, shoot, ordinary crossbow, shoot, back to your sword. This can share a key with a clicker — both will fire."
      : "Runs once per press: swaps to the crossbow, shoots, swaps back to your sword. This can share a key with a clicker — both will fire.",
  );
  bowAfterSwitch.value = String(crossbow.afterSwitchMs);
  bowClickHold.value = String(crossbow.clickHoldMs);
  bowAfterClick.value = String(crossbow.afterClickMs);

  bowBindToggle.checked = crossbow.bindEnabled;
  bowBindWrap.classList.toggle("open", crossbow.bindEnabled);
  text(bowBindLabel, vkLabel(crossbow.bindVk));

  text(
    bowEntryNote,
    crossbow.bindEnabled ? `Press ${vkLabel(crossbow.bindVk)} to shoot` : "Ready",
  );
}

function renderCrossbowStatus(status: CrossbowStatus): void {
  bowEntryDot.classList.toggle("live", status.busy);
  text(bowRuns, status.runs === 0 ? "Not run yet" : `Run ${status.runs} times`);
}

function wireCrossbow(): void {
  bowEntry.addEventListener("click", () => {
    const open = !bowBody.classList.contains("open");
    bowBody.classList.toggle("open", open);
    bowEntry.classList.toggle("open", open);
    bowEntry.setAttribute("aria-expanded", String(open));
  });

  bowRunBtn.addEventListener("click", () => {
    void invoke("fire_crossbow");
  });

  bowBindToggle.addEventListener("change", () => {
    crossbow.bindEnabled = bowBindToggle.checked;
    renderCrossbow();
    pushCrossbow();
  });

  bowBindBtn.addEventListener("click", () => {
    if (capturing) {
      void invoke("cancel_capture");
      endCapture();
      return;
    }
    beginCapture({ kind: "bowBind" });
  });

  const number = (
    input: HTMLInputElement,
    low: number,
    high: number,
    apply: (value: number) => void,
  ) => {
    input.addEventListener("change", () => {
      apply(Math.min(high, Math.max(low, Math.round(Number(input.value) || 0))));
      renderCrossbow();
      pushCrossbow();
    });
  };

  bowTactical.addEventListener("change", () => {
    crossbow.tacticalEnabled = bowTactical.checked;
    renderCrossbow();
    pushCrossbow();
  });

  number(bowSlot, 1, 9, (v) => (crossbow.crossbowSlot = v));
  number(bowTacticalSlot, 1, 9, (v) => (crossbow.tacticalSlot = v));
  number(bowSecondSwitch, 0, 5000, (v) => (crossbow.secondSwitchMs = v));
  number(bowSwordSlot, 1, 9, (v) => (crossbow.swordSlot = v));
  number(bowKeyHold, 0, 2000, (v) => (crossbow.keyHoldMs = v));
  number(bowAfterSwitch, 0, 5000, (v) => (crossbow.afterSwitchMs = v));
  number(bowClickHold, 0, 2000, (v) => (crossbow.clickHoldMs = v));
  number(bowAfterClick, 0, 5000, (v) => (crossbow.afterClickMs = v));
}

/**
 * Reset buttons, found by `data-reset` rather than listed one at a time.
 *
 * Each one asks Rust for that macro's factory settings, so no default is
 * written down twice and none of them can drift out of step with the code
 * that actually uses them.
 */
function wireResets(): void {
  // Settings are copied into the existing object rather than swapped for a
  // new one, so anything else already holding a reference still sees the
  // change.
  const restore: Record<
    string,
    (fresh: Record<string, unknown>) => void
  > = {
    fisher: (fresh) => {
      Object.assign(fisher, fresh, keepBind(fisher));
      renderFisher();
      pushFisher();
    },
    gumdrop: (fresh) => {
      Object.assign(gumdrop, fresh, keepBind(gumdrop));
      renderGumdrop();
      pushGumdrop();
    },
    skywars: (fresh) => {
      Object.assign(skywars, fresh, keepBind(skywars));
      renderSkywars();
      pushSkywars();
    },
    davey: (fresh) => {
      Object.assign(davey, fresh, keepBind(davey));
      renderDavey();
      pushDavey();
    },
    crossbow: (fresh) => {
      Object.assign(crossbow, fresh, keepBind(crossbow));
      renderCrossbow();
      pushCrossbow();
    },
    overlay: (fresh) => {
      Object.assign(overlay, fresh);
      renderOverlay();
      pushOverlay();
    },
  };

  document
    .querySelectorAll<HTMLButtonElement>("[data-reset]")
    .forEach((button) => {
      const which = button.dataset.reset ?? "";
      const apply = restore[which];
      if (!apply) return;

      let armed = false;
      let timer: number | undefined;

      const disarm = () => {
        armed = false;
        button.textContent = "Reset";
        button.classList.remove("confirming");
        window.clearTimeout(timer);
      };

      button.addEventListener("click", () => {
        // Nothing here is recoverable once it is gone, so it takes two
        // presses, the same as deleting a saved profile does.
        if (!armed) {
          armed = true;
          button.textContent = "Sure?";
          button.classList.add("confirming");
          window.clearTimeout(timer);
          timer = window.setTimeout(disarm, 4000);
          return;
        }

        disarm();
        void invoke<Record<string, unknown>>("macro_defaults", { which })
          .then(apply)
          .catch(() => {});
      });

      button.addEventListener("mouseleave", () => {
        if (armed) disarm();
      });
    });
}

/** The hotkey is yours, not part of the tuning, so a reset leaves it alone. */
function keepBind(current: { bindEnabled: boolean; bindVk: number }): {
  bindEnabled: boolean;
  bindVk: number;
} {
  return { bindEnabled: current.bindEnabled, bindVk: current.bindVk };
}

/** Scroll to change, select on click, and no reaction to the arrow keys. */
function wireNumberInputs(): void {
  const nudge = (input: HTMLInputElement, by: number) => {
    const now = Number(input.value) || 0;
    const min = input.min === "" ? -Infinity : Number(input.min);
    const max = input.max === "" ? Infinity : Number(input.max);

    const next = Math.min(max, Math.max(min, now + by));
    if (next === now) return;

    input.value = String(next);
    input.dispatchEvent(new Event("change"));
  };

  document
    .querySelectorAll<HTMLInputElement>('.macro-body input[type="number"]')
    .forEach((input) => {
      const step = () => Number(input.step) || 1;

      // the arrow keys leave the value alone
      input.addEventListener("keydown", (event) => {
        if (event.key === "ArrowUp" || event.key === "ArrowDown") {
          event.preventDefault();
        }
      });

      input.addEventListener(
        "wheel",
        (event) => {
          event.preventDefault();
          const jump = event.shiftKey ? step() * 10 : step();
          nudge(input, event.deltaY < 0 ? jump : -jump);
        },
        { passive: false },
      );

      // Clicking a field selects what is in it, so typing a new value
      // replaces the old one instead of landing beside a digit.
      input.addEventListener("focus", () => input.select());
    });
}

function wireSkywars(): void {
  skyEntry.addEventListener("click", () => {
    const open = !skyBody.classList.contains("open");
    skyBody.classList.toggle("open", open);
    skyEntry.classList.toggle("open", open);
    skyEntry.setAttribute("aria-expanded", String(open));
  });

  skyRunBtn.addEventListener("click", () => {
    void invoke("fire_skywars");
  });

  skyBindToggle.addEventListener("change", () => {
    skywars.bindEnabled = skyBindToggle.checked;
    renderSkywars();
    pushSkywars();
  });

  skyRestore.addEventListener("change", () => {
    skywars.restoreCursor = skyRestore.checked;
    pushSkywars();
  });

  skyBindBtn.addEventListener("click", () => {
    if (capturing) {
      void invoke("cancel_capture");
      endCapture();
      return;
    }
    beginCapture({ kind: "skyBind" });
  });

  const timing = (input: HTMLInputElement, cap: number, apply: (value: number) => void) => {
    input.addEventListener("change", () => {
      apply(Math.min(cap, Math.max(0, Math.round(Number(input.value) || 0))));
      renderSkywars();
      pushSkywars();
    });
  };

  timing(skySettle, 2000, (v) => (skywars.settleMs = v));
  timing(skyClickHold, 2000, (v) => (skywars.clickHoldMs = v));
  timing(skyBetween, 5000, (v) => (skywars.betweenMs = v));
  timing(skyRetryGap, 2000, (v) => (skywars.retryGapMs = v));

  skyClicks.addEventListener("change", () => {
    skywars.clicksPerItem = Math.min(5, Math.max(1, Math.round(Number(skyClicks.value) || 1)));
    renderSkywars();
    pushSkywars();
  });
}

function renderAutomationStatus(status: AutomationStatus): void {
  macroRunning = status.running;
  syncTitleDot();

  autoHero.classList.toggle("live", status.running);
  autoRunBtn.classList.toggle("live", status.running);
  autoStatus.textContent = status.running ? "Running" : "Stopped";
  autoRunLabel.textContent = status.running ? "Stop" : "Run";

  const step = status.running ? status.step : -1;
  if (step !== runningStep) {
    runningStep = step;
    renderSteps();
  }

  if (status.running) {
    autoSub.textContent =
      automation.repeat === 0
        ? `Pass ${status.pass} — looping`
        : `Pass ${status.pass} of ${automation.repeat}`;
  } else {
    autoSub.textContent = automation.bindEnabled
      ? `Press ${vkLabel(automation.bindVk)} to run`
      : "Press Run to start";
  }
}

interface RecordingStatus {
  recording: boolean;
  events: number;
  seconds: number;
}

let recordPoll = 0;

function renderRecording(status: RecordingStatus): void {
  recordBtn.textContent = status.recording ? "Stop" : "Record";
  recordBtn.classList.toggle("recording", status.recording);

  recordMoves.disabled = status.recording;

  recordHint.textContent = status.recording
    ? `Recording — ${status.events} step${status.events === 1 ? "" : "s"}, ${status.seconds.toFixed(1)}s`
    : "Do it once by hand and the steps write themselves, with the positions and timing already right.";
}

async function toggleRecording(): Promise<void> {
  const status = await invoke<RecordingStatus>("recording_status");

  if (status.recording) {
    window.clearInterval(recordPoll);
    const steps = await invoke<Step[]>("stop_recording");

    if (steps.length === 0) {
      recordHint.textContent = "Nothing was recorded — no clicks or keys outside this window.";
      renderRecording({ recording: false, events: 0, seconds: 0 });
      return;
    }

    automation.steps.push(...steps);
    renderSteps();
    pushAutomation();
    renderRecording({ recording: false, events: 0, seconds: 0 });
    recordHint.textContent = `Added ${steps.length} step${steps.length === 1 ? "" : "s"}.`;
    return;
  }

  const started = await invoke<RecordingStatus>("start_recording", {
    withMoves: recordMoves.checked,
  });
  renderRecording(started);

  window.clearInterval(recordPoll);
  recordPoll = window.setInterval(() => {
    void invoke<RecordingStatus>("recording_status")
      .then(renderRecording)
      .catch(() => {});
  }, 200);
}

function wireAutomation(): void {
  recordBtn.addEventListener("click", () => {
    void toggleRecording().catch(() => {
      recordHint.textContent = "Couldn't start recording.";
    });
  });

  autoRunBtn.addEventListener("click", () => {
    void invoke<boolean>("toggle_automation");
  });

  autoBindToggle.addEventListener("change", () => {
    automation.bindEnabled = autoBindToggle.checked;
    renderAutomation();
    pushAutomation();
  });

  autoBindBtn.addEventListener("click", () => {
    if (capturing) {
      void invoke("cancel_capture");
      endCapture();
      return;
    }
    beginCapture({ kind: "autoBind" });
  });

  autoRepeat.addEventListener("change", () => {
    automation.repeat = Math.max(0, Number(autoRepeat.value) || 1);
    renderAutomation();
    pushAutomation();
  });

  autoLoopBtn.addEventListener("click", () => {
    automation.repeat = automation.repeat === 0 ? 1 : 0;
    renderAutomation();
    pushAutomation();
  });

  wirePair(
    autoDelay,
    autoDelayValue,
    (value) => {
      automation.stepDelayMs = Math.max(0, value);
    },
    () => {
      autoDelay.value = String(Math.min(automation.stepDelayMs, 2000));
      autoDelayValue.value = trimNum(automation.stepDelayMs);
      paintRange(autoDelay);
    },
  );

  autoClearBtn.addEventListener("click", () => {
    automation.steps = [];
    renderSteps();
    pushAutomation();
  });

  document.querySelectorAll<HTMLButtonElement>("[data-add]").forEach((button) => {
    button.addEventListener("click", () => {
      const kind = button.dataset.add as Step["kind"] | undefined;
      if (!kind) return;
      automation.steps.push(newStep(kind));
      renderSteps();
      pushAutomation();
    });
  });
}

function moveIndicator(item: HTMLElement): void {
  navIndicator.style.height = `${item.offsetHeight}px`;
  navIndicator.style.transform = `translateY(${item.offsetTop}px)`;
  navIndicator.classList.add("ready");
}

function wireTabs(): void {
  const items = Array.from(document.querySelectorAll<HTMLButtonElement>(".nav-item"));
  let currentIndex = items.findIndex((item) => item.classList.contains("active"));
  if (currentIndex < 0) currentIndex = 0;

  items.forEach((item, index) => {
    item.addEventListener("click", () => {
      const target = item.dataset.tab;
      if (!target || index === currentIndex) return;

      const goingDown = index > currentIndex;
      currentIndex = index;

      items.forEach((other) => other.classList.toggle("active", other === item));
      moveIndicator(item);

      document.querySelectorAll<HTMLElement>(".panel").forEach((panel) => {
        const isTarget = panel.dataset.panel === target;
        panel.classList.remove("enter-down", "enter-up");
        panel.classList.toggle("active", isTarget);
        if (isTarget) {

          void panel.offsetWidth;
          panel.classList.add(goingDown ? "enter-down" : "enter-up");
        }
      });

      requestAnimationFrame(positionAllSegments);

      if (target === "optimize") void refreshOptimizations();

      if (target === "clicker") requestAnimationFrame(drawGraph);
    });
  });

  const active = items[currentIndex];
  if (active) {

    requestAnimationFrame(() => moveIndicator(active));
  }
}

function wireWindowButtons(): void {
  el<HTMLButtonElement>("btnMinimize").addEventListener("click", () => {
    void invoke("window_minimize");
  });
  el<HTMLButtonElement>("btnClose").addEventListener("click", () => {
    void invoke("window_close");
  });
}

function wireSegments(): void {
  modeGroup.querySelectorAll<HTMLButtonElement>(".seg").forEach((seg) => {
    seg.addEventListener("click", () => {
      profile.mode = seg.dataset.value ?? "toggle";
      setSegment(modeGroup, profile.mode);
      renderModeHint();
      push();
    });
  });

  buttonGroup.querySelectorAll<HTMLButtonElement>(".seg").forEach((seg) => {
    seg.addEventListener("click", () => {
      profile.button = seg.dataset.value ?? "left";
      setSegment(buttonGroup, profile.button);
      renderConflict();
      push();
    });
  });

  swatchRow.querySelectorAll<HTMLButtonElement>(".swatch").forEach((swatch) => {
    swatch.addEventListener("click", () => {
      settings.accentHue = Number(swatch.dataset.hue) || 222;
      renderAccent();
      push();
    });
  });

  wirePair(
    accentSlider,
    accentValue,
    (value) => {

      settings.accentHue = ((value % 360) + 360) % 360;
    },
    renderAccent,
  );

  wirePair(
    accentSatSlider,
    accentSatValue,
    (value) => {
      settings.accentSat = Math.min(100, Math.max(0, value));
    },
    renderAccent,
  );

  themeGroup.querySelectorAll<HTMLButtonElement>(".seg").forEach((seg) => {
    seg.addEventListener("click", () => {
      settings.theme = seg.dataset.value ?? "gradient-dark";
      renderTheme();
      push();
    });
  });

  deliveryDirect.addEventListener("change", () => {
    profile.delivery = deliveryDirect.checked ? "window" : "system";
    renderDelivery();
    renderConflict();
    push();
  });

  targetModeGroup.querySelectorAll<HTMLButtonElement>(".seg").forEach((seg) => {
    seg.addEventListener("click", () => {
      profile.targetMode = seg.dataset.value ?? "cursor";
      if (profile.targetMode === "pinned" && knownWindows.length === 0) {
        refreshWindows();
      }
      renderDelivery();
      push();
    });
  });

  refreshWindowsBtn.addEventListener("click", refreshWindows);

  resetPointBtn.addEventListener("click", () => {

    profile.targetX = -1;
    profile.targetY = -1;
    renderTargetMode();
    push();
  });

  pickPointBtn.addEventListener("click", () => {
    if (capturing) {
      void invoke("cancel_capture");
      endCapture();
      return;
    }
    if (!profile.targetTitle) {
      pinnedPointLabel.textContent = "Choose a window first";
      return;
    }
    pinnedPointLabel.textContent = "Point at the spot and click…";
    beginCapture({ kind: "clickPoint" });
  });

  bindToggle.addEventListener("change", () => {
    profile.bindEnabled = bindToggle.checked;
    renderBind();
    push();
  });

  dutyToggle.addEventListener("change", () => {
    profile.dutyEnabled = dutyToggle.checked;
    renderDuty();
    push();
  });

  precisionGroup.querySelectorAll<HTMLButtonElement>(".seg").forEach((seg) => {
    seg.addEventListener("click", () => {
      profile.precision = seg.dataset.value ?? "balanced";
      setSegment(precisionGroup, profile.precision);
      precisionHint.textContent =
        profile.precision === "max"
          ? "Spin-driven. Sub-millisecond accuracy, but pegs a full core while active — expect fan noise, and possibly coil whine or audio stutter."
          : "Sleep-driven. Barely touches the CPU. Batch sizing corrects for any timing slop, so the average rate is just as accurate.";
      push();
    });
  });
}

function captureButton(target: CaptureTarget): HTMLElement | null {
  switch (target.kind) {
    case "bind":
      return bindBtn;
    case "panic":
      return panicBtn;
    case "autoBind":
      return autoBindBtn;
    case "fisherBind":
      return fisherBindBtn;
    case "dropBind":
      return dropBindBtn;
    case "skyBind":
      return skyBindBtn;
    case "dvyBind":
      return dvyBindBtn;
    case "dvyHoldKey":
      return dvyHoldKeyBtn;
    case "bowBind":
      return bowBindBtn;
    default:
      return null;
  }
}

function beginCapture(target: CaptureTarget): void {
  capturing = target;

  const button = captureButton(target);
  if (button) {
    button.classList.add("listening");
    const label = button.querySelector(".keycap");
    if (label) label.textContent = "Press any key…";
  }

  const wantsPosition =
    target.kind === "position" ||
    target.kind === "clickPoint" ||
    target.kind === "extraPoint" ||
    target.kind === "pixel";
  void invoke(wantsPosition ? "begin_position_capture" : "begin_capture");
  if (target.kind === "position" || target.kind === "stepKey") renderSteps();
}

function endCapture(): void {
  bindBtn.classList.remove("listening");
  panicBtn.classList.remove("listening");
  autoBindBtn.classList.remove("listening");
  const wasStep = capturing?.kind === "position" || capturing?.kind === "stepKey";
  capturing = null;
  bindLabel.textContent = vkLabel(profile.bindVk);
  panicLabel.textContent = vkLabel(settings.panicVk);
  autoBindLabel.textContent = vkLabel(automation.bindVk);
  if (wasStep) renderSteps();
}

function wireBinds(): void {
  bindBtn.addEventListener("click", () => {
    if (capturing) {
      void invoke("cancel_capture");
      endCapture();
      return;
    }
    beginCapture({ kind: "bind" });
  });

  panicBtn.addEventListener("click", () => {
    if (capturing) {
      void invoke("cancel_capture");
      endCapture();
      return;
    }
    beginCapture({ kind: "panic" });
  });

  clearPanicBtn.addEventListener("click", () => {
    settings.panicVk = 0;
    panicLabel.textContent = vkLabel(0);
    push();
  });
}

function commitCps(which: "min" | "max", raw: number): void {
  const value = Math.min(CPS_CEILING, Math.max(0.01, raw));
  if (which === "max") {
    profile.cpsMax = value;
    if (profile.cpsMin > value) profile.cpsMin = value;
  } else {
    profile.cpsMin = value;
    if (profile.cpsMax < value) profile.cpsMax = value;
  }
  renderCps();
  renderDuty();

  resetGraph();
  push();
}

function fieldToCps(input: HTMLInputElement): number {
  const raw = Number(input.value);
  if (!Number.isFinite(raw) || raw <= 0) return 0.01;
  return delayMode() ? delayToCps(raw) : raw;
}

function wireCps(): void {
  rateModeGroup.querySelectorAll<HTMLButtonElement>(".seg").forEach((seg) => {
    seg.addEventListener("click", () => {
      profile.rateMode = seg.dataset.value ?? "cps";
      renderCps();
      push();
    });
  });

  cpsMaxSlider.addEventListener("input", () => {
    commitCps("max", positionToCps(Number(cpsMaxSlider.value)));
  });
  cpsMinSlider.addEventListener("input", () => {
    commitCps("min", positionToCps(Number(cpsMinSlider.value)));
  });

  cpsMaxInput.addEventListener("change", () => {
    commitCps("max", fieldToCps(cpsMaxInput));
  });
  cpsMinInput.addEventListener("change", () => {
    commitCps("min", fieldToCps(cpsMinInput));
  });

  cpsChips.querySelectorAll<HTMLButtonElement>(".chip").forEach((chip) => {
    chip.addEventListener("click", () => {
      commitCps("max", Number(chip.dataset.cps) || 1);
    });
  });
}

function wireToggles(): void {
  randomizeToggle.addEventListener("change", () => {
    profile.randomize = randomizeToggle.checked;
    renderCps();
    push();
  });

  shakeToggle.addEventListener("change", () => {
    profile.shakeEnabled = shakeToggle.checked;
    renderAll();
    push();
  });

  const shakeField = (
    input: HTMLInputElement,
    low: number,
    high: number,
    apply: (value: number) => void,
  ) => {
    input.addEventListener("change", () => {
      apply(Math.min(high, Math.max(low, Number(input.value) || low)));
      renderAll();
      push();
    });
  };

  shakeField(shakePx, 1, 400, (v) => (profile.shakePx = v));
  shakeField(shakeMs, 1, 60000, (v) => (profile.shakeMs = v));

  wirePair(
    jitterSlider,
    jitterValue,
    (value) => {
      profile.jitter = Math.min(Math.max(value, 0), 95);
    },
    () => {
      jitterSlider.value = String(profile.jitter);
      jitterValue.value = trimNum(profile.jitter);
      paintRange(jitterSlider);
    },
  );

  wirePair(
    dutySlider,
    dutyValue,
    (value) => {
      profile.dutyCycle = Math.min(Math.max(value, 0.1), 95);
    },
    renderDuty,
  );

  sequenceToggle.addEventListener("change", () => {
    profile.sequenceEnabled = sequenceToggle.checked;
    reveal(sequenceWrap, profile.sequenceEnabled);
    buttonHint.hidden = !profile.sequenceEnabled;
    buttonGroup.style.opacity = profile.sequenceEnabled ? "0.45" : "";
    renderConflict();
    if (profile.sequenceEnabled) renderSequencePreview();
    push();
  });

  sequenceInput.addEventListener("input", () => {
    profile.sequence = sequenceInput.value;
    renderSequencePreview();
    push();
  });

  limitToggle.addEventListener("change", () => {
    profile.limitEnabled = limitToggle.checked;
    reveal(limitWrap, profile.limitEnabled);
    push();
  });

  limitInput.addEventListener("change", () => {
    profile.limitCount = Math.max(1, Number(limitInput.value) || 1);
    limitInput.value = String(profile.limitCount);
    push();
  });

  addPointBtn.addEventListener("click", () => {
    if (capturing) {
      void invoke("cancel_capture");
      endCapture();
      return;
    }
    if (!profile.targetTitle) {
      pointHint.textContent = "Choose a window first.";
      return;
    }
    pointHint.textContent = "Point at the next spot and click…";
    beginCapture({ kind: "extraPoint" });
  });

  burstToggle.addEventListener("change", () => {
    profile.burstEnabled = burstToggle.checked;
    renderBurst();
    push();
  });

  burstCount.addEventListener("change", () => {
    profile.burstCount = Math.max(1, Math.round(Number(burstCount.value) || 1));
    renderBurst();
    push();
  });

  burstPause.addEventListener("change", () => {
    profile.burstPauseMs = Math.min(600000, Math.max(0, Number(burstPause.value) || 0));
    renderBurst();
    push();
  });

  pixelToggle.addEventListener("change", () => {
    profile.pixelEnabled = pixelToggle.checked;
    renderPixel();
    push();
  });

  pixelStopGroup.querySelectorAll<HTMLButtonElement>(".seg").forEach((seg) => {
    seg.addEventListener("click", () => {
      profile.pixelStopOn = seg.dataset.value ?? "change";
      renderPixel();
      push();
    });
  });

  pixelTolerance.addEventListener("change", () => {
    profile.pixelTolerance = Math.min(100, Math.max(0, Number(pixelTolerance.value) || 0));
    renderPixel();
    push();
  });

  pickPixelBtn.addEventListener("click", () => {
    if (capturing) {
      void invoke("cancel_capture");
      endCapture();
      return;
    }
    pixelHint.textContent = "Point at the spot and click…";
    beginCapture({ kind: "pixel" });
  });

  resamplePixelBtn.addEventListener("click", () => {
    void invoke<number | null>("sample_pixel", {
      x: Math.round(profile.pixelX),
      y: Math.round(profile.pixelY),
    })
      .then((rgb) => {
        if (rgb === null) {
          pixelHint.textContent =
            "Couldn't read that pixel — fullscreen games and video overlays are invisible to this.";
          return;
        }
        profile.pixelRgb = rgb;
        renderPixel();
        push();
      })
      .catch(() => {});
  });

  timeLimitToggle.addEventListener("change", () => {
    profile.timeLimitEnabled = timeLimitToggle.checked;
    renderTimeLimit();
  renderBurst();
  renderPixel();
    push();
  });

  timeLimitInput.addEventListener("change", () => {
    const seconds = Number(timeLimitInput.value);
    profile.timeLimitSecs = Math.min(86400, Math.max(0.1, seconds || 0.1));
    renderTimeLimit();
  renderBurst();
  renderPixel();
    push();
  });

  startDelayToggle.addEventListener("change", () => {
    profile.startDelayEnabled = startDelayToggle.checked;
    reveal(startDelayWrap, profile.startDelayEnabled);
    push();
  });

  startDelayInput.addEventListener("change", () => {
    const ms = Number(startDelayInput.value);
    profile.startDelayMs = Math.min(60000, Math.max(0, ms || 0));
    startDelayInput.value = trimNum(profile.startDelayMs);
    push();
  });

  filterToggle.addEventListener("change", () => {
    profile.filterEnabled = filterToggle.checked;
    reveal(filterWrap, profile.filterEnabled);
    push();
  });

  filterInput.addEventListener("input", () => {
    profile.filterTitle = filterInput.value;
    push();
  });

  edgeGuardToggle.addEventListener("change", () => {
    settings.edgeGuardEnabled = edgeGuardToggle.checked;
    reveal(edgeGuardWrap, settings.edgeGuardEnabled);
    push();
  });

  wirePair(
    edgeGuardSlider,
    edgeGuardValue,
    (value) => {

      settings.edgeGuardPx = Math.min(Math.max(Math.round(value), 1), 200);
    },
    () => {
      edgeGuardSlider.value = String(Math.min(settings.edgeGuardPx, 120));
      edgeGuardValue.value = String(Math.round(settings.edgeGuardPx));
      paintRange(edgeGuardSlider);
    },
  );

  edgeGuardMode.querySelectorAll<HTMLButtonElement>(".seg").forEach((seg) => {
    seg.addEventListener("click", () => {
      settings.edgeGuardMode = seg.dataset.value ?? "window";
      renderAll();
      push();
    });
  });

  edgeGuardChrome.addEventListener("change", () => {
    settings.edgeGuardChrome = edgeGuardChrome.checked;
    push();
  });

  blurToggle.addEventListener("change", () => {
    settings.blurEnabled = blurToggle.checked;
    push();
  });

  acrylicToggle.addEventListener("change", () => {
    settings.acrylic = acrylicToggle.checked;
    push();
  });

  opacitySlider.addEventListener("input", () => {
    settings.opacity = Number(opacitySlider.value) / 100;
    renderAppearance();
    push();
  });

  pinBtn.addEventListener("click", () => {
    settings.alwaysOnTop = !settings.alwaysOnTop;
    pinBtn.classList.toggle("live", settings.alwaysOnTop);
    pinBtn.setAttribute("aria-pressed", String(settings.alwaysOnTop));
    push();
  });
}

const START_GUARD_MS = 1000;
let powerGuardTimer = 0;

function guardPowerButton(): void {
  window.clearTimeout(powerGuardTimer);

  powerBtn.disabled = true;
  powerBtn.classList.add("cooling");

  powerGuardTimer = window.setTimeout(() => {
    powerBtn.disabled = false;
    powerBtn.classList.remove("cooling");
  }, START_GUARD_MS);
}

function wireActions(): void {
  powerBtn.addEventListener("click", () => {
    void invoke<boolean>("toggle_active", { index: settings.selected }).then((active) => {
      renderActive(active);

      if (active) guardPowerButton();
    });
  });

  resetBtn.addEventListener("click", () => {
    void invoke("reset_clicks", { index: settings.selected });
    lastRun = null;
    statClicks.textContent = "0";
  });

  resetStatsBtn.addEventListener("click", () => {
    void invoke("reset_stats");
    statTotalClicks.textContent = "0";
    statActiveTime.textContent = "0s";
  });

  addProfileBtn.addEventListener("click", () => {

    void invoke<Profile>("default_profile").then((fresh) => {
      fresh.name = `Clicker ${settings.profiles.length + 1}`;
      fresh.enabled = true;
      fresh.bindEnabled = false;
      fresh.bindVk = 0;

      settings.profiles.push(fresh);
      selectProfile(settings.profiles.length - 1);
      push();
    });
  });

  deleteProfileBtn.addEventListener("click", () => {
    if (settings.profiles.length <= 1) return;

    if (!deleteArmed) {
      armDelete();
      return;
    }

    disarmDelete();
    settings.profiles.splice(settings.selected, 1);
    selectProfile(Math.min(settings.selected, settings.profiles.length - 1));
    push();
  });

  profileEnabled.addEventListener("change", () => {
    profile.enabled = profileEnabled.checked;
    renderProfileTabs();
    renderModeHint();
    push();
  });

  profileName.addEventListener("input", () => {
    profile.name = profileName.value;

    const tab = profileTabs.children[settings.selected];
    const label = tab?.querySelector("span:last-child");
    if (label) label.textContent = profile.name;
    push();
  });

  cursorStyleGroup.querySelectorAll<HTMLButtonElement>(".seg").forEach((seg) => {
    seg.addEventListener("click", () => {
      settings.cursorStyle = seg.dataset.value ?? "image";
      renderCursor();
      push();

      if (settings.cursorStyle === "custom" && !settings.cursorImage) {
        cursorFile.click();
      }
    });
  });

  cursorPickBtn.addEventListener("click", () => cursorFile.click());

  cursorFile.addEventListener("change", () => {
    const file = cursorFile.files?.[0];

    cursorFile.value = "";
    if (file) void loadCursorImage(file);
  });

  cursorClearBtn.addEventListener("click", () => {
    settings.cursorImage = "";
    settings.cursorStyle = "image";
    renderCursor();
    push();
  });

  wirePair(
    cursorSizeSlider,
    cursorSizeValue,
    (value) => {
      settings.cursorSize = Math.min(Math.max(value, 12), 64);
    },
    renderCursor,
  );
}

async function wireEvents(): Promise<void> {
  await listen<Status>("status", (event) => {
    renderStatus(event.payload);
    if (!event.payload.capturing && capturing) {

      endCapture();
    }
  });

  await listen<number>("bind-captured", (event) => {
    const vk = event.payload;
    const target = capturing;
    if (!target) return;

    if (target.kind === "panic") {
      settings.panicVk = vk;
      endCapture();
      push();
    } else if (target.kind === "autoBind") {

      const clash = settings.profiles.find(
        (p) => p.enabled !== false && p.bindEnabled && p.bindVk === vk,
      );
      if (clash) {
        endCapture();
        autoBindHint.textContent = `That's ${clash.name}'s bind — pick a different key.`;
        return;
      }
      automation.bindVk = vk;
      endCapture();
      renderAutomation();
      pushAutomation();
    } else if (target.kind === "fisherBind") {
      fisher.bindVk = vk;
      endCapture();
      renderFisher();
  renderGumdrop();
  renderSkywars();
      pushFisher();
    } else if (target.kind === "dropBind") {
      gumdrop.bindVk = vk;
      endCapture();
      renderGumdrop();
  renderSkywars();
      pushGumdrop();
    } else if (target.kind === "skyBind") {
      skywars.bindVk = vk;
      endCapture();
      renderSkywars();
      pushSkywars();
    } else if (target.kind === "dvyBind") {
      davey.bindVk = vk;
      endCapture();
      renderDavey();
      pushDavey();
    } else if (target.kind === "dvyHoldKey") {
      davey.holdVk = vk;
      endCapture();
      renderDavey();
      pushDavey();
    } else if (target.kind === "bowBind") {
      crossbow.bindVk = vk;
      endCapture();
      renderCrossbow();
      pushCrossbow();
    } else if (target.kind === "stepKey") {
      const step = automation.steps[target.index];
      if (step && step.kind === "key") step.vk = vk;
      endCapture();
      pushAutomation();
    } else {
      if (automation.bindEnabled && vk === automation.bindVk) {
        endCapture();
        bindHint.textContent = "That's the macro's hotkey — pick a different key.";
        return;
      }

      const clash = settings.profiles.find(
        (p) => p !== profile && p.enabled !== false && p.bindEnabled && p.bindVk === vk,
      );
      if (clash) {
        endCapture();
        bindHint.textContent = `${clash.name} already uses that — pick a different key.`;
        return;
      }
      profile.bindVk = vk;
      endCapture();
      renderConflict();
      renderBind();
      push();
    }
  });

  await listen<[number, number]>("position-captured", (event) => {
    const target = capturing;
    const [x, y] = event.payload;

    if (target?.kind === "pixel") {
      profile.pixelX = x;
      profile.pixelY = y;

      void invoke<number | null>("sample_pixel", { x, y })
        .then((rgb) => {
          if (rgb !== null) profile.pixelRgb = rgb;
          profile.pixelEnabled = true;
          endCapture();
          renderPixel();
          push();
        })
        .catch(() => endCapture());
      return;
    }

    if (target?.kind === "extraPoint") {
      void invoke<[number, number] | null>("to_client_point", {
        title: profile.targetTitle,
        process: profile.targetProcess,
        x,
        y,
      })
        .then((client) => {
          if (client) profile.points.push({ x: client[0], y: client[1] });
          endCapture();
          renderTargetMode();
          push();
        })
        .catch(() => endCapture());
      return;
    }

    if (target?.kind === "clickPoint") {

      void invoke<[number, number] | null>("to_client_point", {
        title: profile.targetTitle,
        process: profile.targetProcess,
        x,
        y,
      })
        .then((client) => {
          if (client) {
            profile.targetX = client[0];
            profile.targetY = client[1];
          }
          endCapture();
          renderTargetMode();
          push();
        })
        .catch(() => endCapture());
      return;
    }

    if (target?.kind === "position") {
      const step = automation.steps[target.index];
      if (step && step.kind === "move") {
        step.x = x;
        step.y = y;
      }
      endCapture();
      pushAutomation();
    }
  });

  await listen<AutomationStatus>("automation-status", (event) => {
    renderAutomationStatus(event.payload);
  });

  await listen<FisherStatus>("fisher-status", (event) => {
    renderFisherStatus(event.payload);
  });

  await listen<GumdropStatus>("gumdrop-status", (event) => {
    renderGumdropStatus(event.payload);
  });

  await listen<CrossbowStatus>("crossbow-status", (event) => {
    renderCrossbowStatus(event.payload);
  });

  await listen<DaveyStatus>("davey-status", (event) => {
    renderDaveyStatus(event.payload);
  });

  await listen<SkywarsStatus>("skywars-status", (event) => {
    renderSkywarsStatus(event.payload);
  });

  await listen<boolean>("guard-tripped", (event) => {
    heroSub.textContent = event.payload
      ? "Held — cursor is near a window edge or button"
      : `Press ${vkLabel(profile.bindVk)} to stop`;
  });

  await listen("bind-cancelled", () => {
    void invoke("cancel_capture");
    endCapture();
  });
}

async function boot(): Promise<void> {
  settings = await invoke<Settings>("get_settings");
  profile = settings.profiles[settings.selected] ?? settings.profiles[0]!;
  automation = await invoke<Automation>("get_automation");
  fisher = await invoke<Fisher>("get_fisher");
  gumdrop = await invoke<Gumdrop>("get_gumdrop");
  skywars = await invoke<Skywars>("get_skywars");
  davey = await invoke<Davey>("get_davey");
  crossbow = await invoke<Crossbow>("get_crossbow");
  overlay = await invoke<Overlay>("get_overlay");
  renderPresets(settings.presets ?? []);

  document.querySelectorAll<HTMLInputElement>("input").forEach(noAutofill);

  document.addEventListener("contextmenu", (event) => {
    const target = event.target as HTMLElement | null;
    if (target?.tagName === "INPUT" || target?.isContentEditable) return;
    event.preventDefault();
  });

  const wake = () => {
    setDormant();
    if (!drawingWorthwhile()) return;

    void invoke<Status>("get_status").then(renderStatus).catch(() => {});
    void invoke<AutomationStatus>("get_automation_status")
      .then(renderAutomationStatus)
      .catch(() => {});
    requestAnimationFrame(drawGraph);
  };

  document.addEventListener("visibilitychange", wake);

  window.addEventListener("focus", () => {
    windowFocused = true;
    wake();
  });

  window.addEventListener("blur", () => {
    windowFocused = false;
    setDormant();
  });

  windowFocused = document.hasFocus();
  setDormant();

  document.querySelectorAll<HTMLElement>(".segmented").forEach((group) => {
    const indicator = document.createElement("span");
    indicator.className = "seg-indicator";
    group.prepend(indicator);
  });

  wireTabs();
  wireWindowButtons();
  wireSegments();
  wireBinds();
  wireCps();
  wireToggles();
  wireActions();
  wireAutomation();
  wireFisher();
  wireGumdrop();
  wireSkywars();
  wireDavey();
  wireCrossbow();
  wireOverlay();
  wireSharing();
  wireResets();
  wireNumberInputs();
  wireOptimize();
  wireSuggest();

  window.addEventListener("resize", rememberWindowSize);
  window.addEventListener("resize", drawGraph);
  rememberWindowSize();

  resetWindowBtn.addEventListener("click", () => {
    void invoke("reset_window_size")
      .then(() => {

        settings.windowWidth = 0;
        settings.windowHeight = 0;
      })
      .catch(() => {});
  });
  wireUpdates();

  renderAll();
  renderAutomation();
  renderFisher();
  renderGumdrop();
  renderSkywars();
  renderDavey();
  renderCrossbow();
  renderOverlay();
  requestAnimationFrame(positionAllSegments);

  void refreshOptimizations();
  renderStatus(await invoke<Status>("get_status"));
  renderAutomationStatus(await invoke<AutomationStatus>("get_automation_status"));

  await wireEvents();

  window.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && capturing) {
      void invoke("cancel_capture");
      endCapture();
    }
  });

  getVersion()
    .then((version) => {
      appVersion.textContent = version;
      creditVersion.textContent = version;
      renderSuggest();
    })
    .catch(() => {
      appVersion.textContent = "unknown";
      creditVersion.textContent = "unknown";
      renderSuggest();
    });

  void checkForUpdate(false);
}

window.addEventListener("DOMContentLoaded", () => {
  void boot();
});
