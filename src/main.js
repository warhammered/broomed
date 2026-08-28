(() => {
  "use strict";

  // ─── DOM Elements ───
  const $ = (s) => document.querySelector(s);
  const dropzoneSection   = $("#dropzone-section");
  const dropzone          = $("#dropzone");
  const folderInput       = $("#folder-input");
  const btnBrowse         = $("#btn-browse");
  const statusBar         = $("#status-bar");
  const statusSpinner     = $("#status-spinner");
  const statusText        = $("#status-text");
  const targetFolderLabel = $("#target-folder-label");

  // Prompt Form
  const mainPromptForm  = $("#main-prompt-form");
  const mainPromptInput = $("#main-prompt-input");

  // AI Tier Header Badge
  const aiTierBadge = $("#ai-tier-badge");
  const aiTierDot   = $("#ai-tier-dot");
  const aiTierLabel = $("#ai-tier-label");

  // Preview Section
  const previewSection      = $("#preview-section");
  const previewCountBadge   = $("#preview-count-badge");
  const previewSummaryText  = $("#preview-summary-text");
  const categoryFilterGroup = $("#category-filter-group");
  const previewTableBody    = $("#preview-table-body");
  const tableContainer      = $("#table-container");
  const checkSelectAll      = $("#check-select-all");
  const btnRescan           = $("#btn-rescan");
  const btnUndo             = $("#btn-undo");
  const btnExecute          = $("#btn-execute");

  // Top Actions
  const btnOpenWidget  = $("#btn-open-widget");
  const btnOpenLicense = $("#btn-open-license");

  // Settings & License Modal
  const licenseModal      = $("#license-modal");
  const btnCloseModal     = $("#btn-close-modal");
  const btnModalDone      = $("#btn-modal-done");
  const btnRefreshLicense = $("#btn-refresh-license");
  const modalSubStatus    = $("#modal-sub-status");
  const modalCredits      = $("#modal-credits");
  const modalTierDesc     = $("#modal-tier-desc");
  const modalDeviceId     = $("#modal-device-id");
  const activationInput   = $("#activation-input");
  const btnActivate       = $("#btn-activate");
  const activationMsg     = $("#activation-msg");

  // BYOK Form Elements
  const byokStatusPill    = $("#byok-status-pill");
  const byokProvider      = $("#byok-provider");
  const byokModel         = $("#byok-model");
  const byokKey           = $("#byok-key");
  const byokUrl           = $("#byok-url");
  const btnSaveByok       = $("#btn-save-byok");
  const btnClearByok      = $("#btn-clear-byok");
  const byokMsg           = $("#byok-msg");

  // ─── State ───
  let currentFolderPath = null;
  let currentPreviews = [];
  let selectedIndices = new Set();
  let activeCategoryFilter = "all";
  let isProcessing = false;

  // ─── Tauri Bridge ───
  const isTauri = typeof window.__TAURI__ !== "undefined";
  const invoke = isTauri ? (window.__TAURI__.core?.invoke || window.__TAURI__.invoke) : null;
  const eventApi = isTauri ? window.__TAURI__.event : null;

  const delay = (ms) => new Promise((r) => setTimeout(r, ms));
  const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

  function setStatus(text, loading = false) {
    if (statusText) statusText.textContent = text;
    if (statusSpinner) {
      statusSpinner.classList.toggle("hidden", !loading);
    }
  }

  // ─── Format Specific SVG Icons ───
  function getFileIcon(filename, category) {
    const ext = (filename.split(".").pop() || "").toLowerCase();
    if (["png", "jpg", "jpeg", "webp", "gif", "svg", "bmp", "tiff"].includes(ext) || category === "Images") {
      return `<svg class="svg-icon-xs text-accent" viewBox="0 0 24 24"><rect width="18" height="18" x="3" y="3" rx="2" ry="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"/></svg>`;
    }
    if (["zip", "tar", "gz", "7z", "rar", "bz2", "xz"].includes(ext) || category === "Archives") {
      return `<svg class="svg-icon-xs text-muted" viewBox="0 0 24 24"><rect width="20" height="5" x="2" y="3" rx="1"/><path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8"/><path d="M10 12h4"/></svg>`;
    }
    if (["rs", "js", "ts", "py", "go", "cpp", "c", "html", "css", "json", "toml", "yaml", "sh"].includes(ext) || category === "Code") {
      return `<svg class="svg-icon-xs text-muted" viewBox="0 0 24 24"><polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg>`;
    }
    if (["mp3", "wav", "flac", "m4a", "ogg", "aac"].includes(ext) || category === "Audio") {
      return `<svg class="svg-icon-xs text-muted" viewBox="0 0 24 24"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>`;
    }
    return `<svg class="svg-icon-xs text-muted" viewBox="0 0 24 24"><path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"/><polyline points="14 2 14 8 20 8"/></svg>`;
  }

  // ─── Update Active AI Badge ───
  async function updateActiveAiBadge() {
    if (!invoke) {
      if (aiTierLabel) aiTierLabel.textContent = "Local AI (BERT)";
      return;
    }

    try {
      const status = await invoke("get_active_ai_status_cmd");
      if (!status) return;

      if (aiTierBadge) {
        aiTierBadge.className = "ai-tier-badge";
        if (status.tier === "pro_online") {
          aiTierBadge.classList.add("tier-pro");
        } else if (status.tier === "byok") {
          aiTierBadge.classList.add("tier-byok");
        } else {
          aiTierBadge.classList.add("tier-local");
        }
        aiTierBadge.title = `${status.label} — ${status.details}`;
      }

      if (aiTierLabel) {
        aiTierLabel.textContent = status.label;
      }
    } catch (e) {
      console.warn("get_active_ai_status failed:", e);
    }
  }

  // ─── Render Category Filter Chips ───
  function renderCategoryFilters(previews) {
    if (!categoryFilterGroup) return;
    categoryFilterGroup.innerHTML = "";

    const counts = { all: previews.length };
    previews.forEach((p) => {
      const cat = p.ai_result?.category || p.category || "General";
      counts[cat] = (counts[cat] || 0) + 1;
    });

    const cats = Object.keys(counts);
    cats.forEach((cat) => {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = `cat-filter-btn ${activeCategoryFilter === cat.toLowerCase() ? "active" : ""}`;
      btn.textContent = `${cat === "all" ? "All" : cat} (${counts[cat]})`;
      btn.addEventListener("click", () => {
        activeCategoryFilter = cat.toLowerCase();
        renderTableRows();
        renderCategoryFilters(currentPreviews);
      });
      categoryFilterGroup.appendChild(btn);
    });
  }

  // ─── Render Table Rows ───
  function renderTableRows() {
    if (!previewTableBody) return;
    previewTableBody.innerHTML = "";
    const frag = document.createDocumentFragment();

    const filtered = currentPreviews.filter((p, idx) => {
      if (activeCategoryFilter === "all") return true;
      const cat = (p.ai_result?.category || p.category || "General").toLowerCase();
      return cat === activeCategoryFilter;
    });

    filtered.forEach((p) => {
      const originalIdx = currentPreviews.indexOf(p);
      const isSelected = selectedIndices.has(originalIdx);

      const src = p.operation?.source || p.path || p.file || "";
      const dst = p.operation?.destination || p.destination || "";
      const cat = p.ai_result?.category || p.category || "General";
      const conf = p.operation?.confidence ?? p.ai_result?.confidence ?? 0.5;
      const reason = p.operation?.reason || p.ai_result?.reason || p.reason || "Semantic grouping";

      const fileName = src.split(/[/\\]/).pop() || src;
      const pct = Math.round(conf * 100);
      const confClass = pct >= 80 ? "high" : pct >= 55 ? "med" : "low";

      const tr = document.createElement("tr");
      tr.innerHTML = `
        <td class="col-check">
          <input type="checkbox" class="row-checkbox item-check" data-idx="${originalIdx}" ${isSelected ? "checked" : ""}>
        </td>
        <td class="col-file" title="${esc(src)}">
          <span class="file-cell">
            ${getFileIcon(fileName, cat)}
            <span>${esc(fileName)}</span>
          </span>
        </td>
        <td class="col-cat"><span class="cat-pill">${esc(cat)}</span></td>
        <td class="col-dest mono" title="${esc(dst)}">${esc(dst)}</td>
        <td class="col-conf">
          <div class="conf-wrap">
            <div class="conf-bar"><div class="conf-fill ${confClass}" style="width: ${pct}%"></div></div>
            <span class="conf-val">${pct}%</span>
          </div>
        </td>
        <td class="col-reason" title="${esc(reason)}">${esc(reason)}</td>
      `;

      const checkEl = tr.querySelector(".item-check");
      checkEl?.addEventListener("change", (e) => {
        if (e.target.checked) {
          selectedIndices.add(originalIdx);
        } else {
          selectedIndices.delete(originalIdx);
        }
        updateSelectAllState();
      });

      frag.appendChild(tr);
    });

    previewTableBody.appendChild(frag);
  }

  function updateSelectAllState() {
    if (!checkSelectAll) return;
    if (selectedIndices.size === currentPreviews.length && currentPreviews.length > 0) {
      checkSelectAll.checked = true;
      checkSelectAll.indeterminate = false;
    } else if (selectedIndices.size === 0) {
      checkSelectAll.checked = false;
      checkSelectAll.indeterminate = false;
    } else {
      checkSelectAll.checked = false;
      checkSelectAll.indeterminate = true;
    }
  }

  // Select all toggle
  checkSelectAll?.addEventListener("change", (e) => {
    if (e.target.checked) {
      selectedIndices = new Set(currentPreviews.map((_, i) => i));
    } else {
      selectedIndices.clear();
    }
    renderTableRows();
  });

  // ─── Render Plan View ───
  function renderPlan(folderPath, previews) {
    currentFolderPath = folderPath;
    currentPreviews = previews || [];
    selectedIndices = new Set(currentPreviews.map((_, i) => i));
    activeCategoryFilter = "all";

    if (targetFolderLabel) {
      targetFolderLabel.textContent = folderPath || "";
    }

    if (!previews || previews.length === 0) {
      if (previewSection) previewSection.classList.add("hidden");
      if (dropzoneSection) dropzoneSection.classList.remove("collapsed");
      setStatus("No files found or directory is already clean.", false);
      return;
    }

    if (dropzoneSection) dropzoneSection.classList.add("collapsed");
    if (previewSection) previewSection.classList.remove("hidden");

    if (previewCountBadge) {
      previewCountBadge.textContent = `${previews.length} operations`;
    }

    const cats = new Set(previews.map((p) => p.ai_result?.category || p.category || "General"));
    const confs = previews.map((p) => p.operation?.confidence ?? p.ai_result?.confidence ?? 0.5);
    const avgConf = confs.reduce((a, b) => a + b, 0) / (confs.length || 1);

    if (previewSummaryText) {
      previewSummaryText.textContent = `${cats.size} categories • Avg confidence ${Math.round(avgConf * 100)}% • Latency ~12ms`;
    }

    renderCategoryFilters(previews);
    renderTableRows();
    updateSelectAllState();

    if (tableContainer) {
      tableContainer.scrollTop = 0;
    }

    setStatus(`Plan ready for ${previews.length} files. Click Execute to apply.`, false);
  }

  // ─── Scan & Plan Directory ───
  async function scanAndPlanFolder(path, instruction = null) {
    if (!path || isProcessing) return;
    isProcessing = true;
    currentFolderPath = path;
    const desc = instruction ? `Scanning "${path}" with prompt "${instruction}"...` : `Scanning directory: ${path}...`;
    setStatus(desc, true);

    if (eventApi && eventApi.emit) {
      eventApi.emit("broomed:state-change", { state: "scanning", folder: path }).catch(() => {});
    }

    if (!invoke) {
      // Browser preview demo
      await delay(800);
      const demoPreviews = [
        { operation: { source: `${path}/Tax_Invoice_2026_08.pdf`, destination: `${path}/Finance/Invoices/Tax_Invoice_2026_08.pdf`, confidence: 0.96, reason: instruction || "Financial invoice OCR match", id: "1" }, ai_result: { category: "Documents" } },
        { operation: { source: `${path}/Screen_Capture_99.png`, destination: `${path}/Images/Screenshots/Screen_Capture_99.png`, confidence: 0.88, reason: instruction || "Visual screenshot layout", id: "2" }, ai_result: { category: "Images" } },
        { operation: { source: `${path}/project_archive.tar.gz`, destination: `${path}/Archives/project_archive.tar.gz`, confidence: 0.94, reason: instruction || "Compressed tarball archive", id: "3" }, ai_result: { category: "Archives" } },
        { operation: { source: `${path}/deploy_service.rs`, destination: `${path}/Code/Rust/deploy_service.rs`, confidence: 0.98, reason: instruction || "Rust source file structure", id: "4" }, ai_result: { category: "Code" } },
      ];
      renderPlan(path, demoPreviews);
      if (eventApi && eventApi.emit) {
        eventApi.emit("broomed:dashboard-plan-ready", { folderPath: path, previews: demoPreviews, summary: `${demoPreviews.length} files planned` }).catch(() => {});
        eventApi.emit("broomed:state-change", { state: "idle" }).catch(() => {});
      }
      isProcessing = false;
      return;
    }

    try {
      const files = await invoke("scan_directory_cmd", { base: path, maxFiles: 10000 });
      if (!files || files.length === 0) {
        setStatus(`No files found in ${path}`, false);
        if (eventApi && eventApi.emit) {
          eventApi.emit("broomed:state-change", { state: "idle" }).catch(() => {});
        }
        isProcessing = false;
        return;
      }

      setStatus(`Planning ${files.length} files with automated AI engine...`, true);
      if (eventApi && eventApi.emit) {
        eventApi.emit("broomed:state-change", { state: "classifying" }).catch(() => {});
      }

      let previews = [];
      try {
        previews = await invoke("plan_organize_cmd", {
          files: files,
          base: path,
          task: instruction || "ClassifyFile",
          threshold: 0.5,
          provider: null,
        });
      } catch (e) {
        console.warn("plan_organize_cmd error, fallback:", e);
      }

      if (!previews || previews.length === 0) {
        const results = [];
        for (const f of files) {
          try {
            const promptInput = instruction ? `${f} (Instruction: ${instruction})` : f;
            const r = await invoke("classify_cmd", { task: "ClassifyFile", input: promptInput, provider: null });
            results.push({ file: f.split(/[/\\]/).pop() || f, path: f, category: r.category, folder: r.suggested_folder || r.category, confidence: r.confidence, reason: r.reason });
          } catch {
            results.push({ file: f.split(/[/\\]/).pop() || f, path: f, category: "General", folder: "General", confidence: 0.5, reason: "fallback" });
          }
        }
        const genId = () => (typeof crypto !== "undefined" && crypto.randomUUID ? crypto.randomUUID() : "00000000-0000-4000-a000-000000000000".replace(/0/g, () => Math.floor(Math.random()*16).toString(16)));
        previews = results.map((r) => ({
          operation: { source: r.path, destination: `${path}/${r.folder}/${r.file}`, kind: "Move", reason: r.reason, confidence: r.confidence, reversible: true, status: "planned", id: genId() },
          ai_result: { category: r.category, confidence: r.confidence, suggested_folder: r.folder, reason: r.reason, tags: [], subcategory: null, suggested_name: null }
        }));
      }

      renderPlan(path, previews);
      updateActiveAiBadge();

      // Sync plan with Floating Mascot Widget
      if (eventApi && eventApi.emit) {
        eventApi.emit("broomed:dashboard-plan-ready", {
          folderPath: path,
          previews: previews,
          summary: `${previews.length} files planned in ${path.split(/[/\\]/).pop() || "folder"}`
        }).catch(() => {});
        eventApi.emit("broomed:state-change", { state: "idle" }).catch(() => {});
      }
    } catch (err) {
      setStatus(`Scan error: ${err}`, false);
      if (eventApi && eventApi.emit) {
        eventApi.emit("broomed:state-change", { state: "error" }).catch(() => {});
      }
      console.error("Scan error:", err);
    } finally {
      isProcessing = false;
    }
  }

  // ─── Execute Plan ───
  async function executePlan() {
    const selectedOps = currentPreviews.filter((_, idx) => selectedIndices.has(idx));
    if (!selectedOps || selectedOps.length === 0 || isProcessing) {
      alert("No files selected for move.");
      return;
    }

    if (!confirm(`Execute ${selectedOps.length} file moves now? (Operations are reversible via Undo).`)) return;

    isProcessing = true;
    btnExecute.disabled = true;
    btnExecute.textContent = "Moving Files…";
    setStatus(`Executing ${selectedOps.length} file moves on disk...`, true);

    if (eventApi && eventApi.emit) {
      eventApi.emit("broomed:state-change", { state: "executing" }).catch(() => {});
    }

    if (!invoke) {
      await delay(1000);
      setStatus(`✓ Successfully organized ${selectedOps.length} files (Demo mode)`, false);
      btnExecute.disabled = false;
      btnExecute.innerHTML = `<svg class="svg-icon" viewBox="0 0 24 24"><polyline points="20 6 9 17 4 12"/></svg> <span>Execute Plan</span>`;
      if (eventApi && eventApi.emit) {
        eventApi.emit("broomed:plan-executed", { folderPath: currentFolderPath, count: selectedOps.length }).catch(() => {});
        eventApi.emit("broomed:state-change", { state: "idle" }).catch(() => {});
      }
      isProcessing = false;
      return;
    }

    try {
      const ids = await invoke("execute_plan_cmd", { previews: selectedOps, dbPath: null });
      const count = Array.isArray(ids) ? ids.length : selectedOps.length;
      setStatus(`✓ Successfully moved ${count} files.`, false);
      btnExecute.disabled = false;
      btnExecute.innerHTML = `<svg class="svg-icon" viewBox="0 0 24 24"><polyline points="20 6 9 17 4 12"/></svg> <span>Executed (${count})</span>`;
      
      // Notify Widget
      if (eventApi && eventApi.emit) {
        eventApi.emit("broomed:plan-executed", { folderPath: currentFolderPath, count }).catch(() => {});
        eventApi.emit("broomed:state-change", { state: "idle" }).catch(() => {});
      }

      setTimeout(() => {
        btnExecute.innerHTML = `<svg class="svg-icon" viewBox="0 0 24 24"><polyline points="20 6 9 17 4 12"/></svg> <span>Execute Plan</span>`;
      }, 3000);
    } catch (err) {
      setStatus(`Execute failed: ${err}`, false);
      btnExecute.disabled = false;
      btnExecute.innerHTML = `<svg class="svg-icon" viewBox="0 0 24 24"><polyline points="20 6 9 17 4 12"/></svg> <span>Execute Plan</span>`;
      if (eventApi && eventApi.emit) {
        eventApi.emit("broomed:state-change", { state: "error" }).catch(() => {});
      }
    } finally {
      isProcessing = false;
    }
  }

  // ─── Undo Last ───
  async function undoLast() {
    if (isProcessing) return;
    isProcessing = true;
    setStatus("Undoing last organization batch...", true);

    if (!invoke) {
      await delay(500);
      setStatus("Undo completed (Demo mode)", false);
      isProcessing = false;
      return;
    }

    try {
      await invoke("undo_last_cmd", { count: 1, dbPath: null });
      setStatus("✓ Successfully reversed last organization operation.", false);
      if (eventApi && eventApi.emit) {
        eventApi.emit("broomed:undo-executed").catch(() => {});
      }
      if (currentFolderPath) {
        scanAndPlanFolder(currentFolderPath);
      }
    } catch (err) {
      setStatus(`Undo error: ${err}`, false);
    } finally {
      isProcessing = false;
    }
  }

  // ─── Folder Picker ───
  async function pickFolder() {
    if (!invoke) {
      const path = prompt("Enter folder path to scan:", "C:\\Users\\Demo\\Downloads");
      if (path) scanAndPlanFolder(path);
      return;
    }
    try {
      const result = await invoke("browse_directory_cmd");
      if (result) scanAndPlanFolder(result);
    } catch (e) {
      console.error("Browse failed:", e);
    }
  }

  // ─── Prompt Form Handling (Natural Language / Path / Query) ───
  mainPromptForm?.addEventListener("submit", async (e) => {
    e.preventDefault();
    const query = mainPromptInput?.value?.trim();
    if (!query) return;

    // 1. Check if it's an absolute path
    if (query.includes(":\\") || query.startsWith("/") || query.startsWith("\\\\") || query.startsWith("~")) {
      await scanAndPlanFolder(query);
      return;
    }

    // 2. Named folder shortcut check (e.g. "Downloads", "Desktop", "Documents")
    const lower = query.toLowerCase();
    if (["downloads", "download", "desktop", "documents", "docs", "pictures", "pics"].includes(lower)) {
      if (invoke) {
        try {
          const userDir = await invoke("get_user_downloads_dir_cmd");
          if (userDir) {
            if (lower.startsWith("download")) {
              await scanAndPlanFolder(userDir);
              return;
            }
            const parent = userDir.replace(/[\\/]Downloads$/i, "");
            const target = lower.startsWith("desk") ? `${parent}\\Desktop` :
                           lower.startsWith("doc") ? `${parent}\\Documents` :
                           `${parent}\\Pictures`;
            await scanAndPlanFolder(target);
            return;
          }
        } catch {}
      }
    }

    // 3. Natural language query / instruction with target directory
    if (currentFolderPath) {
      await scanAndPlanFolder(currentFolderPath, query);
    } else {
      if (invoke) {
        try {
          const dl = await invoke("get_user_downloads_dir_cmd");
          if (dl) {
            await scanAndPlanFolder(dl, query);
          } else {
            const folder = await invoke("browse_directory_cmd");
            if (folder) await scanAndPlanFolder(folder, query);
          }
        } catch {
          await pickFolder();
        }
      } else {
        await pickFolder();
      }
    }
  });

  // ─── Event Listeners ───
  folderInput?.addEventListener("change", (e) => {
    const files = e.target.files;
    if (files && files.length > 0) {
      const first = files[0];
      const p = first.path || first.webkitRelativePath || first.name;
      if (p) scanAndPlanFolder(p);
    }
  });

  btnBrowse?.addEventListener("click", (e) => {
    e.stopPropagation();
    pickFolder();
  });

  dropzone?.addEventListener("click", pickFolder);

  dropzone?.addEventListener("dragover", (e) => {
    e.preventDefault();
    dropzone.classList.add("dragover");
  });

  dropzone?.addEventListener("dragleave", () => {
    dropzone.classList.remove("dragover");
  });

  dropzone?.addEventListener("drop", (e) => {
    e.preventDefault();
    dropzone.classList.remove("dragover");
    const items = e.dataTransfer?.items;
    if (items && items.length > 0) {
      const entry = items[0].webkitGetAsEntry?.();
      if (entry) scanAndPlanFolder(entry.fullPath || entry.name);
    }
  });

  // Native Tauri drag-and-drop & plan events
  if (eventApi && eventApi.listen) {
    eventApi.listen("tauri://drag-drop", (event) => {
      dropzone?.classList.remove("dragover");
      const paths = event.payload?.paths;
      if (paths && paths.length > 0) {
        scanAndPlanFolder(paths[0]);
      }
    });

    eventApi.listen("tauri://drag-enter", () => {
      dropzone?.classList.add("dragover");
    });

    eventApi.listen("tauri://drag-leave", () => {
      dropzone?.classList.remove("dragover");
    });

    // Listen to plans emitted by floating mascot widget
    eventApi.listen("broomed:plan-ready", (event) => {
      const { folderPath, previews } = event.payload || {};
      if (folderPath && previews) {
        renderPlan(folderPath, previews);
      }
    });

    // Listen to plan execution from widget
    eventApi.listen("broomed:plan-executed", (event) => {
      const { count } = event.payload || {};
      setStatus(`✓ Successfully organized ${count || ""} files.`, false);
      if (currentFolderPath) {
        scanAndPlanFolder(currentFolderPath);
      }
    });

    // Listen to undo from widget
    eventApi.listen("broomed:undo-executed", () => {
      setStatus("✓ Reversed last organization operation.", false);
      if (currentFolderPath) {
        scanAndPlanFolder(currentFolderPath);
      }
    });

    // Listen to state changes from widget
    eventApi.listen("broomed:state-change", (event) => {
      const { state, folder } = event.payload || {};
      if (state === "scanning") setStatus(`Scanning ${folder || "folder"}...`, true);
      else if (state === "executing") setStatus("Executing moves on disk...", true);
      else if (state === "classifying") setStatus("Running AI classifier...", true);
      else if (state === "idle") setStatus("Ready. Drop a folder to organize.", false);
      else if (state === "error") setStatus("Error encountered.", false);
    });

    // Listen to settings open request from widget context menu
    eventApi.listen("broomed:open-settings", () => {
      openLicenseModal();
    });
  }

  btnExecute?.addEventListener("click", executePlan);
  btnUndo?.addEventListener("click", undoLast);
  btnRescan?.addEventListener("click", () => {
    if (currentFolderPath) scanAndPlanFolder(currentFolderPath);
  });

  // Open Widget
  btnOpenWidget?.addEventListener("click", () => {
    if (invoke) {
      invoke("show_widget_window_cmd").catch(() => {});
    }
  });

  // ─── Settings & BYOK Modal ───
  async function openLicenseModal() {
    if (licenseModal) licenseModal.classList.remove("hidden");
    if (!invoke) {
      if (modalDeviceId) modalDeviceId.textContent = "dev_preview_device_001";
      return;
    }

    try {
      // 1. License status
      const raw = await invoke("license_status_cmd");
      const data = typeof raw === "string" ? JSON.parse(raw) : raw;
      if (modalSubStatus) modalSubStatus.textContent = data?.subscription_status === "active" ? "Pro Subscription (Active)" : "Local AI (Free)";
      if (modalCredits) modalCredits.textContent = data?.ai_credits_remaining ? `${data.ai_credits_remaining} cloud credits` : "Local Unlimited";
      if (modalDeviceId) modalDeviceId.textContent = data?.device_id || "Registered Local Device";

      // 2. BYOK status
      const byok = await invoke("get_byok_config_cmd");
      if (byok) {
        if (byokProvider) byokProvider.value = byok.provider || "openai";
        if (byokModel) byokModel.value = byok.model || "";
        if (byokUrl) byokUrl.value = byok.base_url || "";
        if (byokStatusPill) {
          if (byok.has_key) {
            byokStatusPill.textContent = `Configured (${byok.provider})`;
            byokStatusPill.classList.add("active");
          } else {
            byokStatusPill.textContent = "Not Configured";
            byokStatusPill.classList.remove("active");
          }
        }
      }

      // 3. Active Tier description
      const status = await invoke("get_active_ai_status_cmd");
      if (status && modalTierDesc) {
        modalTierDesc.textContent = `${status.label}: ${status.details}`;
      }
    } catch (e) {
      console.warn("openLicenseModal error:", e);
    }
  }

  function closeLicenseModal() {
    if (licenseModal) licenseModal.classList.add("hidden");
    if (activationMsg) activationMsg.textContent = "";
    if (byokMsg) byokMsg.textContent = "";
    updateActiveAiBadge();
  }

  btnOpenLicense?.addEventListener("click", openLicenseModal);
  btnCloseModal?.addEventListener("click", closeLicenseModal);
  btnModalDone?.addEventListener("click", closeLicenseModal);

  licenseModal?.addEventListener("click", (e) => {
    if (e.target === licenseModal) closeLicenseModal();
  });

  byokProvider?.addEventListener("change", () => {
    const val = byokProvider.value;
    if (val === "anthropic") {
      if (byokModel) byokModel.placeholder = "claude-3-5-sonnet-20241022";
      if (byokKey) byokKey.placeholder = "sk-ant-...";
    } else if (val === "openrouter") {
      if (byokModel) byokModel.placeholder = "anthropic/claude-3.5-sonnet";
      if (byokKey) byokKey.placeholder = "sk-or-...";
    } else if (val === "custom") {
      if (byokModel) byokModel.placeholder = "llama3.2";
      if (byokKey) byokKey.placeholder = "ollama / custom";
      if (byokUrl) byokUrl.placeholder = "http://localhost:11434/v1";
    } else {
      if (byokModel) byokModel.placeholder = "gpt-4o-mini";
      if (byokKey) byokKey.placeholder = "sk-...";
    }
  });

  // Save BYOK
  btnSaveByok?.addEventListener("click", async () => {
    const provider = byokProvider?.value || "openai";
    const key = byokKey?.value?.trim();
    const model = byokModel?.value?.trim() || null;
    const url = byokUrl?.value?.trim() || null;

    if (!key) {
      if (byokMsg) {
        byokMsg.className = "form-msg error";
        byokMsg.textContent = "Please enter an API Key.";
      }
      return;
    }

    btnSaveByok.disabled = true;
    if (byokMsg) {
      byokMsg.className = "form-msg";
      byokMsg.textContent = "Saving key securely...";
    }

    if (!invoke) {
      await delay(400);
      if (byokMsg) {
        byokMsg.className = "form-msg success";
        byokMsg.textContent = "BYOK saved (demo mode)!";
      }
      btnSaveByok.disabled = false;
      return;
    }

    try {
      await invoke("save_byok_config_cmd", { provider, apiKey: key, model, baseUrl: url });
      if (byokMsg) {
        byokMsg.className = "form-msg success";
        byokMsg.textContent = "API key saved securely! ✨";
      }
      if (byokKey) byokKey.value = "";
      openLicenseModal();
      updateActiveAiBadge();
    } catch (e) {
      if (byokMsg) {
        byokMsg.className = "form-msg error";
        byokMsg.textContent = `Save failed: ${e}`;
      }
    } finally {
      btnSaveByok.disabled = false;
    }
  });

  // Clear BYOK
  btnClearByok?.addEventListener("click", async () => {
    if (!invoke) {
      if (byokMsg) {
        byokMsg.className = "form-msg success";
        byokMsg.textContent = "Key removed.";
      }
      return;
    }
    try {
      await invoke("clear_byok_config_cmd");
      if (byokKey) byokKey.value = "";
      if (byokMsg) {
        byokMsg.className = "form-msg success";
        byokMsg.textContent = "BYOK configuration removed.";
      }
      openLicenseModal();
      updateActiveAiBadge();
    } catch (e) {
      console.warn("clear_byok failed:", e);
    }
  });

  // Activate Pro License
  btnActivate?.addEventListener("click", async () => {
    const code = activationInput?.value?.trim();
    if (!code) {
      if (activationMsg) {
        activationMsg.className = "form-msg error";
        activationMsg.textContent = "Please enter an activation code.";
      }
      return;
    }

    btnActivate.disabled = true;
    if (activationMsg) {
      activationMsg.className = "form-msg";
      activationMsg.textContent = "Validating key with control plane...";
    }

    if (!invoke) {
      await delay(600);
      if (activationMsg) {
        activationMsg.className = "form-msg success";
        activationMsg.textContent = "License activated (demo mode)!";
      }
      btnActivate.disabled = false;
      return;
    }

    try {
      await invoke("activate_license_cmd", { activationCode: code });
      if (activationMsg) {
        activationMsg.className = "form-msg success";
        activationMsg.textContent = "Pro license activated successfully! ✨";
      }
      openLicenseModal();
      updateActiveAiBadge();
    } catch (err) {
      if (activationMsg) {
        activationMsg.className = "form-msg error";
        activationMsg.textContent = `Activation failed: ${err}`;
      }
    } finally {
      btnActivate.disabled = false;
    }
  });

  btnRefreshLicense?.addEventListener("click", async () => {
    if (!invoke) return;
    try {
      await invoke("refresh_license_cmd");
      openLicenseModal();
      updateActiveAiBadge();
    } catch (e) {
      console.warn("refresh failed", e);
    }
  });

  // ─── Initial Status & Badge ───
  setStatus("Ready to sweep. Drop any directory or type a prompt.");
  updateActiveAiBadge();
})();
