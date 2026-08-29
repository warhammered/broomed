/**
 * Broomed Widget — Pure Desktop Companion UI & Engine
 * 
 * Frame-based mascot animation with refresh-rate independent playback,
 * unified contextual hub, side-by-side plan inspector, and embedded settings.
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
        height: height || 180.0,
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
    await setWidgetDimensions(200.0, 180.0, restoreX);
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
    await setWidgetDimensions(200.0, 232.0, 0);
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
    await setWidgetDimensions(200.0, 275.0, 0);
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

    // Fetch live AI status badge
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
      // 1. Live AI Tier Status
      try {
        const s = await inv("get_active_ai_status_cmd");
        if (s && settingsActiveTierPill) {
          settingsActiveTierPill.textContent = s.label;
          settingsTierDesc.textContent = s.details;
        }
      } catch {}

      // 2. BYOK Config
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

      // 3. License & Device ID
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
    showToast("Scanning folder…");

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

  // ─── Mascot Click / Drag / Wake ──────────────────────────────
  let dragStartX = 0, dragStartY = 0, isDragging = false;
  const DRAG_THRESHOLD = 5;

  function wakeMascotIfSleeping() {
    if (currentMascotState === "sleeping") {
      setMascotState("coffee");
      resetActivityTimer();
      return true;
    }
    return false;
  }

  function handleMascotMouseDown(e) {
    if (e.button === 0) {
      dragStartX = e.screenX;
      dragStartY = e.screenY;
      isDragging = false;

      if (activeFlyoutMode !== "closed" && activeFlyoutMode !== "input" && activeFlyoutMode !== "approval") {
        closeAllFlyouts();
        e.stopPropagation();
        return;
      }

      if (wakeMascotIfSleeping()) {
        e.stopPropagation();
        return;
      }
    }
  }

  function handleMascotMouseMove(e) {
    if (e.buttons === 1 && !isDragging) {
      const dx = Math.abs(e.screenX - dragStartX);
      const dy = Math.abs(e.screenY - dragStartY);
      if (dx > DRAG_THRESHOLD || dy > DRAG_THRESHOLD) {
        isDragging = true;
        const inv = getInvoke() || invoke;
        if (inv) inv("drag_widget_window_cmd").catch(() => {});
      }
    }
  }

  function handleMascotClick(e) {
    if (e.button !== 0) return;
    if (isDragging) {
      e.stopPropagation();
      e.preventDefault();
      isDragging = false;
      return;
    }
    e.stopPropagation();

    if (activeFlyoutMode !== "closed" && activeFlyoutMode !== "input" && activeFlyoutMode !== "approval") {
      closeAllFlyouts();
      return;
    }

    if (wakeMascotIfSleeping()) return;

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
  mascotImg.addEventListener("mousemove", handleMascotMouseMove);
  mascotRegion.addEventListener("mousemove", handleMascotMouseMove);
  mascotImg.addEventListener("click", handleMascotClick);
  mascotRegion.addEventListener("click", handleMascotClick);
  mascotImg.addEventListener("mouseup", () => {
    setTimeout(() => { isDragging = false; }, 50);
  });

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
        // Check named folders
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
        // Fallback to active explorer window or downloads
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
