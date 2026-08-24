import { initMascot } from "./mascot/index.js";
import { sfx } from "./audio.js";

(() => {
  "use strict";

  // ─── DOM Elements ───
  const $ = (s) => document.querySelector(s);
  const petContainer = $("#pet-container");
  const mascotEl = $("#mascot");
  const mascotBubble = $("#mascot-bubble");
  const bubbleMainView = $("#bubble-main-view");
  const bubbleLicenseView = $("#bubble-license-view");
  const bubbleMsg = $("#bubble-msg");
  const bubbleInput = $("#bubble-input");
  const bubbleSendBtn = $("#bubble-send-btn");
  const bubbleClose = $("#bubble-close");
  const folderInput = $("#folder-input");
  const contextMenu = $("#context-menu");
  const particlesContainer = $("#particles");

  // Chips
  const chipClean = $("#chip-clean");
  const chipUndo = $("#chip-undo");
  const chipLicense = $("#chip-license");

  // License Sub-View
  const licenseBackBtn = $("#license-back-btn");
  const licenseCodeInput = $("#license-code-input");
  const licenseSubmitBtn = $("#license-submit-btn");
  const licenseStatusMsg = $("#license-status-msg");

  // Context Menu Items
  const menuClean = $("#menu-clean");
  const menuUndo = $("#menu-undo");
  const menuLicense = $("#menu-license");
  const menuWeb = $("#menu-web");
  const menuQuit = $("#menu-quit");

  // ─── Initialize Mascot Controller & Renderer ───
  const mascot = initMascot(mascotEl, mascotBubble, bubbleInput, bubbleClose);

  // ─── Tauri Bridge ───
  const isTauri = typeof window.__TAURI__ !== "undefined";
<<<<<<< HEAD
  const invoke = isTauri ? window.__TAURI__.core?.invoke : null;
  const appWindow = isTauri ? window.__TAURI__.window?.getCurrentWindow?.() : null;
=======
  const invoke = isTauri ? window.__TAURI__.core.invoke : null;
  const eventApi = isTauri && window.__TAURI__.event ? window.__TAURI__.event : null;

  // ─── Widget handoff: receive plan from widget ──────────────────
  function applyPlanFromWidget(payload) {
    try {
      console.info("[Broomed] applyPlanFromWidget payload:", payload);
      const folder = payload?.folderPath || payload?.folder || null;
      const previews = payload?.previews || payload?.plan || [];
      if (!previews || previews.length === 0) {
        console.warn("[Broomed] applyPlanFromWidget: empty previews, payload:", payload);
        statusText.textContent = "Widget sent empty plan (0 ops) — check widget logs. Folder: " + (folder || "unknown");
        // Still set folderPath so user can Preview manually
        if (folder) folderPath = folder;
        return;
      }
      if (folder) folderPath = folder;
      // Ensure each preview has a valid UUID (widget fallback previously sent id:"")
      const genId = () => (typeof crypto !== "undefined" && crypto.randomUUID ? crypto.randomUUID() : "00000000-0000-4000-a000-000000000000".replace(/0/g, () => Math.floor(Math.random()*16).toString(16)));
      for (const p of previews) {
        if (p.operation && (!p.operation.id || p.operation.id === "")) {
          p.operation.id = genId();
        }
      }
      // Convert PlanPreview[] to previewData format expected by renderPreview/executePlan
      const mapped = previews.map((p) => {
        const op = p.operation || p;
        const ai = p.ai_result || p.aiResult || {};
        const src = op.source || op.path || ai.path || "";
        return {
          file: (src.split(/[/\\]/).pop() || src),
          path: src,
          category: ai.category || "General",
          folder: ai.suggested_folder || ai.category || op.destination?.split(/[/\\]/).slice(-2, -1)[0] || "General",
          confidence: typeof ai.confidence === "number" ? ai.confidence : (op.confidence ?? 0.5),
          reason: ai.reason || op.reason || "",
          _preview: p, // keep original for execute
        };
      });
      // Store original previews for executePlan (needs PlanPreview[])
      window.__broomed_last_previews = previews;
      renderPreview(mapped);
      setState("preview");
      statusText.textContent = "Plan from widget: " + previews.length + " ops in " + (folder || "folder");
    } catch (e) {
      console.warn("applyPlanFromWidget failed", e);
    }
  }

  // Listen via Tauri event (widget emits broomed:plan-ready)
  if (eventApi && eventApi.listen) {
    eventApi.listen("broomed:plan-ready", (event) => {
      console.info("[Broomed] received broomed:plan-ready", event);
      const payload = event?.payload || event;
      applyPlanFromWidget(payload);
    }).catch((e) => console.warn("listen broomed:plan-ready failed", e));
    // Also listen for Rust-emitted event (same name)
    eventApi.listen("broomed:plan-ready-rust", (event) => {
      console.info("[Broomed] received broomed:plan-ready-rust", event);
      const payload = event?.payload || event;
      applyPlanFromWidget(payload);
    }).catch((e) => console.warn("listen broomed:plan-ready-rust failed", e));
  } else {
    console.warn("[Broomed] Tauri event API not available, widget handoff via JS events only");
  }
  // Fallback: also accept via window event (for browser preview)
  window.addEventListener("broomed:plan-ready", (e) => applyPlanFromWidget(e.detail));
>>>>>>> 523f7173de8fb52ca9a5a0ef64785dd8e8d30f3e

  // ─── Native Window Dragging ───
  if (appWindow) {
    petContainer.addEventListener("mousedown", (e) => {
      // Don't drag if clicking inside bubble or context menu
      if ((mascotBubble && mascotBubble.contains(e.target)) || (contextMenu && contextMenu.contains(e.target))) {
        return;
      }
      if (e.button === 0) {
        try {
          appWindow.startDragging();
        } catch {
          // ignore
        }
      }
    });
  }

  const delay = (ms) => new Promise((r) => setTimeout(r, ms));

  // ─── Particle Effects ───
  function spawnCelebrationStars() {
    if (!particlesContainer) return;
    particlesContainer.innerHTML = "";
    const count = 10;
    for (let i = 0; i < count; i++) {
      const star = document.createElement("div");
      star.className = "particle-star";
      const angle = (i / count) * Math.PI * 2 + (Math.random() * 0.4 - 0.2);
      const dist = 24 + Math.random() * 32;
      const tx = Math.cos(angle) * dist;
      const ty = Math.sin(angle) * dist - 12;

      star.style.setProperty("--tx", `${tx}px`);
      star.style.setProperty("--ty", `${ty}px`);
      star.style.left = "50%";
      star.style.top = "40%";
      particlesContainer.appendChild(star);

      setTimeout(() => star.remove(), 750);
    }
  }

  // ─── Organize Folder Workflow ───
  async function organizeFolder(path) {
    if (!path) return;

    sfx.playBrush();
    mascot.setAppState({ scanning: true, organizing: true });
    if (bubbleMsg) bubbleMsg.textContent = `Sweeping folder: ${path.split(/[\\/]/).pop()}...`;

    if (!invoke) {
      // Browser demo simulation
      await delay(1200);
      mascot.setAppState({ scanning: false, organizing: false });
      mascot.notifySuccess();
      sfx.playChime();
      spawnCelebrationStars();
      if (bubbleMsg) bubbleMsg.textContent = "All organized and clean! ✨";
      return;
    }

    try {
      // 1. Scan directory
      const files = await invoke("scan_directory_cmd", { base: path, maxFiles: 10000 });
      if (!files || files.length === 0) {
        mascot.setAppState({ scanning: false, organizing: false, error: false });
        if (bubbleMsg) bubbleMsg.textContent = "Folder is already clean & empty.";
        return;
      }

      // 2. Classify & plan
      const previews = await invoke("plan_organize", {
        files,
        base: path,
        task: "ClassifyFile",
        threshold: 0.5,
        provider: "bundled",
      });

      if (previews && previews.length > 0) {
        // 3. Execute plan
        await invoke("execute_plan_cmd", { previews, dbPath: null });
      }

      // 4. Success celebration
      mascot.setAppState({ scanning: false, organizing: false, error: false });
      mascot.notifySuccess();
      sfx.playChime();
      spawnCelebrationStars();
      if (bubbleMsg) bubbleMsg.textContent = `Organized ${previews ? previews.length : 0} files! ✨`;
    } catch (err) {
      console.error("Organize failed:", err);
      mascot.setAppState({ scanning: false, organizing: false, error: true });
      sfx.playError();
      if (bubbleMsg) bubbleMsg.textContent = "Sweep encountered an issue.";
    }
  }

  // ─── Undo Last Action ───
  async function undoLastAction() {
    mascot.setAppState({ attention: true });
    sfx.playPop();

    if (invoke) {
      try {
        const undone = await invoke("undo_last_operation", {});
        if (bubbleMsg) bubbleMsg.textContent = undone ? "Reverted last move! ↩️" : "No previous moves to undo.";
      } catch (err) {
        if (bubbleMsg) bubbleMsg.textContent = "Undo failed: " + err;
      }
    } else {
      if (bubbleMsg) bubbleMsg.textContent = "Reverted last move! ↩️";
    }

    setTimeout(() => mascot.setAppState({ attention: false }), 1500);
  }

  // ─── Context Menu Controls (OpenPets style) ───
  function showContextMenu(x, y) {
    if (!contextMenu) return;
    contextMenu.classList.remove("hidden");
    contextMenu.style.left = `${Math.min(x, 90)}px`;
    contextMenu.style.top = `${Math.min(y, 100)}px`;
    sfx.playPop();
  }

<<<<<<< HEAD
  function hideContextMenu() {
    if (contextMenu) contextMenu.classList.add("hidden");
  }
=======
  // ─── Render Preview Table ───
  function renderPreview(data) {
    previewData = data;
    // Clear widget handoff cache if this is a fresh render not from widget
    // (widget path sets window.__broomed_last_previews explicitly after calling renderPreview)
    previewBody.innerHTML = "";
    previewCount.textContent = data.length + " files";
>>>>>>> 523f7173de8fb52ca9a5a0ef64785dd8e8d30f3e

  petContainer.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    showContextMenu(e.clientX, e.clientY);
  });

  document.addEventListener("click", (e) => {
    if (contextMenu && !contextMenu.contains(e.target)) {
      hideContextMenu();
    }
  });

  menuClean?.addEventListener("click", () => {
    hideContextMenu();
    folderInput?.click();
  });

  menuUndo?.addEventListener("click", () => {
    hideContextMenu();
    undoLastAction();
  });

  menuLicense?.addEventListener("click", () => {
    hideContextMenu();
    showLicenseView();
  });

  menuWeb?.addEventListener("click", () => {
    hideContextMenu();
    window.open("https://broomed.app", "_blank");
  });

  menuQuit?.addEventListener("click", () => {
    hideContextMenu();
    if (appWindow) {
      appWindow.close();
    }
  });

  // ─── Action Chips Controls ───
  chipClean?.addEventListener("click", () => {
    folderInput?.click();
  });

  chipUndo?.addEventListener("click", () => {
    undoLastAction();
  });

  chipLicense?.addEventListener("click", () => {
    showLicenseView();
  });

  // ─── License Sub-View Handlers ───
  function showLicenseView() {
    if (!mascotBubble) return;
    mascotBubble.classList.remove("hidden");
    bubbleMainView?.classList.add("hidden");
    bubbleLicenseView?.classList.remove("hidden");
    if (licenseStatusMsg) licenseStatusMsg.textContent = "";
    sfx.playPop();
  }

  function hideLicenseView() {
    bubbleLicenseView?.classList.add("hidden");
    bubbleMainView?.classList.remove("hidden");
  }

  licenseBackBtn?.addEventListener("click", () => {
    hideLicenseView();
  });

  licenseSubmitBtn?.addEventListener("click", async () => {
    const code = licenseCodeInput?.value?.trim();
    if (!code) {
      if (licenseStatusMsg) {
        licenseStatusMsg.className = "license-status-msg error";
        licenseStatusMsg.textContent = "Please enter an activation code.";
      }
<<<<<<< HEAD
      return;
    }

    if (licenseStatusMsg) {
      licenseStatusMsg.className = "license-status-msg";
      licenseStatusMsg.textContent = "Validating with edge control plane...";
=======
      previewBody.appendChild(frag);
      if (i < data.length) requestAnimationFrame(renderBatch);
    }
    renderBatch();
  }

  // ─── Scan & Classify ───
  async function scanFolder(path) {
    folderPath = path;
    window.__broomed_last_previews = null;
    setState("scanning");

    if (!invoke) {
      await delay(1000);
      setState("classifying");
      await delay(700);
      renderPreview(generateDemoData());
      setState("preview");
      return;
    }

    try {
      const files = await invoke("scan_directory_cmd", { base: path, maxFiles: 10000 });
      if (!files || files.length === 0) {
        setState("idle");
        statusText.textContent = "No files found in directory";
        return;
      }

      setState("classifying");
      const results = [];
      for (const f of files) {
        try {
          const r = await invoke("classify_cmd", {
            task: "ClassifyFile",
            input: f,
            provider: currentAiMode === "online" ? "online" : "bundled",
          });
          results.push({
            file: fileName(f), path: f,
            category: r.category,
            folder: r.suggested_folder || r.category,
            confidence: r.confidence,
            reason: r.reason,
          });
        } catch {
          results.push({
            file: fileName(f), path: f,
            category: "General", folder: "General",
            confidence: 0.5, reason: "classification fallback",
          });
        }
      }
      renderPreview(results);
      setState("preview");
    } catch (err) {
      setState("error");
      statusText.textContent = "Scan failed: " + String(err);
    }
  }

  async function classifyFiles() {
    window.__broomed_last_previews = null;
    setState("classifying");

    if (!invoke) {
      await delay(600);
      renderPreview(generateDemoData());
      setState("preview");
      return;
>>>>>>> 523f7173de8fb52ca9a5a0ef64785dd8e8d30f3e
    }

    if (invoke) {
      try {
        await invoke("activate_license_code", { code });
        if (licenseStatusMsg) {
          licenseStatusMsg.className = "license-status-msg success";
          licenseStatusMsg.textContent = "License activated successfully! ✨";
        }
        sfx.playChime();
        spawnCelebrationStars();
        setTimeout(hideLicenseView, 1500);
      } catch (err) {
        if (licenseStatusMsg) {
          licenseStatusMsg.className = "license-status-msg error";
          licenseStatusMsg.textContent = "Activation failed: " + err;
        }
        sfx.playError();
      }
<<<<<<< HEAD
    } else {
      if (licenseStatusMsg) {
        licenseStatusMsg.className = "license-status-msg success";
        licenseStatusMsg.textContent = "License activated (demo mode)! ✨";
      }
      sfx.playChime();
      setTimeout(hideLicenseView, 1500);
=======
    }
    renderPreview(results);
    setState("preview");
  }

  // ─── Execute & Undo ───
  async function executePlan() {
    setState("executing");

    if (!invoke) {
      await delay(1200);
      setState("executed");
      return;
    }

    try {
      // If we have a widget-provided plan, execute it directly (preserves destinations)
      let previews = window.__broomed_last_previews;
      if (!previews || !Array.isArray(previews) || previews.length === 0) {
        const files = previewData.map((d) => d.path || d.file).filter(Boolean);
        const base = folderPath || ".";
        previews = await invoke("plan_organize", {
          files, base, task: "ClassifyFile", threshold: 0.5, provider: currentAiMode === "online" ? "online" : "bundled",
        });
      } else {
        // Validate that stored previews still match current previewData length; if not, re-plan
        if (previews.length !== previewData.length) {
          const files = previewData.map((d) => d.path || d.file).filter(Boolean);
          const base = folderPath || ".";
          previews = await invoke("plan_organize", {
            files, base, task: "ClassifyFile", threshold: 0.5, provider: currentAiMode === "online" ? "online" : "bundled",
          });
          window.__broomed_last_previews = previews;
        }
      }
      if (!previews || previews.length === 0) {
        setState("error");
        statusText.textContent = "Nothing to execute";
        return;
      }
      // Fix empty ids from widget synthetic previews
      const genId2 = () => (typeof crypto !== "undefined" && crypto.randomUUID ? crypto.randomUUID() : "00000000-0000-4000-a000-000000000000".replace(/0/g, () => Math.floor(Math.random()*16).toString(16)));
      for (const p of previews) {
        if (p.operation && (!p.operation.id || p.operation.id === "")) p.operation.id = genId2();
      }
      await invoke("execute_plan_cmd", { previews, dbPath: null });
      window.__broomed_last_previews = null;
      setState("executed");
    } catch (err) {
      setState("error");
      statusText.textContent = "Execute failed: " + String(err);
    }
  }

  async function undoLast() {
    setState("executing");

    if (!invoke) {
      await delay(500);
      setState("preview");
      return;
    }

    try {
      await invoke("undo_last_cmd", { count: 1, dbPath: null });
      setState("preview");
    } catch (err) {
      setState("error");
      statusText.textContent = "Undo failed: " + String(err);
    }
  }

  // ─── File Picker ───
  async function pickFolder() {
    if (!invoke) {
      const path = prompt("Enter folder path to scan:", "C:\\Users\\Demo\\Downloads");
      if (path) scanFolder(path);
      return;
    }
    try {
      const result = await invoke("browse_directory_cmd");
      if (result) scanFolder(result);
    } catch {
      setState("error");
      statusText.textContent = "Could not open folder picker";
    }
  }

  // ─── Event Listeners ───
  btnLicenseModal.addEventListener("click", openLicenseModal);
  btnCloseModal.addEventListener("click", closeLicenseModal);
  btnModalDone.addEventListener("click", closeLicenseModal);
  btnActivate.addEventListener("click", activateLicense);
  btnRefresh.addEventListener("click", refreshLicense);

  licenseModal.addEventListener("click", (e) => {
    if (e.target === licenseModal) closeLicenseModal();
  });

  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && !licenseModal.classList.contains("hidden")) {
      closeLicenseModal();
>>>>>>> 523f7173de8fb52ca9a5a0ef64785dd8e8d30f3e
    }
  });

  // ─── Drag and Drop Handling (Desktop & Browser) ───
  petContainer.addEventListener("dragover", (e) => {
    e.preventDefault();
    mascotEl.classList.add("dragover");
  });

  petContainer.addEventListener("dragleave", (e) => {
    e.preventDefault();
    mascotEl.classList.remove("dragover");
  });

  petContainer.addEventListener("drop", (e) => {
    e.preventDefault();
    mascotEl.classList.remove("dragover");

    const items = e.dataTransfer?.items;
    if (items && items.length > 0) {
      const entry = items[0].webkitGetAsEntry?.();
      if (entry) {
        organizeFolder(entry.fullPath || entry.name);
      }
    }
  });

  // Tauri native file drop listener
  if (isTauri && window.__TAURI__?.event?.listen) {
    window.__TAURI__.event.listen("tauri://drag-drop", (event) => {
      mascotEl.classList.remove("dragover");
      const paths = event.payload?.paths;
      if (paths && paths.length > 0) {
        organizeFolder(paths[0]);
      }
    });

    window.__TAURI__.event.listen("tauri://drag-enter", () => {
      mascotEl.classList.add("dragover");
      sfx.playBrush();
    });

    window.__TAURI__.event.listen("tauri://drag-leave", () => {
      mascotEl.classList.remove("dragover");
    });
  }

  // ─── Click to Browse (Alternative) ───
  if (folderInput) {
    folderInput.addEventListener("change", (e) => {
      const files = e.target.files;
      if (files && files.length > 0) {
        const fullPath = files[0].path || files[0].webkitRelativePath || files[0].name;
        organizeFolder(fullPath);
      }
    });
  }

  // ─── Mascot Query / Prompt Handling ───
  function handleQuery() {
    const text = bubbleInput?.value?.trim();
    if (!text) return;

    sfx.playPop();
    mascot.setAppState({ attention: true });
    if (bubbleMsg) bubbleMsg.textContent = `Analyzing: "${text}"...`;
    if (bubbleInput) bubbleInput.value = "";

    setTimeout(() => {
      mascot.setAppState({ attention: false });
      if (bubbleMsg) bubbleMsg.textContent = "Ready to organize! Drop any directory or file.";
    }, 1800);
  }

  bubbleSendBtn?.addEventListener("click", handleQuery);
  bubbleInput?.addEventListener("keydown", (e) => {
    if (e.key === "Enter") handleQuery();
  });
})();
