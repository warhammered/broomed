/**
 * Broomed Widget — Animation Engine + Chat Interaction
 *
 * Frame-based mascot animation using requestAnimationFrame + delta time
 * for refresh-rate independent playback.  7 idle frames at 5.56 fps.
 */
(() => {
  "use strict";

  // ─── Mascot States & Animation Engine ────────────────────────
  const MASCOT_STATES = {
    coffee: {
      name: "Coffee / Active Idle",
      frames: Array.from({length: 7}, (_, i) => `assets/mascot/coffee/broomed_coffee_${i+1}.png`),
      durationMs: 250
    },
<<<<<<< HEAD
=======
    searching: {
      name: "Searching / Thinking",
      frames: Array.from({length: 7}, (_, i) => `assets/mascot/searching/broomed_searching_${i+1}.png`),
      durationMs: 250
    },
>>>>>>> 27381596d3bff0b1a26b8941f4429928eb36fc85
    brooming: {
      name: "Brooming / Working",
      frames: Array.from({length: 14}, (_, i) => `assets/mascot/brooming/broomed-brooming-${i+1}.png`),
      durationMs: 250
    },
    sleeping: {
      name: "Sleeping / Deep Idle",
      frames: Array.from({length: 7}, (_, i) => `assets/mascot/sleeping/broomed_sleeping_${i+1}.png`),
      durationMs: 250
    }
  };

  // ─── DOM ─────────────────────────────────────────────────────
  const $ = (sel) => document.querySelector(sel);
<<<<<<< HEAD
  const mascotImg    = $("#mascot-img");
  const mascotRegion = $("#mascot-region");
  const chatPanel    = $("#chat-panel");
  const chatMessages = $("#chat-messages");
  const chatForm     = $("#chat-form");
  const chatInput    = $("#chat-input");
  const chatSend     = $("#chat-send");\r?\n
=======
  const widgetRoot         = $("#widget-root");
  const mascotCol          = $("#mascot-col");
  const mascotImg          = $("#mascot-img");
  const mascotRegion       = $("#mascot-region");
  const chatForm           = $("#chat-form");
  const chatInput          = $("#chat-input");
  const chatSend           = $("#chat-send");
  const btnSwitchPlan      = $("#btn-switch-plan");
  const pendingPillCount   = $("#pending-pill-count");
  const approvalCard       = $("#approval-card");
  const approvalBadge      = $("#approval-badge");
  const approvalTitle      = $("#approval-title");
  const approvalSummary    = $("#approval-summary");
  const btnApprove         = $("#btn-approve");
  const btnPreviewMain     = $("#btn-preview-main");
  const btnSwitchInput     = $("#btn-switch-input");
  const btnDismissApproval = $("#btn-dismiss-approval");
  const statusToast        = $("#status-toast");
  const toastText          = $("#toast-text");

  // Context Menu DOM
  const widgetContextMenu  = $("#widget-context-menu");
  const menuAiStatus       = $("#menu-ai-status");
  const menuPickFolder     = $("#menu-pick-folder");
  const menuOpenDashboard  = $("#menu-open-dashboard");
  const menuUndoLast       = $("#menu-undo-last");
  const menuToggleSleep    = $("#menu-toggle-sleep");
  const menuSleepLabel     = $("#menu-sleep-label");
  const menuSettings       = $("#menu-settings");
  const menuHideWidget     = $("#menu-hide-widget");

>>>>>>> 27381596d3bff0b1a26b8941f4429928eb36fc85
  // ─── Tauri Bridge ────────────────────────────────────────────
  const inv = window.__TAURI__?.core?.invoke;
  const evt = window.__TAURI__?.event;

  // ─── Local AI status ─────────────────────────────────────────
  let localAiStatus = null; // { available, reason }
  async function checkLocalAiStatus() {
    const inv = inv;
    if (!inv) return null;
    try {
      const raw = await inv("model_status_cmd");
      const data = typeof raw === "string" ? JSON.parse(raw) : raw;
      const hasModel = data && data.models && data.models["all-MiniLM-L6-v2"];
      const baseDir = data?.base_dir || "";
      const totalMb = data?.total_mb || 0;
      const available = !!hasModel;
      let probeReason = "heuristic fallback (model files not found)";
      let isHeuristic = true;
      try {
        const probe = await inv("classify_cmd", { task: "ClassifyFile", input: "probe_test_invoice.pdf", provider: "bundled" });
        if (probe && probe.reason) {
          if (probe.reason.includes("bundled model")) isHeuristic = false;
          else if (probe.reason.includes("embedding cosine")) isHeuristic = false;
          else if (probe.reason.includes("heuristic")) isHeuristic = true;
          probeReason = probe.reason;
        }
      } catch {}
      localAiStatus = { available: !isHeuristic, reason: probeReason, baseDir, totalMb };
      if (isHeuristic) {
        console.warn("[Broomed] Local AI fallback active — heuristic mode. Reason:", probeReason, "baseDir:", baseDir);
      } else {
        console.info("[Broomed] Local AI active:", probeReason);
      }
      return localAiStatus;
    } catch (e) {
      console.warn("[Broomed] model_status check failed:", e);
      return null;
    }
  }

  // ─── State ───────────────────────────────────────────────────
  const stateFramesCache = {};
  let currentMascotState = "coffee";
  let frames        = [];       // preloaded Image objects for active state
  let currentFrame  = 0;
  let elapsed       = 0;        // accumulated delta (ms)
  let lastTimestamp = null;
  let chatOpen      = false;
  let reducedMotion = false;
  let appState      = "idle";   // idle | scanning | classifying | preview | executing | error

  // ─── Preload All State Frames ────────────────────────────────
  function preloadFrames() {
    Object.keys(MASCOT_STATES).forEach((stateKey) => {
      const stateDef = MASCOT_STATES[stateKey];
      stateFramesCache[stateKey] = stateDef.frames.map((src) => {
        const img = new Image();
        img.src = src;
        // Automatic PNG <-> SVG fallback
        img.onerror = () => {
          if (src.endsWith(".png")) {
            const svgAlt = src.replace(/\.png$/, ".svg").replace(/_/g, "-");
            img.src = svgAlt;
          } else if (src.endsWith(".svg")) {
            const pngAlt1 = src.replace(/\.svg$/, ".png");
            const pngAlt2 = src.replace(/\.svg$/, ".png").replace(/-/g, "_");
            const testImg = new Image();
            testImg.onload = () => { img.src = pngAlt1; };
            testImg.onerror = () => { img.src = pngAlt2; };
            testImg.src = pngAlt1;
          }
        };
        return img;
      });
    });

    setMascotState("coffee");
    startAnimationLoop();
  }

  // ─── Mascot State Switcher ───────────────────────────────────
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
  // Expose globally for dev/preview usage
  window.setMascotState = setMascotState;

  // ─── Animation Loop ──────────────────────────────────────────
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

  // ─── Reduced Motion ──────────────────────────────────────────
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

  // ─── Dynamic Window Dimensions Helper ────────────────────────
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

  async function setWidgetHeight(height) {
    await setWidgetDimensions(200.0, height, 0.0);
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

  // ─── Plan & Prompt History Persistence ───────────────────────
  const STORAGE_KEY_PLAN    = "broomed_pending_plan";
  const STORAGE_KEY_HISTORY = "broomed_prompt_history";

  let pendingApproval = null;
  try {
    const savedPlan = sessionStorage.getItem(STORAGE_KEY_PLAN);
    if (savedPlan) pendingApproval = JSON.parse(savedPlan);
  } catch {}

  let promptHistory = [];
  let historyIndex = -1;
  try {
    const savedHist = localStorage.getItem(STORAGE_KEY_HISTORY);
    if (savedHist) promptHistory = JSON.parse(savedHist);
  } catch {}

  function savePendingPlan(plan) {
    pendingApproval = plan;
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
    if (btnSwitchPlan && pendingPillCount) {
      if (pendingApproval && pendingApproval.previews && pendingApproval.previews.length > 0) {
        pendingPillCount.textContent = pendingApproval.previews.length;
        btnSwitchPlan.classList.remove("hidden");
      } else {
        btnSwitchPlan.classList.add("hidden");
      }
    }
  }

  // Arrow key navigation for prompt history
  chatInput.addEventListener("keydown", (e) => {
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

  // ─── Input Bar & Approval State Management ───────────────────
  let widgetOpenMode = "closed"; // "closed" | "input" | "approval"

  async function openInputBar() {
    widgetOpenMode = "input";
    approvalCard.classList.add("hidden");
    updatePendingBadge();
    chatForm.classList.add("active");
    await setWidgetHeight(232.0);
    setTimeout(() => chatInput.focus(), 150);
  }

  async function showApprovalCard(folderPath, previews, summaryText, autoHandoff = true) {
    savePendingPlan({ folderPath, previews, summaryText });
    widgetOpenMode = "approval";
    chatForm.classList.remove("active");

    approvalBadge.textContent = previews.length + " ops";
    approvalTitle.textContent = "Plan Ready";
    approvalSummary.textContent = summaryText || ("Move " + previews.length + " files into categorized folders?");

    btnApprove.disabled = false;
    btnApprove.textContent = "✓ Organize";
    btnPreviewMain.disabled = false;
    btnPreviewMain.textContent = "↗ Preview";

    approvalCard.classList.remove("hidden");
    await setWidgetHeight(275.0);

    if (autoHandoff && previews.length > 0) {
      try {
        const inv = getInvoke() || invoke;
        if (inv) inv("emit_plan_to_main_cmd", { folderPath, previews }).catch(() => {});
      } catch {}
    }
  }

  async function closeWidget() {
    widgetOpenMode = "closed";
    chatForm.classList.remove("active");
    approvalCard.classList.add("hidden");
    chatInput.blur();
    await setWidgetHeight(180.0);
    setMascotState("coffee");
  }

  function toggleWidget() {
    if (widgetOpenMode !== "closed") {
      closeWidget();
    } else {
      // Reopen: if there is a pending approval, show it! Otherwise open input
      if (pendingApproval && pendingApproval.previews && pendingApproval.previews.length > 0) {
        showApprovalCard(pendingApproval.folderPath, pendingApproval.previews, pendingApproval.summaryText, false);
      } else {
        openInputBar();
      }
    }
  }

  // Switch between approval card and input bar
  btnSwitchInput?.addEventListener("click", () => {
    openInputBar();
  });

  btnSwitchPlan?.addEventListener("click", () => {
    if (pendingApproval) {
      showApprovalCard(pendingApproval.folderPath, pendingApproval.previews, pendingApproval.summaryText, false);
    }
  });

  btnDismissApproval.addEventListener("click", () => {
    savePendingPlan(null);
    closeWidget();
    showToast("Plan dismissed");
  });

  btnPreviewMain.addEventListener("click", async () => {
    if (!pendingApproval) return;
    btnPreviewMain.disabled = true;
    btnPreviewMain.textContent = "Opening…";
    try {
      await openMainWithPlan(pendingApproval.folderPath, pendingApproval.previews);
      await closeWidget();
    } catch (e) {
      showToast("Could not open main app: " + String(e));
    } finally {
      btnPreviewMain.disabled = false;
      btnPreviewMain.textContent = "↗ Preview";
    }
  });

  btnApprove.addEventListener("click", async () => {
    if (!pendingApproval || !pendingApproval.previews || pendingApproval.previews.length === 0) return;
    const plan = pendingApproval;
    btnApprove.disabled = true;
    btnApprove.textContent = "Organizing…";
    setStatus("Organizing…", "executing");

    try {
      const ids = await executePlanDirectly(plan.previews);
      const count = Array.isArray(ids) ? ids.length : plan.previews.length;
      showToast("✓ Organized " + count + " files");
      savePendingPlan(null);

      const evtApi = getEventApi();
      if (evtApi && evtApi.emit) {
        evtApi.emit("broomed:plan-executed", { folderPath: plan.folderPath, count }).catch(() => {});
        evtApi.emit("broomed:state-change", { state: "idle" }).catch(() => {});
      }

      await closeWidget();
      setStatus("Done", "idle");
    } catch (e) {
      showToast("Execute failed: " + String(e));
      setStatus("Error", "error");
      btnApprove.disabled = false;
      btnApprove.textContent = "✓ Organize";
    }
  });

  // ─── Mascot Click vs Drag & Wake Handler ─────────────────────
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

      // If context menu is open, clicking simply closes the menu
      if (contextMenuOpen) {
        hideContextMenu();
        e.stopPropagation();
        return;
      }

      // If sleeping, wake up immediately on mousedown
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
        if (inv) {
          inv("drag_widget_window_cmd").catch(() => {});
        }
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

    // If context menu is open, left-click closes it
    if (contextMenuOpen) {
      hideContextMenu();
      return;
    }

    if (wakeMascotIfSleeping()) {
      return;
    }

    toggleWidget();
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

  // ─── Right-click on widget → open main window ────────────────
  function openMainWindow() {
    const inv = inv;
    if (inv) {
      inv("show_main_window_cmd").catch((err) => console.error("show_main_window failed", err));
    } else {
      window.open("index.html", "_blank");
    }
  }

  async function openMainWithPlan(folderPath, previews) {
<<<<<<< HEAD
    const inv = inv;
    const evtApi = evt;
    // Try JS event emit first (Tauri v2)
=======
    const inv = getInvoke() || invoke;
    const evtApi = getEventApi();
>>>>>>> 27381596d3bff0b1a26b8941f4429928eb36fc85
    if (evtApi && evtApi.emit) {
      try {
        await evtApi.emit("broomed:plan-ready", { folderPath, previews, timestamp: Date.now() });
      } catch (e) { console.warn("event emit failed", e); }
    }
    if (inv) {
      try { await inv("emit_plan_to_main_cmd", { folderPath, previews }); } catch {}
      try { await inv("show_main_window_cmd"); } catch (e) { console.error("show_main_window failed", e); }
    } else {
      window.open("index.html", "_blank");
    }
  }

  async function executePlanDirectly(previews) {
    const inv = inv;
    if (!inv) return false;
    try {
      const ids = await inv("execute_plan_cmd", { previews, dbPath: null });
      return ids;
    } catch (e) {
      console.error("execute_plan failed", e);
      throw e;
    }
  }

  // ─── Custom Right-Click Context Menu (Side-by-Side) ─────────
  let contextMenuOpen = false;
  let lastMenuSide = "right";
  let lastXOffset = 0;

  async function showContextMenu() {
    contextMenuOpen = true;
    
    // Close other panels while menu is open so nothing overlaps
    chatForm.classList.remove("active");
    approvalCard.classList.add("hidden");

    if (menuSleepLabel) {
      menuSleepLabel.textContent = currentMascotState === "sleeping" ? "Wake Up" : "Put to Sleep";
    }

    if (menuAiStatus) {
      const inv = getInvoke() || invoke;
      if (inv) {
        try {
          const s = await inv("get_active_ai_status_cmd");
          if (s && s.label) menuAiStatus.textContent = s.label.split(" ")[0] || "Local AI";
        } catch {}
      }
    }

    // Determine available screen space to left vs right
    const screenW = typeof window !== "undefined" && window.screen ? (window.screen.availWidth || 1920) : 1920;
    const currentWinX = typeof window !== "undefined" ? (window.screenX || window.screenLeft || 0) : 0;
    const spaceOnRight = screenW - (currentWinX + 200);

    let side = "right";
    let xOffset = 0;

    // If less than 210px space on right and space exists on left, dock to left
    if (spaceOnRight < 210 && currentWinX > 210) {
      side = "left";
      xOffset = -200.0;
      if (widgetRoot) {
        widgetRoot.classList.remove("menu-active-right");
        widgetRoot.classList.add("menu-active-left");
      }
      if (widgetContextMenu) {
        widgetContextMenu.classList.remove("pos-right");
        widgetContextMenu.classList.add("pos-left");
      }
    } else {
      side = "right";
      xOffset = 0.0;
      if (widgetRoot) {
        widgetRoot.classList.remove("menu-active-left");
        widgetRoot.classList.add("menu-active-right");
      }
      if (widgetContextMenu) {
        widgetContextMenu.classList.remove("pos-left");
        widgetContextMenu.classList.add("pos-right");
      }
    }

    lastMenuSide = side;
    lastXOffset = xOffset;

    if (widgetContextMenu) widgetContextMenu.classList.remove("hidden");
    await setWidgetDimensions(400.0, 240.0, xOffset);
  }

  async function hideContextMenu() {
    if (!contextMenuOpen) return;
    contextMenuOpen = false;
    if (widgetContextMenu) widgetContextMenu.classList.add("hidden");
    if (widgetRoot) {
      widgetRoot.classList.remove("menu-active-right", "menu-active-left");
    }

    const restoreX = lastXOffset !== 0 ? -lastXOffset : 0;
    lastXOffset = 0;

    if (widgetOpenMode === "approval" && pendingApproval) {
      approvalCard.classList.remove("hidden");
      await setWidgetDimensions(200.0, 275.0, restoreX);
    } else if (widgetOpenMode === "input") {
      chatForm.classList.add("active");
      await setWidgetDimensions(200.0, 232.0, restoreX);
    } else {
      await setWidgetDimensions(200.0, 180.0, restoreX);
    }
  }

  // Right-click triggers custom menu everywhere on widget
  window.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    e.stopPropagation();
    showContextMenu();
  });

<<<<<<< HEAD
  // Prevent double-click from maximizing/fullscreening the widget

=======
  document.addEventListener("dblclick", (e) => {
    e.preventDefault();
    e.stopPropagation();
  });
>>>>>>> 27381596d3bff0b1a26b8941f4429928eb36fc85

  // Menu action: Pick Folder
  menuPickFolder?.addEventListener("click", async (e) => {
    e.stopPropagation();
    await hideContextMenu();
    const inv = getInvoke() || invoke;
    if (inv) {
      try {
        const folder = await inv("browse_directory_cmd");
        if (folder) {
          chatInput.value = folder;
          chatForm.dispatchEvent(new Event("submit"));
        }
      } catch (err) {
        console.warn("browse failed:", err);
      }
    } else {
      const p = prompt("Enter folder to scan:", "C:\\Downloads");
      if (p) {
        chatInput.value = p;
        chatForm.dispatchEvent(new Event("submit"));
      }
    }
  });

  // Menu action: Open Dashboard
  menuOpenDashboard?.addEventListener("click", async (e) => {
    e.stopPropagation();
    await hideContextMenu();
    openMainWindow();
  });

  // Menu action: Undo Last
  menuUndoLast?.addEventListener("click", async (e) => {
    e.stopPropagation();
    await hideContextMenu();
    const inv = getInvoke() || invoke;
    if (inv) {
      try {
        await inv("undo_last_cmd", { count: 1, dbPath: null });
        showToast("✓ Last organization reversed");
      } catch (err) {
        showToast("Undo error: " + err);
      }
    } else {
      showToast("✓ Last move reversed (demo)");
    }
  });

  // Menu action: Sleep / Wake Mascot
  menuToggleSleep?.addEventListener("click", async (e) => {
    e.stopPropagation();
    await hideContextMenu();
    if (currentMascotState === "sleeping") {
      setMascotState("coffee");
    } else {
      setMascotState("sleeping");
    }
  });

  // Menu action: Settings & BYOK
  menuSettings?.addEventListener("click", async (e) => {
    e.stopPropagation();
    await hideContextMenu();
    const inv = getInvoke() || invoke;
    const evtApi = getEventApi();
    if (inv) {
      await inv("show_main_window_cmd").catch(() => {});
      if (evtApi && evtApi.emit) {
        evtApi.emit("broomed:open-settings").catch(() => {});
      }
    } else {
      window.open("index.html", "_blank");
    }
  });

  // Menu action: Hide Mascot
  menuHideWidget?.addEventListener("click", async (e) => {
    e.stopPropagation();
    await hideContextMenu();
    const inv = getInvoke() || invoke;
    if (inv) {
      await inv("hide_widget_window_cmd").catch(() => {});
    }
  });

  // ─── Click Outside to Close ──────────────────────────────────
  document.addEventListener("click", (e) => {
    if (contextMenuOpen && widgetContextMenu && !widgetContextMenu.contains(e.target)) {
      hideContextMenu();
      return;
    }
    if (widgetOpenMode !== "closed" && !chatForm.contains(e.target) && !approvalCard.contains(e.target) && !mascotRegion.contains(e.target)) {
      closeWidget();
    }
  });

  // ─── Escape to Close ─────────────────────────────────────────
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      if (contextMenuOpen) {
        hideContextMenu();
      } else if (widgetOpenMode !== "closed") {
        closeWidget();
      }
    }
  });

  // ─── Activity & Idle Sleep Timer ─────────────────────────────
  let sleepTimer = null;
  const INACTIVITY_SLEEP_DELAY = 45000; // 45 seconds of inactivity -> sleeping

  function resetActivityTimer() {
    // Do NOT wake the mascot if it is in full idle / sleeping state!
    if (currentMascotState === "sleeping") {
      return;
    }
    if (sleepTimer) clearTimeout(sleepTimer);
    if (appState === "idle" && widgetOpenMode === "closed") {
      sleepTimer = setTimeout(() => {
        if (appState === "idle" && widgetOpenMode === "closed") {
          setMascotState("sleeping");
        }
      }, INACTIVITY_SLEEP_DELAY);
    }
  }

  // Mousemove and interactions only keep active state alive, never wake sleeping mascot
  ["mousemove", "keydown", "touchstart"].forEach((evt) => {
    document.addEventListener(evt, () => {
      if (currentMascotState !== "sleeping") {
        resetActivityTimer();
      }
    }, { passive: true });
  });

  // ─── Status Helpers ──────────────────────────────────────────
  function setStatus(text, mode = "idle") {
    appState = mode;

    // Automatic Mascot Animation State Switching
    if (mode === "scanning" || mode === "executing") {
      setMascotState("brooming");
    } else if (mode === "classifying" || mode === "thinking" || mode === "planning") {
      setMascotState("searching");
    } else if (mode === "sleeping" || mode === "exhausted" || mode === "no_credits" || mode === "idle_timeout") {
      setMascotState("sleeping");
    } else {
      setMascotState("coffee");
<<<<<<< HEAD
    }

=======
      resetActivityTimer();
    }
