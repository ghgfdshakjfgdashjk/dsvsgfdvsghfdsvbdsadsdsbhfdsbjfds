import { listen } from "@tauri-apps/api/event";

interface ClickerStatus {
  name: string;
  active: boolean;
  cps: number;
}

interface Status {
  clickers: ClickerStatus[];
  running: number;
}

const pill = document.getElementById("pill") as HTMLElement;
const value = document.getElementById("value") as HTMLElement;

function show(cps: number, live: boolean): void {
  pill.classList.toggle("idle", !live);
  value.textContent = cps >= 100 ? Math.round(cps).toLocaleString() : cps.toFixed(1);
}

void listen<Status>("status", (event) => {
  // whichever clicker is actually going; a guarded one still counts as running
  const running = event.payload.clickers.find((c) => c.active);
  show(running ? running.cps : 0, Boolean(running));
});

show(0, false);
