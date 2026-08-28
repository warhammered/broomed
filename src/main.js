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
  const invoke = isTauri ? (window.__TAURI__.core?.invoke || window.__TAURI__.invoke) : null;
  const appWindow = isTauri && window.__TAURI__.window ? window.__TAURI__.window.getCurrentWindow?.() : null;

  // ─── Native Window Dragging ───
  if (appWindow && petContainer) {
    petContainer.addEventListener("mousedown", (e) => {
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

  // ─── Context Menu Controls ───
  function showContextMenu(x, y) {
    if (!contextMenu) return;
    contextMenu.classList.remove("hidden");
    contextMenu.style.left = `${Math.min(x, 90)}px`;
    contextMenu.style.top = `${Math.min(y, 100)}px`;
    sfx.playPop();
  }

  function hideContextMenu() {
    if (contextMenu) contextMenu.classList.add("hidden");
  }

  petContainer?.addEventListener("contextmenu", (e) => {
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
      return;
    }

    if (licenseStatusMsg) {
      licenseStatusMsg.className = "license-status-msg";
      licenseStatusMsg.textContent = "Validating with edge control plane...";
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
    } else {
      if (licenseStatusMsg) {
        licenseStatusMsg.className = "license-status-msg success";
        licenseStatusMsg.textContent = "License activated (demo mode)! ✨";
      }
      sfx.playChime();
      setTimeout(hideLicenseView, 1500);
    }
  });

  // ─── Drag and Drop Handling (Desktop & Browser) ───
  petContainer?.addEventListener("dragover", (e) => {
    e.preventDefault();
    mascotEl?.classList.add("dragover");
  });

  petContainer?.addEventListener("dragleave", (e) => {
    e.preventDefault();
    mascotEl?.classList.remove("dragover");
  });

  petContainer?.addEventListener("drop", (e) => {
    e.preventDefault();
    mascotEl?.classList.remove("dragover");

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
      mascotEl?.classList.remove("dragover");
      const paths = event.payload?.paths;
      if (paths && paths.length > 0) {
        organizeFolder(paths[0]);
      }
    });

    window.__TAURI__.event.listen("tauri://drag-enter", () => {
      mascotEl?.classList.add("dragover");
      sfx.playBrush();
    });

    window.__TAURI__.event.listen("tauri://drag-leave", () => {
      mascotEl?.classList.remove("dragover");
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
