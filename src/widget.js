/**
 * Broomed Widget — Gesture-Aware Desktop Companion UI & Engine
 * 
 * Frame-based mascot animation with refresh-rate independent playback,
 * directional facing awareness, dynamic velocity tilt, drop-onto-folder detection,
 * shake-to-undo/dismiss, petting wake-up, and side-by-side contextual drawers.
 */
(() => {
  "use strict";

  // ─── Mascot States & Animation Engine ────────────────────────
  const MASCOT_STATES = {
    coffee: {
      name: "Coffee / Active Idle",
      frames: Array.from({ length: 7 }, (_, i) => `assets/mascot/coffee/broomed_coffee_${i + 1}.png`),
      durationMs: 250
    },
    searching: {
      name: "Searching / Thinking",
      frames: Array.from({ length: 7 }, (_, i) => `assets/mascot/searching/broomed_searching_${i + 1}.png`),
      durationMs: 250
    },
    brooming: {
      name: "Brooming / Working",
      frames: Array.from({ length: 14 }, (_, i) => `assets/mascot/brooming/broomed-brooming-${i + 1}.png`),
      durationMs: 250
    },
    sleeping: {
      name: "Sleeping / Deep Idle",
      frames: Array.from({ length: 7 }, (_, i) => `assets/mascot/sleeping/broomed_sleeping_${i + 1}.png`),
      durationMs: 250
    }
  };

  // ─── DOM References ──────────────────────────────────────────
  const $ = (sel) => document.querySelector(sel);
  const widgetRoot           = $("#widget-root");
  const mascotCol            = $("#mascot-col");
  const mascotImg            = $("#mascot-img");
  const mascotRegion         = $("#mascot-region");
  const chatForm             = $("#chat-form");
  const chatInput            = $("#chat-input");
  const chatSend             = $("#chat-send");
  const btnSwitchPlan        = $("#btn-switch-plan");
  const pendingPillCount     = $("#pending-pill-count");
  const approvalCard         = $("#approval-card");
  const approvalBadge        = $("#approval-badge");
  const approvalTitle        = $("#approval-title");
  const approvalSummary      = $("#approval-summary");
  const btnApprove           = $("#btn-approve");
  const btnInspectPlan       = $("#btn-inspect-plan");
  const btnSwitchInput       = $("#btn-switch-input");
  const btnDismissApproval   = $("#btn-dismiss-approval");
  const statusToast          = $("#status-toast");
  const toastText            = $("#toast-text");

  // Context Menu DOM
  const widgetContextMenu    = $("#widget-context-menu");
  const menuAiStatus         = $("#menu-ai-status");
  const menuCleanDownloads   = $("#menu-clean-downloads");
  const menuCleanDesktop     = $("#menu-clean-desktop");
  const menuCleanExplorer    = $("#menu-clean-explorer");
  const menuPickFolder       = $("#menu-pick-folder");
  const menuInspectCurrentPlan = $("#menu-inspect-current-plan");
  const menuUndoLast         = $("#menu-undo-last");
  const menuSettings         = $("#menu-settings");
  const menuToggleSleep      = $("#menu-toggle-sleep");
  const menuSleepLabel       = $("#menu-sleep-label");
  const menuHideWidget       = $("#menu-hide-widget");
  const menuQuitApp          = $("#menu-quit-app");

  // Plan Inspector Drawer DOM
  const widgetPlanDrawer     = $("#widget-plan-drawer");
  const drawerCountBadge     = $("#drawer-count-badge");
  const drawerTargetPath     = $("#drawer-target-path");
  const btnCloseDrawer       = $("#btn-close-drawer");
  const drawerCatBar         = $("#drawer-cat-bar");
  const drawerCheckAll       = $("#drawer-check-all");
  const drawerSelectionSummary = $("#drawer-selection-summary");
  const drawerOpsList        = $("#drawer-ops-list");
  const btnDrawerExecute     = $("#btn-drawer-execute");
  const btnDrawerRescan      = $("#btn-drawer-rescan");
  const btnDrawerDismiss     = $("#btn-drawer-dismiss");

  // Settings Flyout DOM
  const widgetSettingsFlyout = $("#widget-settings-flyout");
  const btnCloseSettings     = $("#btn-close-settings");
  const settingsActiveTierPill = $("#settings-active-tier-pill");
  const settingsTierDesc     = $("#settings-tier-desc");
  const settingsByokStatus   = $("#settings-byok-status");
  const settingsByokProvider = $("#settings-byok-provider");
  const settingsByokModel    = $("#settings-byok-model");
  const settingsByokKey      = $("#settings-byok-key");
  const settingsByokUrl      = $("#settings-byok-url");
  const settingsByokUrlRow   = $("#settings-byok-url-row");
  const btnSaveByok          = $("#btn-save-byok");
  const btnClearByok         = $("#btn-clear-byok");
  const settingsByokMsg      = $("#settings-byok-msg");
  const settingsActivationInput = $("#settings-activation-input");
  const btnActivateLicense   = $("#btn-activate-license");
  const settingsActivationMsg = $("#settings-activation-msg");
  const settingsDeviceId     = $("#settings-device-id");
  const btnSettingsRefresh   = $("#btn-settings-refresh");
  const btnSettingsDone      = $("#btn-settings-done");

  // ─── Tauri Bridge ────────────────────────────────────────────
  function getInvoke() {
    if (typeof window.__TAURI__ !== "undefined" && window.__TAURI__.core && window.__TAURI__.core.invoke) {
      return window.__TAURI__.core.invoke;
    }
    if (typeof window.__TAURI_INTERNALS__ !== "undefined" && window.__TAURI_INTERNALS__.invoke) {
      return window.__TAURI_INTERNALS__.invoke;
    }
    if (typeof window.__TAURI__ !== "undefined" && window.__TAURI__.invoke) {
      return window.__TAURI__.invoke;
    }
    return null;
  }
  let invoke = getInvoke();
  if (!invoke) {
    let pollCount = 0;
    const poll = setInterval(() => {
      invoke = getInvoke();
      if (invoke || pollCount++ > 20) clearInterval(poll);
    }, 100);
  }

  function getEventApi() {
    if (typeof window.__TAURI__ !== "undefined" && window.__TAURI__.event) return window.__TAURI__.event;
    if (typeof window.__TAURI_INTERNALS__ !== "undefined" && window.__TAURI_INTERNALS__.event) return window.__TAURI_INTERNALS__.event;
    return null;
  }

  // ─── State ───────────────────────────────────────────────────
  const stateFramesCache = {};
  let currentMascotState = "coffee";
  let frames        = [];
  let currentFrame  = 0;
  let elapsed       = 0;
  let lastTimestamp = null;
  let reducedMotion = false;
  let appState      = "idle"; // idle | scanning | classifying | executing | error

  // Active view: "closed" | "input" | "approval" | "context-menu" | "drawer" | "settings"
  let activeFlyoutMode = "closed"; 
  let lastDockSide = "right";
  let lastXOffset = 0;

  // Plan & Selection State
  const STORAGE_KEY_PLAN    = "broomed_pending_plan";
  const STORAGE_KEY_HISTORY = "broomed_prompt_history";

  let pendingApproval = null;
  let selectedIndices = new Set();
  let activeCategoryFilter = "all";

  try {
    const savedPlan = sessionStorage.getItem(STORAGE_KEY_PLAN);
    if (savedPlan) {
      pendingApproval = JSON.parse(savedPlan);
      selectedIndices = new Set(pendingApproval.previews.map((_, i) => i));
    }
  } catch {}

  let promptHistory = [];
  let historyIndex = -1;
  try {
    const savedHist = localStorage.getItem(STORAGE_KEY_HISTORY);
    if (savedHist) promptHistory = JSON.parse(savedHist);
  } catch {}

  // ─── Direction & Gesture Motion Engine ───────────────────────
  let facingDirection = "right"; // "right" | "left"
  let currentTiltAngle = 0;
  let lastMouseX = null, lastMouseY = null, lastMouseTime = null;
  let dragVelocitySamples = []; // [{ time, dx, vx }]
  let isDraggingMascot = false;
  let dragStartedAt = 0;

  // Petting detector state
  let petStrokeCount = 0;
  let lastPetDirection = 0;
  let lastPetTime = 0;

  function updateFacingDirection(dir) {
    if (facingDirection === dir) return;
    facingDirection = dir;
    if (dir === "left") {
      mascotImg.classList.remove("facing-right");
      mascotImg.classList.add("facing-left");
    } else {
      mascotImg.classList.remove("facing-left");
      mascotImg.classList.add("facing-right");
    }
  }

  function setTiltAngle(deg) {
    currentTiltAngle = Math.max(-10, Math.min(10, deg));
    mascotImg.style.setProperty("--tilt-angle", `${currentTiltAngle.toFixed(1)}deg`);
  }

  function dampTiltAngle() {
    if (Math.abs(currentTiltAngle) > 0.1) {
      currentTiltAngle *= 0.75;
      mascotImg.style.setProperty("--tilt-angle", `${currentTiltAngle.toFixed(1)}deg`);
      requestAnimationFrame(dampTiltAngle);
    } else {
      currentTiltAngle = 0;
      mascotImg.style.setProperty("--tilt-angle", "0deg");
    }
  }

  function triggerShakeEffect() {
    mascotImg.classList.add("shaking");
    setTimeout(() => {
      mascotImg.classList.remove("shaking");
    }, 600);
  }

  function triggerPettingReaction() {
    // Only allow petting reaction when idle or sleeping, with no open drawers or menus
    if (appState !== "idle" && currentMascotState !== "sleeping") return;
    if (activeFlyoutMode !== "closed" && activeFlyoutMode !== "input") return;

    mascotImg.classList.add("petting-bounce");
    if (currentMascotState === "sleeping") {
      setMascotState("coffee");
      resetActivityTimer();
      showToast("☕ Woke up with a smile!");
    }
    setTimeout(() => {
      mascotImg.classList.remove("petting-bounce");
    }, 500);
  }

  // ─── Preload Frames & Animation Loop ─────────────────────────
  function preloadFrames() {
    Object.keys(MASCOT_STATES).forEach((stateKey) => {
      const stateDef = MASCOT_STATES[stateKey];
      stateFramesCache[stateKey] = stateDef.frames.map((src) => {
        const img = new Image();
        img.src = src;
        img.onerror = () => {
          if (src.endsWith(".png")) {
            img.src = src.replace(/\.png$/, ".svg").replace(/_/g, "-");
          }
        };
        return img;
      });
    });

    setMascotState("coffee");
    updateFacingDirection("right");
    startAnimationLoop();
  }

  function setMascotState(stateKey) {
    if (!MASCOT_STATES[stateKey]) return;
    if (currentMascotState === stateKey && frames.length > 0) return;

    currentMascotState = stateKey;
    frames = stateFramesCache[stateKey] || [];
    currentFrame = 0;
    elapsed = 0;
    if (frames.length > 0 && frames[0].src) {
      mascotImg.src = frames[0].src;
    }
  }
  window.setMascotState = setMascotState;

  function startAnimationLoop() {
    lastTimestamp = performance.now();
    requestAnimationFrame(tick);
  }

  function tick(timestamp) {
    requestAnimationFrame(tick);
    if (lastTimestamp === null) {
      lastTimestamp = timestamp;
      return;
    }

    const delta = timestamp - lastTimestamp;
    lastTimestamp = timestamp;

    if (reducedMotion) return;
    if (delta > 250) return;

    const stateDef = MASCOT_STATES[currentMascotState] || MASCOT_STATES.coffee;
    const frameDuration = stateDef.durationMs || 180;
    const frameCount = frames.length;

    if (frameCount <= 1) {
      if (frames[0] && mascotImg.src !== frames[0].src) {
        mascotImg.src = frames[0].src;
      }
      return;
    }

    elapsed += delta;
    if (elapsed >= frameDuration) {
      elapsed -= frameDuration;
      if (elapsed > frameDuration) elapsed = 0;

      currentFrame = (currentFrame + 1) % frameCount;
      if (frames[currentFrame]) {
        mascotImg.src = frames[currentFrame].src;
      }
    }
  }

  function checkReducedMotion() {
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    reducedMotion = mq.matches;
    mq.addEventListener("change", (e) => {
      reducedMotion = e.matches;
      if (reducedMotion) {
        mascotImg.src = frames[0]?.src || mascotImg.src;
      }
    });
  }

  // ─── Dynamic Window Dimension & Screen Docking Helper ────────
  async function setWidgetDimensions(width, height, xOffset = 0) {
    const inv = getInvoke() || invoke;
    if (inv) {
      await inv("resize_widget_cmd", {
        width: width || 200.0,
        height: height || 206.0,
        xOffset: xOffset || 0.0,
      }).catch(() => {});
    }
  }

  function computeDocking(requiredWidth = 260) {
    const screenW = typeof window !== "undefined" && window.screen ? (window.screen.availWidth || 1920) : 1920;
    const currentWinX = typeof window !== "undefined" ? (window.screenX || window.screenLeft || 0) : 0;
    const spaceOnRight = screenW - (currentWinX + 200);

    if (spaceOnRight < requiredWidth + 10 && currentWinX > requiredWidth + 10) {
      return { side: "left", xOffset: -requiredWidth };
    }
    return { side: "right", xOffset: 0.0 };
  }

  function applyDockingClasses(el, side) {
    if (!el) return;
    if (side === "left") {
      el.classList.remove("pos-right");
      el.classList.add("pos-left");
      widgetRoot.classList.remove("dock-right");
      widgetRoot.classList.add("dock-left");
    } else {
      el.classList.remove("pos-left");
      el.classList.add("pos-right");
      widgetRoot.classList.remove("dock-left");
      widgetRoot.classList.add("dock-right");
    }
  }

  // ─── Toast Notifications ─────────────────────────────────────
  let toastTimer = null;
  function showToast(text, durationMs = 2400) {
    if (!toastText || !statusToast) return;
    toastText.textContent = text;
    statusToast.classList.remove("hidden");
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      statusToast.classList.add("hidden");
    }, durationMs);
  }

  // ─── Plan & History Persistence ──────────────────────────────
  function savePendingPlan(plan) {
    pendingApproval = plan;
    if (plan && plan.previews) {
      selectedIndices = new Set(plan.previews.map((_, i) => i));
    } else {
      selectedIndices.clear();
    }
    try {
      if (plan) sessionStorage.setItem(STORAGE_KEY_PLAN, JSON.stringify(plan));
      else sessionStorage.removeItem(STORAGE_KEY_PLAN);
    } catch {}
    updatePendingBadge();
  }

  function addPromptToHistory(txt) {
    if (!txt) return;
    promptHistory = promptHistory.filter((p) => p !== txt);
    promptHistory.unshift(txt);
    if (promptHistory.length > 50) promptHistory.pop();
    try { localStorage.setItem(STORAGE_KEY_HISTORY, JSON.stringify(promptHistory)); } catch {}
    historyIndex = -1;
  }

  function updatePendingBadge() {
    const hasPlan = pendingApproval && pendingApproval.previews && pendingApproval.previews.length > 0;
    if (btnSwitchPlan && pendingPillCount) {
      if (hasPlan) {
        pendingPillCount.textContent = pendingApproval.previews.length;
        btnSwitchPlan.classList.remove("hidden");
      } else {
        btnSwitchPlan.classList.add("hidden");
      }
    }
    if (menuInspectCurrentPlan) {
      menuInspectCurrentPlan.classList.toggle("hidden", !hasPlan);
    }
  }

  // ─── Helpers: File Icons & Escaping ──────────────────────────
  function esc(s) {
    return String(s || "").replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }

  function getFileIcon(filename, category) {
    const ext = (filename.split(".").pop() || "").toLowerCase();
    if (["png", "jpg", "jpeg", "webp", "gif", "svg", "bmp"].includes(ext) || category === "Images") {
      return `<svg class="svg-icon text-accent" viewBox="0 0 24 24"><rect width="18" height="18" x="3" y="3" rx="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"/></svg>`;
    }
    if (["zip", "tar", "gz", "7z", "rar", "bz2"].includes(ext) || category === "Archives") {
      return `<svg class="svg-icon text-muted" viewBox="0 0 24 24"><rect width="20" height="5" x="2" y="3" rx="1"/><path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8"/><path d="M10 12h4"/></svg>`;
    }
    if (["rs", "js", "ts", "py", "go", "cpp", "c", "html", "css", "json", "toml"].includes(ext) || category === "Code") {
      return `<svg class="svg-icon text-muted" viewBox="0 0 24 24"><polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg>`;
    }
    if (["mp3", "wav", "flac", "m4a", "ogg"].includes(ext) || category === "Audio") {
      return `<svg class="svg-icon text-muted" viewBox="0 0 24 24"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>`;
    }
    return `<svg class="svg-icon text-muted" viewBox="0 0 24 24"><path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"/><polyline points="14 2 14 8 20 8"/></svg>`;
  }

  // ─── Status & Mascot Synchronizer ───────────────────────────
  function setStatus(text, mode = "idle") {
    appState = mode;
    if (mode === "scanning" || mode === "executing") {
      setMascotState("brooming");
    } else if (mode === "classifying" || mode === "thinking" || mode === "planning") {
      setMascotState("searching");
    } else if (mode === "sleeping") {
      setMascotState("sleeping");
    } else {
      setMascotState("coffee");
      resetActivityTimer();
    }
  }

  // ─── Close All Flyouts / Reset Window ────────────────────────
  async function closeAllFlyouts() {
    activeFlyoutMode = "closed";
    widgetContextMenu?.classList.add("hidden");
    widgetPlanDrawer?.classList.add("hidden");
    widgetSettingsFlyout?.classList.add("hidden");
    approvalCard?.classList.add("hidden");
    chatForm?.classList.remove("active");
    widgetRoot.classList.remove("dock-left", "dock-right");

    const restoreX = lastXOffset !== 0 ? -lastXOffset : 0;
    lastXOffset = 0;
    await setWidgetDimensions(200.0, 206.0, restoreX);
  }

  // ─── 1. Input Bar Mode ───────────────────────────────────────
  async function openInputBar() {
    activeFlyoutMode = "input";
    widgetContextMenu?.classList.add("hidden");
    widgetPlanDrawer?.classList.add("hidden");
    widgetSettingsFlyout?.classList.add("hidden");
    approvalCard?.classList.add("hidden");

    updatePendingBadge();
    chatForm.classList.add("active");
    await setWidgetDimensions(200.0, 240.0, 0);
    setTimeout(() => chatInput.focus(), 150);
  }

  // ─── 2. Mini Approval Mode ───────────────────────────────────
  async function showApprovalCard(folderPath, previews, summaryText) {
    savePendingPlan({ folderPath, previews, summaryText });
    activeFlyoutMode = "approval";
    widgetContextMenu?.classList.add("hidden");
    widgetPlanDrawer?.classList.add("hidden");
    widgetSettingsFlyout?.classList.add("hidden");
    chatForm.classList.remove("active");

    approvalBadge.textContent = previews.length + " ops";
    approvalTitle.textContent = "Plan Ready";
    approvalSummary.textContent = summaryText || ("Move " + previews.length + " files into categorized folders?");

    btnApprove.disabled = false;
    btnApprove.textContent = "✓ Organize";
    btnInspectPlan.disabled = false;
    btnInspectPlan.textContent = "↗ Inspect";

    approvalCard.classList.remove("hidden");
    await setWidgetDimensions(200.0, 310.0, 0);
  }

  // ─── 3. Context Menu Mode ────────────────────────────────────
  async function showContextMenu() {
    activeFlyoutMode = "context-menu";
    chatForm.classList.remove("active");
    approvalCard.classList.add("hidden");
    widgetPlanDrawer.classList.add("hidden");
    widgetSettingsFlyout.classList.add("hidden");

    if (menuSleepLabel) {
      menuSleepLabel.textContent = currentMascotState === "sleeping" ? "Wake Up" : "Put to Sleep";
    }

    updatePendingBadge();

    const inv = getInvoke() || invoke;
    if (inv && menuAiStatus) {
      inv("get_active_ai_status_cmd").then((s) => {
        if (s && s.label) menuAiStatus.textContent = s.label.split(" ")[0] || "Local AI";
      }).catch(() => {});
    }

    const { side, xOffset } = computeDocking(200);
    lastDockSide = side;
    lastXOffset = xOffset;
    applyDockingClasses(widgetContextMenu, side);

    widgetContextMenu.classList.remove("hidden");
    await setWidgetDimensions(400.0, 340.0, xOffset);
  }

  // ─── 4. Plan Inspector Drawer Mode ───────────────────────────
  function renderPlanDrawerRows() {
    if (!drawerOpsList || !pendingApproval) return;
    drawerOpsList.innerHTML = "";

    const previews = pendingApproval.previews || [];
    const frag = document.createDocumentFragment();

    const filtered = previews.filter((p) => {
      if (activeCategoryFilter === "all") return true;
      const cat = (p.ai_result?.category || "General").toLowerCase();
      return cat === activeCategoryFilter;
    });

    filtered.forEach((p) => {
      const originalIdx = previews.indexOf(p);
      const isSelected = selectedIndices.has(originalIdx);
      const src = p.operation?.source || "";
      const dst = p.operation?.destination || "";
      const cat = p.ai_result?.category || "General";
      const conf = p.operation?.confidence ?? p.ai_result?.confidence ?? 0.5;
      const reason = p.operation?.reason || p.ai_result?.reason || "Semantic grouping";
      const fileName = src.split(/[/\\]/).pop() || src;
      const dstName = dst.split(/[/\\]/).slice(-2).join("/");

      const pct = Math.round(conf * 100);
      const confClass = pct >= 80 ? "high" : pct >= 55 ? "med" : "low";

      const item = document.createElement("div");
      item.className = "drawer-op-item";
      item.innerHTML = `
        <input type="checkbox" class="drawer-checkbox op-check" data-idx="${originalIdx}" ${isSelected ? "checked" : ""}>
        <div class="op-info">
          <div class="op-file-row">
            <span class="op-filename" title="${esc(src)}">${esc(fileName)}</span>
            <span class="op-cat-badge">${esc(cat)}</span>
          </div>
          <div class="op-dest-row mono" title="${esc(dst)}">→ ${esc(dstName)}</div>
          <div class="op-meta-row">
            <div class="op-conf-bar" title="Confidence: ${pct}%">
              <div class="op-conf-fill ${confClass}" style="width: ${pct}%"></div>
            </div>
            <span class="op-reason" title="${esc(reason)}">${esc(reason)}</span>
          </div>
        </div>
      `;

      item.querySelector(".op-check")?.addEventListener("change", (e) => {
        if (e.target.checked) selectedIndices.add(originalIdx);
        else selectedIndices.delete(originalIdx);
        updateDrawerSelectionState();
      });

      frag.appendChild(item);
    });

    drawerOpsList.appendChild(frag);
  }

  function renderDrawerCategoryFilters() {
    if (!drawerCatBar || !pendingApproval) return;
    drawerCatBar.innerHTML = "";

    const previews = pendingApproval.previews || [];
    const counts = { all: previews.length };
    previews.forEach((p) => {
      const cat = p.ai_result?.category || "General";
      counts[cat] = (counts[cat] || 0) + 1;
    });

    Object.keys(counts).forEach((cat) => {
      const chip = document.createElement("button");
      chip.type = "button";
      chip.className = `cat-chip ${activeCategoryFilter === cat.toLowerCase() ? "active" : ""}`;
      chip.textContent = `${cat === "all" ? "All" : cat} (${counts[cat]})`;
      chip.addEventListener("click", () => {
        activeCategoryFilter = cat.toLowerCase();
        renderDrawerCategoryFilters();
        renderPlanDrawerRows();
      });
      drawerCatBar.appendChild(chip);
    });
  }

  function updateDrawerSelectionState() {
    if (!pendingApproval) return;
    const total = pendingApproval.previews?.length || 0;
    const count = selectedIndices.size;

    if (drawerSelectionSummary) {
      drawerSelectionSummary.textContent = `${count} of ${total} selected`;
    }

    if (drawerCheckAll) {
      if (count === total && total > 0) {
        drawerCheckAll.checked = true;
        drawerCheckAll.indeterminate = false;
      } else if (count === 0) {
        drawerCheckAll.checked = false;
        drawerCheckAll.indeterminate = false;
      } else {
        drawerCheckAll.checked = false;
        drawerCheckAll.indeterminate = true;
      }
    }

    if (btnDrawerExecute) {
      btnDrawerExecute.textContent = `✓ Execute (${count})`;
      btnDrawerExecute.disabled = count === 0;
    }
  }

  async function openPlanDrawer() {
    if (!pendingApproval || !pendingApproval.previews || pendingApproval.previews.length === 0) {
      showToast("No active plan to inspect");
      return;
    }

    activeFlyoutMode = "drawer";
    widgetContextMenu.classList.add("hidden");
    widgetSettingsFlyout.classList.add("hidden");
    chatForm.classList.remove("active");
    approvalCard.classList.add("hidden");

    if (drawerCountBadge) drawerCountBadge.textContent = `${pendingApproval.previews.length} ops`;
    if (drawerTargetPath) drawerTargetPath.textContent = pendingApproval.folderPath || "";

    activeCategoryFilter = "all";
    renderDrawerCategoryFilters();
    renderPlanDrawerRows();
    updateDrawerSelectionState();

    const { side, xOffset } = computeDocking(260);
    lastDockSide = side;
    lastXOffset = xOffset;
    applyDockingClasses(widgetPlanDrawer, side);

    widgetPlanDrawer.classList.remove("hidden");
    await setWidgetDimensions(460.0, 420.0, xOffset);
  }

  // ─── 5. Settings Flyout Mode ─────────────────────────────────
  async function openSettingsFlyout() {
    activeFlyoutMode = "settings";
    widgetContextMenu.classList.add("hidden");
    widgetPlanDrawer.classList.add("hidden");
    chatForm.classList.remove("active");
    approvalCard.classList.add("hidden");

    const inv = getInvoke() || invoke;
    if (inv) {
      try {
        const s = await inv("get_active_ai_status_cmd");
        if (s && settingsActiveTierPill) {
          settingsActiveTierPill.textContent = s.label;
          settingsTierDesc.textContent = s.details;
        }
      } catch {}

      try {
        const byok = await inv("get_byok_config_cmd");
        if (byok) {
          if (settingsByokProvider) settingsByokProvider.value = byok.provider || "openai";
          if (settingsByokModel) settingsByokModel.value = byok.model || "";
          if (settingsByokUrl) settingsByokUrl.value = byok.base_url || "";
          if (settingsByokStatus) {
            if (byok.has_key) {
              settingsByokStatus.textContent = `Active (${byok.provider})`;
              settingsByokStatus.classList.add("active");
            } else {
              settingsByokStatus.textContent = "Not Configured";
              settingsByokStatus.classList.remove("active");
            }
          }
        }
      } catch {}

      try {
        const devRaw = await inv("get_device_info_cmd");
        const dev = typeof devRaw === "string" ? JSON.parse(devRaw) : devRaw;
        if (dev && settingsDeviceId) {
          settingsDeviceId.textContent = dev.device_id || "Registered Local Device";
        }
      } catch {}
    } else {
      if (settingsDeviceId) settingsDeviceId.textContent = "dev_preview_device_001";
    }

    const { side, xOffset } = computeDocking(260);
    lastDockSide = side;
    lastXOffset = xOffset;
    applyDockingClasses(widgetSettingsFlyout, side);

    widgetSettingsFlyout.classList.remove("hidden");
    await setWidgetDimensions(460.0, 460.0, xOffset);
  }

  // ─── Plan Execution & Core Workflow ──────────────────────────
  async function executePlanDirectly(previews) {
    const inv = getInvoke() || invoke;
    if (!inv) {
      await delay(600);
      return previews.map((_, i) => String(i));
    }
    return await inv("execute_plan_cmd", { previews, dbPath: null });
  }

  async function scanAndPlanFolder(folderPath, instruction = null) {
    if (!folderPath) return;
    setStatus("Thinking…", "thinking");
    showToast(`Scanning: ${folderPath.split(/[/\\]/).pop() || folderPath}…`);

    const inv = getInvoke() || invoke;
    if (!inv) {
      await delay(800);
      const demoPreviews = [
        { operation: { source: `${folderPath}/Tax_Invoice_2026.pdf`, destination: `${folderPath}/Documents/Tax_Invoice_2026.pdf`, confidence: 0.95, reason: "Financial invoice match", id: "1" }, ai_result: { category: "Documents" } },
        { operation: { source: `${folderPath}/Screenshot_42.png`, destination: `${folderPath}/Images/Screenshot_42.png`, confidence: 0.89, reason: "Visual screenshot", id: "2" }, ai_result: { category: "Images" } },
        { operation: { source: `${folderPath}/project_archive.zip`, destination: `${folderPath}/Archives/project_archive.zip`, confidence: 0.92, reason: "Compressed archive", id: "3" }, ai_result: { category: "Archives" } },
      ];
      await showApprovalCard(folderPath, demoPreviews, `Organize ${demoPreviews.length} files in ${folderPath.split(/[/\\]/).pop() || "folder"}?`);
      setStatus("Ready", "idle");
      return;
    }

    try {
      setStatus("Scanning files…", "scanning");
      const files = await inv("scan_directory_cmd", { base: folderPath, maxFiles: 10000 });

      if (!files || files.length === 0) {
        showToast("No files found or folder clean");
        setStatus("Ready", "idle");
        return;
      }

      setStatus(`Classifying ${files.length} files…`, "classifying");
      showToast(`Classifying ${files.length} files…`);

      let previews = [];
      try {
        previews = await inv("plan_organize_cmd", {
          files,
          base: folderPath,
          task: instruction || "ClassifyFile",
          threshold: 0.5,
          provider: null,
        });
      } catch (e) {
        console.warn("plan_organize failed, fallback:", e);
      }

      if (!previews || previews.length === 0) {
        const results = [];
        for (const f of files) {
          try {
            const promptInput = instruction ? `${f} (Instruction: ${instruction})` : f;
            const r = await inv("classify_cmd", { task: "ClassifyFile", input: promptInput, provider: null });
            results.push({ file: f.split(/[/\\]/).pop() || f, path: f, category: r.category, folder: r.suggested_folder || r.category, confidence: r.confidence, reason: r.reason });
          } catch {
            results.push({ file: f.split(/[/\\]/).pop() || f, path: f, category: "General", folder: "General", confidence: 0.5, reason: "classification fallback" });
          }
        }
        const genId = () => (typeof crypto !== "undefined" && crypto.randomUUID ? crypto.randomUUID() : "00000000-0000-4000-a000-000000000000".replace(/0/g, () => Math.floor(Math.random()*16).toString(16)));
        previews = results.map((r) => ({
          operation: { source: r.path, destination: `${folderPath}/${r.folder}/${r.file}`, kind: "Move", reason: r.reason, confidence: r.confidence, reversible: true, status: "planned", id: genId() },
          ai_result: { category: r.category, confidence: r.confidence, suggested_folder: r.folder, reason: r.reason, tags: [], subcategory: null, suggested_name: null }
        }));
      }

      const cats = new Set(previews.map((p) => p.ai_result?.category || "General"));
      const summary = `${previews.length} files into ${cats.size} categories in ${folderPath.split(/[/\\]/).pop() || "folder"}?`;

      await showApprovalCard(folderPath, previews, summary);
      setStatus("Ready", "idle");
    } catch (err) {
      showToast(`Scan error: ${err}`);
      setStatus("Error", "error");
    }
  }

  // ─── Gesture: Hover Gaze & Petting Reaction ──────────────────
  mascotRegion?.addEventListener("mousemove", (e) => {
    const rect = mascotRegion.getBoundingClientRect();
    const relX = e.clientX - rect.left;
    const midX = rect.width / 2;

    // Gaze tracking (without dragging)
    if (!isDraggingMascot) {
      if (relX < midX - 8) {
        updateFacingDirection("left");
      } else if (relX > midX + 8) {
        updateFacingDirection("right");
      }
    }

    // Petting gesture detection (gentle sweeping strokes — ONLY during idle or sleeping)
    const canPet = (currentMascotState === "sleeping" || (appState === "idle" && activeFlyoutMode === "closed"));
    if (!canPet || isDraggingMascot) {
      petStrokeCount = 0;
      return;
    }

    const now = performance.now();
    const currentDir = relX > midX ? 1 : -1;
    if (lastPetDirection !== 0 && currentDir !== lastPetDirection && (now - lastPetTime < 350)) {
      petStrokeCount++;
      if (petStrokeCount >= 3) {
        triggerPettingReaction();
        petStrokeCount = 0;
      }
    } else if (now - lastPetTime > 600) {
      petStrokeCount = 0;
    }
    lastPetDirection = currentDir;
    lastPetTime = now;
  });

  // ─── Active Drag Tracker (60 FPS Hardware Position Sampling & Target Overlay) ───
  let activeDragInterval = null;
  let lastDragSampleX = null;
  let lastDragSampleTime = null;
  let activeTargetWindow = null;
  let lastOverlayHwnd = null;
  let lastTargetTrackTime = 0;

  function startActiveDragTracker() {
    if (activeDragInterval) clearInterval(activeDragInterval);
    lastDragSampleX = null;
    lastDragSampleTime = performance.now();
    activeTargetWindow = null;
    lastOverlayHwnd = null;
    lastTargetTrackTime = 0;

    activeDragInterval = setInterval(async () => {
      if (!isDraggingMascot) {
        stopActiveDragTracker();
        return;
      }

      const inv = getInvoke() || invoke;
      let curX = null;
      let curY = null;
      let isMouseDown = true;

      if (inv) {
        try {
          const pos = await inv("get_cursor_position_cmd");
          if (pos) {
            curX = pos[0];
            curY = pos[1];
            isMouseDown = pos[2];
          }
        } catch {}
      }

      // If left button was released while dragging, trigger drop resolution immediately
      if (!isMouseDown) {
        stopActiveDragTracker();
        handleMascotMouseUp({ screenX: curX || 0, screenY: curY || 0 });
        return;
      }

      if (curX === null) {
        curX = typeof window !== "undefined" ? (window.screenX || window.screenLeft || 0) : 0;
      }

      const now = performance.now();
      if (lastDragSampleX !== null && lastDragSampleTime !== null) {
        const dt = Math.max(1, now - lastDragSampleTime);
        const dx = curX - lastDragSampleX;
        const vx = dx / dt; // px / ms

        if (Math.abs(dx) >= 2) {
          // 1. Active Real-time Directional Awareness
          if (dx > 0) {
            updateFacingDirection("right");
          } else {
            updateFacingDirection("left");
          }

          // 2. Active Inertia Tilt
          const tilt = Math.max(-8, Math.min(8, vx * 5.0));
          setTiltAngle(tilt);

          // 3. Shake Detection
          dragVelocitySamples.push({ time: now, dx, vx });
          if (dragVelocitySamples.length > 25) dragVelocitySamples.shift();

          const recentSamples = dragVelocitySamples.filter((s) => now - s.time < 700);
          let directionChanges = 0;
          let prevSign = 0;
          for (const sample of recentSamples) {
            const sign = Math.sign(sample.dx);
            if (sign !== 0 && prevSign !== 0 && sign !== prevSign) {
              directionChanges++;
            }
            if (sign !== 0) prevSign = sign;
          }

          if (directionChanges >= 3 && Math.abs(vx) > 0.6) {
            dragVelocitySamples = [];
            triggerShakeEffect();
            if (pendingApproval) {
              savePendingPlan(null);
              closeAllFlyouts();
              showToast("💫 Plan dismissed via shake");
            } else if (inv) {
              inv("undo_last_cmd", { count: 1, dbPath: null }).then(() => {
                showToast("💫 Shake: Undoing last move");
              }).catch(() => {});
            }
          }
        }
      }

      // 4. Real-Time Target Window Detection & Flowing Border Overlay
      if (inv && curX !== null && curY !== null && (now - lastTargetTrackTime > 35)) {
        lastTargetTrackTime = now;
        try {
          const target = await inv("track_target_window_cmd", { x: curX, y: curY });
          if (target) {
            activeTargetWindow = target;
            if (target.hwnd !== lastOverlayHwnd) {
              lastOverlayHwnd = target.hwnd;
              inv("show_target_overlay_cmd", {
                x: target.x,
                y: target.y,
                width: target.width,
                height: target.height,
                folderName: target.folder_name,
              }).catch(() => {});
            }
          } else {
            if (lastOverlayHwnd !== null) {
              lastOverlayHwnd = null;
              activeTargetWindow = null;
              inv("hide_target_overlay_cmd", { confirmed: false }).catch(() => {});
            }
          }
        } catch {}
      }

      lastDragSampleX = curX;
      lastDragSampleTime = now;
    }, 16);
  }

  function stopActiveDragTracker() {
    if (activeDragInterval) {
      clearInterval(activeDragInterval);
      activeDragInterval = null;
    }
    isDraggingMascot = false;
    mascotRegion?.classList.remove("dragging");
    dampTiltAngle();
  }

  // ─── Gesture: Drag, Direction Velocity, Shake & Drop Resolver ─
  let dragStartX = 0, dragStartY = 0;
  const DRAG_THRESHOLD = 5;

  function handleMascotMouseDown(e) {
    if (e.button === 0) {
      dragStartX = e.screenX;
      dragStartY = e.screenY;
      lastMouseX = e.screenX;
      lastMouseY = e.screenY;
      lastMouseTime = performance.now();
      dragStartedAt = performance.now();
      isDraggingMascot = false;
      dragVelocitySamples = [];

      if (activeFlyoutMode !== "closed" && activeFlyoutMode !== "input" && activeFlyoutMode !== "approval") {
        closeAllFlyouts();
        e.stopPropagation();
        return;
      }

      if (currentMascotState === "sleeping") {
        setMascotState("coffee");
        resetActivityTimer();
        e.stopPropagation();
        return;
      }
    }
  }

  function handleMascotMouseMove(e) {
    if (e.buttons === 1) {
      const now = performance.now();
      const dxTotal = Math.abs(e.screenX - dragStartX);
      const dyTotal = Math.abs(e.screenY - dragStartY);

      if (!isDraggingMascot && (dxTotal > DRAG_THRESHOLD || dyTotal > DRAG_THRESHOLD)) {
        isDraggingMascot = true;
        mascotRegion.classList.add("dragging");
        startActiveDragTracker();
        const inv = getInvoke() || invoke;
        if (inv) inv("drag_widget_window_cmd").catch(() => {});
      }

      if (isDraggingMascot && lastMouseX !== null && lastMouseTime !== null) {
        const dt = Math.max(1, now - lastMouseTime);
        const dx = e.screenX - lastMouseX;
        const vx = dx / dt;

        if (dx > 1) updateFacingDirection("right");
        else if (dx < -1) updateFacingDirection("left");

        setTiltAngle(Math.max(-8, Math.min(8, vx * 4.5)));
      }

      lastMouseX = e.screenX;
      lastMouseY = e.screenY;
      lastMouseTime = now;
    }
  }

  async function handleMascotMouseUp(e) {
    stopActiveDragTracker();

    if (dragStartedAt > 0 && performance.now() - dragStartedAt > 120) {
      const inv = getInvoke() || invoke;

      // ── Gesture: Drop-into-Folder Window Resolver with Live Flowing Target Overlay ──
      if (inv) {
        if (activeTargetWindow) {
          const target = activeTargetWindow;
          activeTargetWindow = null;
          lastOverlayHwnd = null;

          // Trigger drop confirmation pulse on target overlay
          inv("hide_target_overlay_cmd", { confirmed: true }).catch(() => {});

          mascotImg.classList.add("target-glow");
          setTimeout(() => mascotImg.classList.remove("target-glow"), 1200);
          showToast(`🎯 Dropped on: ${target.folder_name}`);
          await scanAndPlanFolder(target.path || target.folder_name);
          return;
        } else {
          lastOverlayHwnd = null;
          inv("hide_target_overlay_cmd", { confirmed: false }).catch(() => {});
        }

        try {
          let curX = e.screenX;
          let curY = e.screenY;
          const pos = await inv("get_cursor_position_cmd").catch(() => null);
          if (pos && (pos[0] !== 0 || pos[1] !== 0)) {
            curX = pos[0];
            curY = pos[1];
          }

          const targetFolder = await inv("get_explorer_path_at_point_cmd", { x: curX, y: curY });
          if (targetFolder && targetFolder.trim().length > 0) {
            mascotImg.classList.add("target-glow");
            setTimeout(() => mascotImg.classList.remove("target-glow"), 1200);
            showToast(`🎯 Dropped on: ${targetFolder.split(/[/\\]/).pop() || targetFolder}`);
            await scanAndPlanFolder(targetFolder);
            return;
          }
        } catch (err) {
          console.warn("Drop target resolve error:", err);
        }
      }
    } else {
      const inv = getInvoke() || invoke;
      if (inv) {
        inv("hide_target_overlay_cmd", { confirmed: false }).catch(() => {});
      }
    }
  }

  function handleMascotClick(e) {
    if (e.button !== 0) return;
    if (isDraggingMascot) {
      e.stopPropagation();
      e.preventDefault();
      return;
    }
    e.stopPropagation();

    if (activeFlyoutMode !== "closed" && activeFlyoutMode !== "input" && activeFlyoutMode !== "approval") {
      closeAllFlyouts();
      return;
    }

    if (activeFlyoutMode === "input" || activeFlyoutMode === "approval") {
      closeAllFlyouts();
    } else {
      if (pendingApproval && pendingApproval.previews && pendingApproval.previews.length > 0) {
        showApprovalCard(pendingApproval.folderPath, pendingApproval.previews, pendingApproval.summaryText);
      } else {
        openInputBar();
      }
    }
  }

  mascotImg.addEventListener("mousedown", handleMascotMouseDown);
  mascotRegion.addEventListener("mousedown", handleMascotMouseDown);
  window.addEventListener("mousemove", handleMascotMouseMove);
  window.addEventListener("mouseup", handleMascotMouseUp);
  mascotImg.addEventListener("click", handleMascotClick);
  mascotRegion.addEventListener("click", handleMascotClick);

  // Right-click triggers custom menu
  window.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    e.stopPropagation();
    showContextMenu();
  });

  // ─── Input Form & History Handling ───────────────────────────
  chatInput?.addEventListener("keydown", (e) => {
    if (e.key === "ArrowUp") {
      if (promptHistory.length === 0) return;
      e.preventDefault();
      if (historyIndex < promptHistory.length - 1) historyIndex++;
      chatInput.value = promptHistory[historyIndex] || "";
    } else if (e.key === "ArrowDown") {
      if (promptHistory.length === 0) return;
      e.preventDefault();
      if (historyIndex > 0) {
        historyIndex--;
        chatInput.value = promptHistory[historyIndex] || "";
      } else if (historyIndex === 0) {
        historyIndex = -1;
        chatInput.value = "";
      }
    }
  });

  chatForm?.addEventListener("submit", async (e) => {
    e.preventDefault();
    const text = chatInput?.value?.trim();
    if (!text) return;

    addPromptToHistory(text);
    chatInput.value = "";
    const inv = getInvoke() || invoke;

    if (!inv) {
      await scanAndPlanFolder("C:\\Users\\Demo\\Downloads", text);
      return;
    }

    try {
      setStatus("Thinking…", "thinking");
      showToast("Analyzing request…");
      const intentJson = await inv("parse_intent_cmd", { text });

      let folderPath = null;
      try {
        const parsed = JSON.parse(intentJson);
        folderPath = parsed?.path || parsed?.folder || parsed?.directory || null;
      } catch {
        const match = intentJson.match(/(?:path|folder|directory):\s*"?([^",}\n]+)"?/i);
        if (match) folderPath = match[1].replace(/['"]/g, "");
      }

      if (!folderPath) {
        const lower = text.toLowerCase();
        if (["downloads", "download", "desktop", "documents", "docs"].includes(lower)) {
          const userDl = await inv("get_user_downloads_dir_cmd").catch(() => null);
          if (userDl) {
            if (lower.startsWith("download")) folderPath = userDl;
            else {
              const parent = userDl.replace(/[\\/]Downloads$/i, "");
              folderPath = lower.startsWith("desk") ? `${parent}\\Desktop` : `${parent}\\Documents`;
            }
          }
        }
      }

      if (!folderPath) {
        folderPath = await inv("get_active_explorer_path_cmd").catch(() => null);
        if (!folderPath) folderPath = await inv("get_user_downloads_dir_cmd").catch(() => null);
      }

      if (!folderPath) {
        showToast("Please specify a folder path");
        setStatus("Ready", "idle");
        return;
      }

      await scanAndPlanFolder(folderPath, text);
    } catch (err) {
      showToast(`Error: ${err}`);
      setStatus("Error", "error");
    }
  });

  // Switch between input bar & approval card
  btnSwitchPlan?.addEventListener("click", () => {
    if (pendingApproval) {
      showApprovalCard(pendingApproval.folderPath, pendingApproval.previews, pendingApproval.summaryText);
    }
  });

  btnSwitchInput?.addEventListener("click", () => {
    openInputBar();
  });

  btnDismissApproval?.addEventListener("click", () => {
    savePendingPlan(null);
    closeAllFlyouts();
    showToast("Plan dismissed");
  });

  btnInspectPlan?.addEventListener("click", () => {
    openPlanDrawer();
  });

  btnApprove?.addEventListener("click", async () => {
    if (!pendingApproval || !pendingApproval.previews || pendingApproval.previews.length === 0) return;
    btnApprove.disabled = true;
    btnApprove.textContent = "Organizing…";
    setStatus("Organizing…", "executing");

    try {
      const ids = await executePlanDirectly(pendingApproval.previews);
      const count = Array.isArray(ids) ? ids.length : pendingApproval.previews.length;
      showToast(`✓ Organized ${count} files`);
      savePendingPlan(null);
      await closeAllFlyouts();
      setStatus("Done", "idle");
    } catch (e) {
      showToast(`Execute failed: ${e}`);
      setStatus("Error", "error");
      btnApprove.disabled = false;
      btnApprove.textContent = "✓ Organize";
    }
  });

  // ─── Plan Inspector Drawer Controls ──────────────────────────
  btnCloseDrawer?.addEventListener("click", () => {
    closeAllFlyouts();
  });

  drawerCheckAll?.addEventListener("change", (e) => {
    if (!pendingApproval) return;
    if (e.target.checked) {
      selectedIndices = new Set(pendingApproval.previews.map((_, i) => i));
    } else {
      selectedIndices.clear();
    }
    renderPlanDrawerRows();
    updateDrawerSelectionState();
  });

  btnDrawerExecute?.addEventListener("click", async () => {
    if (!pendingApproval || selectedIndices.size === 0) return;
    const selectedOps = pendingApproval.previews.filter((_, idx) => selectedIndices.has(idx));
    btnDrawerExecute.disabled = true;
    btnDrawerExecute.textContent = "Moving…";
    setStatus("Organizing…", "executing");

    try {
      const ids = await executePlanDirectly(selectedOps);
      const count = Array.isArray(ids) ? ids.length : selectedOps.length;
      showToast(`✓ Organized ${count} files`);
      savePendingPlan(null);
      await closeAllFlyouts();
      setStatus("Done", "idle");
    } catch (e) {
      showToast(`Execute error: ${e}`);
      setStatus("Error", "error");
      btnDrawerExecute.disabled = false;
      btnDrawerExecute.textContent = `✓ Execute (${selectedIndices.size})`;
    }
  });

  btnDrawerRescan?.addEventListener("click", async () => {
    if (pendingApproval && pendingApproval.folderPath) {
      const path = pendingApproval.folderPath;
      await closeAllFlyouts();
      await scanAndPlanFolder(path);
    }
  });

  btnDrawerDismiss?.addEventListener("click", () => {
    savePendingPlan(null);
    closeAllFlyouts();
    showToast("Plan dismissed");
  });

  // ─── Context Menu Action Handlers ────────────────────────────
  menuCleanDownloads?.addEventListener("click", async (e) => {
    e.stopPropagation();
    await closeAllFlyouts();
    const inv = getInvoke() || invoke;
    if (inv) {
      const dl = await inv("get_user_downloads_dir_cmd").catch(() => null);
      if (dl) scanAndPlanFolder(dl);
    } else {
      scanAndPlanFolder("C:\\Users\\Demo\\Downloads");
    }
  });

  menuCleanDesktop?.addEventListener("click", async (e) => {
    e.stopPropagation();
    await closeAllFlyouts();
    const inv = getInvoke() || invoke;
    if (inv) {
      const dl = await inv("get_user_downloads_dir_cmd").catch(() => null);
      if (dl) {
        const desktop = dl.replace(/[\\/]Downloads$/i, "\\Desktop");
        scanAndPlanFolder(desktop);
      }
    } else {
      scanAndPlanFolder("C:\\Users\\Demo\\Desktop");
    }
  });

  menuCleanExplorer?.addEventListener("click", async (e) => {
    e.stopPropagation();
    await closeAllFlyouts();
    const inv = getInvoke() || invoke;
    if (inv) {
      try {
        const active = await inv("get_active_explorer_path_cmd");
        if (active) {
          scanAndPlanFolder(active);
          return;
        }
      } catch {}
    }
    showToast("No active folder window found");
  });

  menuPickFolder?.addEventListener("click", async (e) => {
    e.stopPropagation();
    await closeAllFlyouts();
    const inv = getInvoke() || invoke;
    if (inv) {
      try {
        const folder = await inv("browse_directory_cmd");
        if (folder) scanAndPlanFolder(folder);
      } catch (err) {
        console.warn("browse failed:", err);
      }
    } else {
      const p = prompt("Enter folder to clean:", "C:\\Downloads");
      if (p) scanAndPlanFolder(p);
    }
  });

  menuInspectCurrentPlan?.addEventListener("click", async (e) => {
    e.stopPropagation();
    openPlanDrawer();
  });

  menuUndoLast?.addEventListener("click", async (e) => {
    e.stopPropagation();
    await closeAllFlyouts();
    const inv = getInvoke() || invoke;
    if (inv) {
      try {
        await inv("undo_last_cmd", { count: 1, dbPath: null });
        showToast("✓ Last organization reversed");
      } catch (err) {
        showToast(`Undo error: ${err}`);
      }
    } else {
      showToast("✓ Last move reversed (demo)");
    }
  });

  menuSettings?.addEventListener("click", async (e) => {
    e.stopPropagation();
    openSettingsFlyout();
  });

  menuToggleSleep?.addEventListener("click", async (e) => {
    e.stopPropagation();
    await closeAllFlyouts();
    if (currentMascotState === "sleeping") setMascotState("coffee");
    else setMascotState("sleeping");
  });

  menuHideWidget?.addEventListener("click", async (e) => {
    e.stopPropagation();
    await closeAllFlyouts();
    const inv = getInvoke() || invoke;
    if (inv) inv("hide_widget_window_cmd").catch(() => {});
  });

  menuQuitApp?.addEventListener("click", async (e) => {
    e.stopPropagation();
    const inv = getInvoke() || invoke;
    if (inv) inv("quit_app_cmd").catch(() => {});
  });

  // ─── Settings Flyout Handlers ────────────────────────────────
  btnCloseSettings?.addEventListener("click", () => {
    closeAllFlyouts();
  });

  btnSettingsDone?.addEventListener("click", () => {
    closeAllFlyouts();
  });

  btnSettingsRefresh?.addEventListener("click", () => {
    openSettingsFlyout();
    showToast("Settings refreshed");
  });

  btnSaveByok?.addEventListener("click", async () => {
    const provider = settingsByokProvider?.value || "openai";
    const key = settingsByokKey?.value?.trim();
    const model = settingsByokModel?.value?.trim() || null;
    const url = settingsByokUrl?.value?.trim() || null;

    if (!key) {
      if (settingsByokMsg) {
        settingsByokMsg.className = "form-msg error";
        settingsByokMsg.textContent = "Please enter an API Key.";
      }
      return;
    }

    btnSaveByok.disabled = true;
    if (settingsByokMsg) {
      settingsByokMsg.className = "form-msg";
      settingsByokMsg.textContent = "Saving key securely…";
    }

    const inv = getInvoke() || invoke;
    if (!inv) {
      await delay(400);
      if (settingsByokMsg) {
        settingsByokMsg.className = "form-msg success";
        settingsByokMsg.textContent = "BYOK saved (demo mode)!";
      }
      btnSaveByok.disabled = false;
      return;
    }

    try {
      await inv("save_byok_config_cmd", { provider, apiKey: key, model, baseUrl: url });
      if (settingsByokMsg) {
        settingsByokMsg.className = "form-msg success";
        settingsByokMsg.textContent = "API key saved securely! ✨";
      }
      if (settingsByokKey) settingsByokKey.value = "";
      setTimeout(() => openSettingsFlyout(), 400);
    } catch (e) {
      if (settingsByokMsg) {
        settingsByokMsg.className = "form-msg error";
        settingsByokMsg.textContent = `Save failed: ${e}`;
      }
    } finally {
      btnSaveByok.disabled = false;
    }
  });

  btnClearByok?.addEventListener("click", async () => {
    const inv = getInvoke() || invoke;
    if (inv) {
      try {
        await inv("clear_byok_config_cmd");
        if (settingsByokKey) settingsByokKey.value = "";
        if (settingsByokMsg) {
          settingsByokMsg.className = "form-msg success";
          settingsByokMsg.textContent = "BYOK configuration removed.";
        }
        setTimeout(() => openSettingsFlyout(), 400);
      } catch (e) {
        if (settingsByokMsg) {
          settingsByokMsg.className = "form-msg error";
          settingsByokMsg.textContent = `Remove failed: ${e}`;
        }
      }
    }
  });

  btnActivateLicense?.addEventListener("click", async () => {
    const code = settingsActivationInput?.value?.trim();
    if (!code) return;
    btnActivateLicense.disabled = true;

    const inv = getInvoke() || invoke;
    if (inv) {
      try {
        await inv("activate_license_cmd", { activationCode: code });
        if (settingsActivationMsg) {
          settingsActivationMsg.className = "form-msg success";
          settingsActivationMsg.textContent = "License activated successfully! ✨";
        }
        setTimeout(() => openSettingsFlyout(), 400);
      } catch (e) {
        if (settingsActivationMsg) {
          settingsActivationMsg.className = "form-msg error";
          settingsActivationMsg.textContent = `Activation failed: ${e}`;
        }
      } finally {
        btnActivateLicense.disabled = false;
      }
    }
  });

  // ─── Click Outside to Close ──────────────────────────────────
  document.addEventListener("click", (e) => {
    if (activeFlyoutMode === "context-menu" && widgetContextMenu && !widgetContextMenu.contains(e.target) && !mascotRegion.contains(e.target)) {
      closeAllFlyouts();
    } else if (activeFlyoutMode === "drawer" && widgetPlanDrawer && !widgetPlanDrawer.contains(e.target) && !mascotRegion.contains(e.target)) {
      closeAllFlyouts();
    } else if (activeFlyoutMode === "settings" && widgetSettingsFlyout && !widgetSettingsFlyout.contains(e.target) && !mascotRegion.contains(e.target)) {
      closeAllFlyouts();
    } else if (activeFlyoutMode === "input" && !chatForm.contains(e.target) && !mascotRegion.contains(e.target)) {
      closeAllFlyouts();
    }
  });

  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      closeAllFlyouts();
    }
  });

  // ─── Native Drag & Drop onto Widget ──────────────────────────
  if (typeof window !== "undefined") {
    document.addEventListener("dragover", (e) => e.preventDefault());
    document.addEventListener("drop", (e) => e.preventDefault());

    const evtApi = getEventApi();
    if (evtApi && evtApi.listen) {
      evtApi.listen("tauri://drag-drop", async (event) => {
        const paths = event.payload?.paths;
        if (paths && paths.length > 0) {
          scanAndPlanFolder(paths[0]);
        }
      });
    }
  }

  // ─── Inactivity Idle Sleep Timer ─────────────────────────────
  let sleepTimer = null;
  const INACTIVITY_SLEEP_DELAY = 45000;

  function resetActivityTimer() {
    if (currentMascotState === "sleeping") return;
    if (sleepTimer) clearTimeout(sleepTimer);
    if (appState === "idle" && activeFlyoutMode === "closed") {
      sleepTimer = setTimeout(() => {
        if (appState === "idle" && activeFlyoutMode === "closed") {
          setMascotState("sleeping");
        }
      }, INACTIVITY_SLEEP_DELAY);
    }
  }

  ["mousemove", "keydown", "touchstart"].forEach((evt) => {
    document.addEventListener(evt, () => {
      if (currentMascotState !== "sleeping") resetActivityTimer();
    }, { passive: true });
  });

  // ─── Utility ─────────────────────────────────────────────────
  const delay = (ms) => new Promise((r) => setTimeout(r, ms));

  // ─── Boot Initialization ─────────────────────────────────────
  checkReducedMotion();
  preloadFrames();
  setStatus("Ready", "idle");
  updatePendingBadge();
})();