>>>>>>> 27381596d3bff0b1a26b8941f4429928eb36fc85
  }

  // ─── Tauri Helpers ──────────────────────────────────────────
  function esc(s) {
    return String(s)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
  }

  // ─── Small Input Submit: Intent Parse → Scan → Approval Flow ──
  chatForm.addEventListener("submit", async (e) => {
    e.preventDefault();

    const text = chatInput.value.trim();
    if (!text) return;

    addPromptToHistory(text);
    chatInput.value = "";
<<<<<<< HEAD
    addMessage(text, "user");

    const inv = inv;
    // No Tauri bridge — demo mode
=======
    const inv = getInvoke() || invoke;

    // Demo / browser fallback
>>>>>>> 27381596d3bff0b1a26b8941f4429928eb36fc85
    if (!inv) {
      setStatus("Thinking…", "thinking");
      showToast("Analyzing request…");
      await delay(800);
      const demoPreviews = [
        { operation: { source: "Demo_Doc.pdf", destination: "Downloads/Documents/Demo_Doc.pdf", id: "1" }, ai_result: { category: "Documents" } },
        { operation: { source: "Photo_1.png", destination: "Downloads/Images/Photo_1.png", id: "2" }, ai_result: { category: "Images" } },
      ];
      await showApprovalCard("C:\\Users\\Demo\\Downloads", demoPreviews, "Organize 2 files into Documents & Images?");
      setStatus("Ready", "idle");
      return;
    }

    chatSend.disabled = true;

    try {
      // 1. Parse intent
      setStatus("Thinking…", "thinking");
      showToast("Analyzing intent…");
      const intentJson = await inv("parse_intent_cmd", { text });

      // 2. Determine folder path
      let folderPath = null;
      try {
        const parsed = JSON.parse(intentJson);
        folderPath = parsed?.path || parsed?.folder || parsed?.directory || null;
      } catch {
        const pathMatch = intentJson.match(/(?:path|folder|directory):\s*"?([^",}\n]+)"?/i);
        if (pathMatch) folderPath = pathMatch[1].replace(/['"]/g, "");
      }

      if (!folderPath) {
        try {
          const explorerPath = await inv("get_active_explorer_path_cmd");
          if (explorerPath) folderPath = explorerPath;
        } catch {}
      }

      if (!folderPath) {
        showToast("Specify a folder, e.g. Downloads");
        setStatus("Ready", "idle");
        chatSend.disabled = false;
        return;
      }

      // 3. Scan directory
      setStatus("Scanning files…", "scanning");
      showToast("Scanning folder…");

      const files = await inv("scan_directory_cmd", {
        base: folderPath,
        maxFiles: 10000,
      });

      if (!files || files.length === 0) {
        showToast("No files found in folder");
        setStatus("Ready", "idle");
        chatSend.disabled = false;
        return;
      }

      // 4. Classify & plan
      setStatus("Classifying " + files.length + " files…", "classifying");
      showToast("Classifying " + files.length + " files…");

      let previews = [];
      let results = [];
      try {
        previews = await inv("plan_organize_cmd", {
          files: files,
          base: folderPath,
          task: "ClassifyFile",
          threshold: 0.5,
          provider: "bundled",
        });
        results = previews.map((p) => ({
          file: (p.operation?.source || p.ai_result?.category || "").split(/[/\\]/).pop() || p.operation?.source || "",
          path: p.operation?.source || "",
          category: p.ai_result?.category || "General",
          folder: p.ai_result?.suggested_folder || p.ai_result?.category || "General",
          confidence: p.ai_result?.confidence ?? 0.5,
          reason: p.ai_result?.reason || "",
        }));

        if (previews.length === 0 && files.length > 0) {
          for (const f of files) {
            try {
              const r = await inv("classify_cmd", { task: "ClassifyFile", input: f, provider: "bundled" });
              results.push({ file: f.split(/[/\\]/).pop() || f, path: f, category: r.category, folder: r.suggested_folder || r.category, confidence: r.confidence, reason: r.reason });
            } catch {
              results.push({ file: f.split(/[/\\]/).pop() || f, path: f, category: "General", folder: "General", confidence: 0.5, reason: "classification fallback" });
            }
          }
          const genId = () => (typeof crypto !== "undefined" && crypto.randomUUID ? crypto.randomUUID() : "00000000-0000-4000-a000-000000000000".replace(/0/g, () => Math.floor(Math.random()*16).toString(16)));
          previews = results.map((r) => ({
            operation: { source: r.path, destination: folderPath + "/" + r.folder + "/" + r.file, kind: "Move", reason: r.reason, confidence: r.confidence, reversible: true, status: "planned", id: genId() },
            ai_result: { category: r.category, confidence: r.confidence, suggested_folder: r.folder, reason: r.reason, tags: [], subcategory: null, suggested_name: null }
          }));
        }
      } catch (e) {
        console.warn("plan_organize failed, fallback to per-file classify:", e);
        results = [];
        for (const f of files) {
          try {
            const r = await inv("classify_cmd", { task: "ClassifyFile", input: f, provider: "bundled" });
            results.push({ file: f.split(/[/\\]/).pop() || f, path: f, category: r.category, folder: r.suggested_folder || r.category, confidence: r.confidence, reason: r.reason });
          } catch {
            results.push({ file: f.split(/[/\\]/).pop() || f, path: f, category: "General", folder: "General", confidence: 0.5, reason: "classification fallback" });
          }
        }
        const genId2 = () => (typeof crypto !== "undefined" && crypto.randomUUID ? crypto.randomUUID() : "00000000-0000-4000-a000-000000000000".replace(/0/g, () => Math.floor(Math.random()*16).toString(16)));
        previews = results.map((r) => ({
          operation: { source: r.path, destination: folderPath + "/" + r.folder + "/" + r.file, kind: "Move", reason: r.reason, confidence: r.confidence, reversible: true, status: "planned", id: genId2() },
          ai_result: { category: r.category, confidence: r.confidence, suggested_folder: r.folder, reason: r.reason, tags: [], subcategory: null, suggested_name: null }
        }));
      }

      // 5. Show Mini Approval Component
      const cats = new Set(results.map((d) => d.category));
      const summary = results.length + " files into " + cats.size + " categories in " + (folderPath.split(/[/\\]/).pop() || "folder") + "?";
      
      await showApprovalCard(folderPath, previews, summary);
      setStatus("Ready", "idle");
    } catch (err) {
      showToast("Error: " + String(err));
      setStatus("Error", "error");
    } finally {
      chatSend.disabled = false;
    }
  });

  // ─── Native Drag and Drop onto Widget ─────────────────────────
  if (typeof window !== "undefined") {
    document.addEventListener("dragover", (e) => e.preventDefault());
    document.addEventListener("drop", (e) => e.preventDefault());

    const evtApi = getEventApi();
    if (evtApi && evtApi.listen) {
      evtApi.listen("tauri://drag-drop", async (event) => {
        const paths = event.payload?.paths;
        if (paths && paths.length > 0) {
          const targetPath = paths[0];
          chatInput.value = targetPath;
          chatForm.dispatchEvent(new Event("submit"));
        }
      });

      // 1. Sync plan from Dashboard
      evtApi.listen("broomed:dashboard-plan-ready", (event) => {
        const { folderPath, previews, summary } = event.payload || {};
        if (folderPath && previews && previews.length > 0) {
          savePendingPlan({
            folderPath,
            previews,
            summaryText: summary || `${previews.length} operations planned`
          });
          setMascotState("coffee");
        }
      });

      // 2. Sync plan executed from Dashboard
      evtApi.listen("broomed:plan-executed", (event) => {
        savePendingPlan(null);
        setMascotState("brooming");
        setTimeout(() => setMascotState("coffee"), 1500);
        if (widgetOpenMode === "approval") {
          closeWidget();
        }
      });

      // 3. Sync animation & state from Dashboard (scanning, organizing, classifying, etc.)
      evtApi.listen("broomed:state-change", (event) => {
        const { state } = event.payload || {};
        if (state) {
          setStatus(state, state);
        }
      });

      // 4. Sync AI mode / license change from Dashboard
      evtApi.listen("broomed:ai-mode-changed", () => {
        checkLocalAiStatus();
        const inv = getInvoke() || invoke;
        if (inv && menuAiStatus) {
          inv("get_active_ai_status_cmd").then((s) => {
            if (s && s.label) menuAiStatus.textContent = s.label.split(" ")[0] || "Local AI";
          }).catch(() => {});
        }
      });

      // 5. Sync Undo from Dashboard
      evtApi.listen("broomed:undo-executed", () => {
        setMascotState("coffee");
      });
    }
  }

  // ─── Utility ─────────────────────────────────────────────────
  const delay = (ms) => new Promise((r) => setTimeout(r, ms));

  // ─── Boot ────────────────────────────────────────────────────
  checkReducedMotion();
  preloadFrames();
  setStatus("Ready", "idle");
  // Probe local AI status shortly after Tauri bridge ready
  setTimeout(() => { checkLocalAiStatus(); }, 800);
  setTimeout(() => { if (!localAiStatus) checkLocalAiStatus(); }, 2500);
})();
