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
      name: "Coffee / Idle",
      frames: Array.from({length: 7}, (_, i) => `assets/mascot/coffee/broomed-coffee-${i+1}.svg`),
      durationMs: 180
    },
    idle: {
      name: "Classic Idle",
      frames: Array.from({length: 7}, (_, i) => `assets/mascot/idle/broomed-idle-${i+2}.svg`),
      durationMs: 180
    },
    brooming: {
      name: "Brooming / Working 1",
      frames: Array.from({length: 7}, (_, i) => `assets/mascot/brooming/broomed-brooming-${i+1}.svg`),
      durationMs: 170
    },
    bookshelf: {
      name: "Books on Shelf / Working 2",
      frames: Array.from({length: 7}, (_, i) => `assets/mascot/bookshelf/broomed-bookshelf-${i+1}.svg`),
      durationMs: 180
    },
    thinking: {
      name: "Chin Rub / Thinking",
      frames: Array.from({length: 7}, (_, i) => `assets/mascot/thinking/broomed-thinking-${i+1}.svg`),
      durationMs: 190
    },
    bucket: {
      name: "Broom in Bucket (Static)",
      frames: ["assets/mascot/bucket/broomed-bucket.svg"],
      durationMs: 1000
    }
  };

  // ─── DOM ─────────────────────────────────────────────────────
  const $ = (sel) => document.querySelector(sel);
  const mascotImg    = $("#mascot-img");
  const mascotRegion = $("#mascot-region");
  const chatPanel    = $("#chat-panel");
  const chatMessages = $("#chat-messages");
  const chatForm     = $("#chat-form");
  const chatInput    = $("#chat-input");
  const chatSend     = $("#chat-send");
  const statusDot    = $("#status-dot");
  const statusText   = $("#status-text");
  const hasStatusBar = !!(statusDot && statusText);

  // ─── Tauri Bridge ────────────────────────────────────────────
  // Robust detection: Tauri v2 injects window.__TAURI__ or window.__TAURI_INTERNALS__
  // Check both, and handle async injection (poll for 2s)
  function getInvoke() {
    if (typeof window.__TAURI__ !== "undefined" && window.__TAURI__.core && window.__TAURI__.core.invoke) {
      return window.__TAURI__.core.invoke;
    }
    if (typeof window.__TAURI_INTERNALS__ !== "undefined" && window.__TAURI_INTERNALS__.invoke) {
      return window.__TAURI_INTERNALS__.invoke;
    }
    // Tauri v2 also exposes via window.__TAURI__.invoke in some configs
    if (typeof window.__TAURI__ !== "undefined" && window.__TAURI__.invoke) {
      return window.__TAURI__.invoke;
    }
    return null;
  }
  let invoke = getInvoke();
  // Poll for Tauri API if not yet injected (happens when script loads before Tauri bootstrap)
  if (!invoke) {
    let pollCount = 0;
    const poll = setInterval(() => {
      invoke = getInvoke();
      if (invoke || pollCount++ > 20) clearInterval(poll);
    }, 100);
  }
  const isTauri = () => !!getInvoke() || !!invoke;

  function getEventApi() {
    if (typeof window.__TAURI__ !== "undefined" && window.__TAURI__.event) return window.__TAURI__.event;
    if (typeof window.__TAURI_INTERNALS__ !== "undefined" && window.__TAURI_INTERNALS__.event) return window.__TAURI_INTERNALS__.event;
    return null;
  }

  // ─── Local AI status ─────────────────────────────────────────
  let localAiStatus = null; // { available, reason }
  async function checkLocalAiStatus() {
    const inv = getInvoke() || invoke;
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

  // ─── Chat Panel Toggle ───────────────────────────────────────
  function openChat() {
    if (chatOpen) return;
    chatOpen = true;
    chatPanel.classList.add("open");
    chatPanel.setAttribute("aria-hidden", "false");
    // Focus input after transition settles
    setTimeout(() => chatInput.focus(), 350);
  }

  function closeChat() {
    if (!chatOpen) return;
    chatOpen = false;
    chatPanel.classList.remove("open");
    chatPanel.setAttribute("aria-hidden", "true");
    chatInput.blur();
  }

  function toggleChat() {
    chatOpen ? closeChat() : openChat();
  }

  // ─── Mascot Click vs Drag ────────────────────────────────────
  // Make mascot draggable by clicking+holding on it, not around it.
  // Distinguish click (toggle chat) vs drag (move window) via movement threshold.
  let dragStartX = 0, dragStartY = 0, isDragging = false;
  const DRAG_THRESHOLD = 5;

  mascotImg.addEventListener("mousedown", (e) => {
    dragStartX = e.clientX;
    dragStartY = e.clientY;
    isDragging = false;
  });

  mascotImg.addEventListener("mousemove", (e) => {
    if (e.buttons === 1) {
      const dx = Math.abs(e.clientX - dragStartX);
      const dy = Math.abs(e.clientY - dragStartY);
      if (dx > DRAG_THRESHOLD || dy > DRAG_THRESHOLD) isDragging = true;
    }
  });

  mascotImg.addEventListener("click", (e) => {
    if (isDragging) {
      e.stopPropagation();
      e.preventDefault();
      isDragging = false;
      return;
    }
    e.stopPropagation();
    toggleChat();
  });

  mascotImg.addEventListener("mouseup", () => {
    // reset after short delay to allow click to fire
    setTimeout(() => { isDragging = false; }, 50);
  });

  // ─── Right-click on widget → open main window ────────────────
  function openMainWindow() {
    const inv = getInvoke() || invoke;
    if (inv) {
      inv("show_main_window_cmd").catch((err) => console.error("show_main_window failed", err));
    } else {
      // browser preview fallback
      window.open("index.html", "_blank");
    }
  }

  async function openMainWithPlan(folderPath, previews) {
    const inv = getInvoke() || invoke;
    const evtApi = getEventApi();
    // Try JS event emit first (Tauri v2)
    if (evtApi && evtApi.emit) {
      try {
        await evtApi.emit("broomed:plan-ready", { folderPath, previews, timestamp: Date.now() });
      } catch (e) { console.warn("event emit failed", e); }
    }
    // Fallback: use Rust command to emit to main window (works even if JS event API not injected)
    if (inv) {
      try { await inv("emit_plan_to_main_cmd", { folderPath, previews }); } catch {}
      try { await inv("show_main_window_cmd"); } catch (e) { console.error("show_main_window failed", e); }
    } else {
      window.open("index.html", "_blank");
    }
  }

  async function executePlanDirectly(previews) {
    const inv = getInvoke() || invoke;
    if (!inv) return false;
    try {
      const ids = await inv("execute_plan_cmd", { previews, dbPath: null });
      return ids;
    } catch (e) {
      console.error("execute_plan failed", e);
      throw e;
    }
  }

  // Direct open on right-click (contextmenu) anywhere on widget
  document.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    openMainWindow();
  });

  // Prevent double-click from maximizing/fullscreening the widget
  document.addEventListener("dblclick", (e) => {
    e.preventDefault();
    e.stopPropagation();
  });

  // ─── Click Outside to Close ──────────────────────────────────
  document.addEventListener("click", (e) => {
    if (chatOpen && !chatPanel.contains(e.target) && !mascotRegion.contains(e.target)) {
      closeChat();
    }
  });

  // ─── Escape to Close ─────────────────────────────────────────
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && chatOpen) {
      closeChat();
    }
  });

  // ─── Chat Messages ───────────────────────────────────────────
  function addMessage(text, type = "bot") {
    // Remove hint on first real message
    const hint = chatMessages.querySelector(".chat-hint");
    if (hint) hint.remove();

    const p = document.createElement("p");
    p.className = "msg " + type;
    p.textContent = text;
    chatMessages.appendChild(p);
    chatMessages.scrollTop = chatMessages.scrollHeight;
    return p;
  }

  function showTyping() {
    return addMessage("Thinking…", "typing");
  }

  function removeTyping(el) {
    if (el && el.parentNode) el.remove();
  }

  // ─── Status Helpers ──────────────────────────────────────────
  function setStatus(text, mode = "idle") {
    appState = mode;

    // Automatic Mascot Animation State Switching
    if (mode === "scanning" || mode === "executing") {
      setMascotState("brooming");
    } else if (mode === "classifying") {
      setMascotState("bookshelf");
    } else if (mode === "thinking") {
      setMascotState("thinking");
    } else if (mode === "exhausted" || mode === "no_credits" || mode === "idle_timeout") {
      setMascotState("bucket");
    } else {
      setMascotState("coffee");
    }

    if (hasStatusBar && statusText) {
      statusText.textContent = text;
    }

    if (hasStatusBar && statusDot) {
      statusDot.classList.remove("active", "error", "processing");
      if (mode === "scanning" || mode === "classifying" || mode === "executing" || mode === "thinking") {
        statusDot.classList.add("processing");
      } else if (mode === "error") {
        statusDot.classList.add("error");
      } else if (mode === "preview" || mode === "executed") {
        statusDot.classList.add("active");
      }
    }
  }

  // ─── Tauri Helpers ──────────────────────────────────────────
  function esc(s) {
    return String(s)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
  }

  // ─── Chat Submit: Intent Parse → Scan Flow ───────────────────
  chatForm.addEventListener("submit", async (e) => {
    e.preventDefault();

    const text = chatInput.value.trim();
    if (!text) return;

    chatInput.value = "";
    addMessage(text, "user");

    const inv = getInvoke() || invoke;
    // No Tauri bridge — demo mode
    if (!inv) {
      const typing = showTyping();
      setStatus("Thinking…", "thinking");
      await delay(600);
      removeTyping(typing);
      addMessage("Widget running in browser preview mode. Open in Tauri to use AI features. (Debug: __TAURI__=" + (typeof window.__TAURI__ !== "undefined") + " __TAURI_INTERNALS__=" + (typeof window.__TAURI_INTERNALS__ !== "undefined") + ")");
      setStatus("Ready", "idle");
      return;
    }

    const typing = showTyping();
    chatSend.disabled = true;

    try {
      // 1. Parse the user's intent
      setStatus("Thinking…", "thinking");
      const intentJson = await inv("parse_intent_cmd", { text });

      // 2. Determine folder — try active explorer path if no explicit path
      let folderPath = null;
      try {
        const parsed = JSON.parse(intentJson);
        folderPath = parsed?.path || parsed?.folder || parsed?.directory || null;
      } catch {
        // intentJson is a Debug string, extract path-like substrings
        const pathMatch = intentJson.match(/(?:path|folder|directory):\s*"?([^",}\n]+)"?/i);
        if (pathMatch) folderPath = pathMatch[1].replace(/['"]/g, "");
      }

      if (!folderPath) {
        try {
          const explorerPath = await inv("get_active_explorer_path_cmd");
          if (explorerPath) folderPath = explorerPath;
        } catch {
          // Ignore — no active explorer
        }
      }

      if (!folderPath) {
        removeTyping(typing);
        addMessage("I couldn't determine a folder path. Try: \"Organize C:\\Users\\Downloads\"");
        setStatus("Ready", "idle");
        chatSend.disabled = false;
        return;
      }

      // 3. Scan the directory
      removeTyping(typing);
      setStatus("Scanning files…", "scanning");
      const scanTyping = addMessage("Scanning " + esc(folderPath) + "…", "typing");

      const files = await inv("scan_directory_cmd", {
        base: folderPath,
        maxFiles: 10000,
      });

      if (!files || files.length === 0) {
        removeTyping(scanTyping);
        addMessage("No files found in " + esc(folderPath));
        setStatus("Ready", "idle");
        chatSend.disabled = false;
        return;
      }

      // 4. Classify + create real plan (fixes handoff bug: previously only classified, never planned)
      removeTyping(scanTyping);
      setStatus("Classifying " + files.length + " files…", "classifying");
      const classTyping = addMessage("Classifying " + files.length + " files…", "typing");

      let previews = [];
      let results = [];
      try {
        // Use batch plan_organize — creates Operation + AiResult per file, threshold 0.5
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
        // Fallback if plan filtered everything (e.g. threshold too high or dst exists)
        if (previews.length === 0 && files.length > 0) {
          // Fall back to per-file classify so user still sees something
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
          console.warn("[Broomed] plan_organize returned 0, fallback to per-file classify:", results.length);
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
        // Build synthetic previews from results so handoff still works
        const genId2 = () => (typeof crypto !== "undefined" && crypto.randomUUID ? crypto.randomUUID() : "00000000-0000-4000-a000-000000000000".replace(/0/g, () => Math.floor(Math.random()*16).toString(16)));
        previews = results.map((r) => ({
          operation: { source: r.path, destination: folderPath + "/" + r.folder + "/" + r.file, kind: "Move", reason: r.reason, confidence: r.confidence, reversible: true, status: "planned", id: genId2() },
          ai_result: { category: r.category, confidence: r.confidence, suggested_folder: r.folder, reason: r.reason, tags: [], subcategory: null, suggested_name: null }
        }));
      }

      // 5. Present summary + hand off to main app
      removeTyping(classTyping);
      const cats = new Set(results.map((d) => d.category));
      const avg = results.reduce((s, d) => s + d.confidence, 0) / (results.length || 1);
      const isHeuristic = results.some((r) => r.reason && r.reason.includes("heuristic"));
      const aiLabel = isHeuristic ? "heuristic (local model not loaded)" : "local AI";
      const summary =
        results.length + " files classified into " + cats.size +
        " categories (avg " + Math.round(avg * 100) + "% confidence, " + aiLabel + ").";

      addMessage(summary, "bot");
      if (isHeuristic && localAiStatus && !localAiStatus.available) {
        addMessage("Note: Local AI model not found at " + (localAiStatus.baseDir || "model dir") + ". Using extension-based fallback. Install models to enable real AI.", "bot");
      }

      // Create actionable handoff UI (sibling element, not nested in <p>)
      const actions = document.createElement("div");
      actions.className = "widget-actions";
      actions.style.cssText = "display:flex;gap:8px;margin-top:8px;flex-wrap:wrap;";
      const btnPreview = document.createElement("button");
      btnPreview.textContent = "Preview in main app (" + previews.length + " ops)";
      btnPreview.style.cssText = "flex:1;padding:8px 10px;border-radius:8px;border:1px solid #6c5ce7;background:#6c5ce7;color:#fff;cursor:pointer;font-weight:600;";
      btnPreview.onclick = async () => {
        btnPreview.disabled = true; btnPreview.textContent = "Opening…";
        try { await openMainWithPlan(folderPath, previews); addMessage("Opened main app with plan. Review and Execute there.", "bot"); }
        catch (e) { addMessage("Failed to open main app: " + String(e), "error"); }
        finally { btnPreview.disabled = false; btnPreview.textContent = "Preview in main app (" + previews.length + " ops)"; }
      };
      const btnExec = document.createElement("button");
      btnExec.textContent = "Execute now";
      btnExec.style.cssText = "padding:8px 10px;border-radius:8px;border:1px solid #00b894;background:#00b894;color:#fff;cursor:pointer;font-weight:600;";
      btnExec.onclick = async () => {
        if (!confirm("Execute " + previews.length + " moves in " + folderPath + "? This can be undone.")) return;
        btnExec.disabled = true; btnExec.textContent = "Executing…";
        setStatus("Executing…", "executing");
        try {
          const ids = await executePlanDirectly(previews);
          addMessage("Executed " + (Array.isArray(ids) ? ids.length : previews.length) + " operations. Undo available in main app.", "bot");
          setStatus("Done", "executed");
        } catch (e) { addMessage("Execute failed: " + String(e), "error"); setStatus("Error", "error"); }
        finally { btnExec.disabled = false; btnExec.textContent = "Execute now"; }
      };
      actions.appendChild(btnPreview);
      if (previews.length > 0) actions.appendChild(btnExec);
      chatMessages.appendChild(actions);
      chatMessages.scrollTop = chatMessages.scrollHeight;

      // Auto-handoff: push plan to main window so it shows immediately when opened
      if (previews.length > 0) {
        try { await openMainWithPlan(folderPath, previews); } catch {}
      }

      setStatus(results.length + " files ready — " + previews.length + " ops planned", "preview");
    } catch (err) {
      removeTyping(typing);
      addMessage("Error: " + String(err), "error");
      setStatus("Error", "error");
    } finally {
      chatSend.disabled = false;
    }
  });

  // ─── Utility ─────────────────────────────────────────────────
  const delay = (ms) => new Promise((r) => setTimeout(r, ms));

  // ─── Boot ────────────────────────────────────────────────────
  checkReducedMotion();
  preloadFrames();
  setStatus("Ready", "idle");
  // Probe local AI status shortly after Tauri bridge ready
  setTimeout(() => { checkLocalAiStatus(); }, 800);
  // Re-check when Tauri finally injects if it was missing at boot
  setTimeout(() => { if (!localAiStatus) checkLocalAiStatus(); }, 2500);
})();
