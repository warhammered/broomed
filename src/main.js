(() => {
  "use strict";

  // ─── Constants ───
  const RENDER_BATCH = 200;
  const CONF_HIGH = 0.90;
  const CONF_MED = 0.70;

  // ─── State ───
  let state = "idle";
  let currentAiMode = "local";
  let previewData = [];
  let folderPath = null;
  let licenseData = null;
  let deviceData = null;

  // ─── DOM ───
  const $ = (s) => document.querySelector(s);
  const mascot = $("#mascot");
  const statusText = $("#status-text");
  const statusSpinner = $("#status-spinner");
  const dropzone = $("#dropzone");
  const folderInput = $("#folder-input");
  const preview = $("#preview");
  const previewBody = $("#preview-body");
  const previewCount = $("#preview-count");
  const previewSummary = $("#preview-summary");
  const previewTableWrap = $("#preview-table-wrap");
  const btnPreview = $("#btn-preview");
  const btnExecute = $("#btn-execute");
  const btnUndo = $("#btn-undo");
  const btnRescan = $("#btn-rescan");
  const app = $("#app");

  // License & Modal DOM
  const btnLicenseModal = $("#btn-license-modal");
  const licenseBadgeText = $("#license-badge-text");
  const licenseIndicatorDot = $("#license-indicator-dot");
  const licenseModal = $("#license-modal");
  const btnCloseModal = $("#btn-close-modal");
  const btnModalDone = $("#btn-modal-done");
  const modalLicenseState = $("#modal-license-state");
  const modalOnlineEnabled = $("#modal-online-enabled");
  const modalExpiresAt = $("#modal-expires-at");
  const modalDeviceId = $("#modal-device-id");
  const modalPublicKey = $("#modal-public-key");
  const activationInput = $("#activation-code-input");
  const btnActivate = $("#btn-activate-license");
  const btnActivateSpinner = $("#btn-activate-spinner");
  const btnRefresh = $("#btn-refresh-license");
  const btnRefreshSpinner = $("#btn-refresh-spinner");
  const modalAlert = $("#modal-alert");

  // ─── Tauri Bridge ───
  const isTauri = typeof window.__TAURI__ !== "undefined";
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

  // ─── Helpers ───
  const delay = (ms) => new Promise((r) => setTimeout(r, ms));
  const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
  const fileName = (p) => p.split(/[/\\]/).pop() || p;

  function showAlert(msg, type = "info") {
    modalAlert.className = "modal-alert alert-" + type;
    modalAlert.textContent = msg;
    modalAlert.classList.remove("hidden");
  }

  function clearAlert() {
    modalAlert.className = "modal-alert hidden";
    modalAlert.textContent = "";
  }

  // ─── State Machine ───
  function setState(s) {
    state = s;

    // Mascot
    const mascotMap = {
      idle: "idle", scanning: "scanning", classifying: "thinking",
      preview: "found", executing: "organizing", executed: "success", error: "error",
    };
    mascot.className = "mascot " + (mascotMap[s] || "idle");
    mascot.classList.toggle("shrink", s !== "idle");

    // Status
    const msgs = {
      idle: "Ready",
      scanning: "Scanning files\u2026",
      classifying: "Classifying with " + currentAiMode.toUpperCase() + " AI\u2026",
      preview: previewData.length + " files classified",
      executing: "Executing plan\u2026",
      executed: "Done!",
      error: statusText.textContent || "Something went wrong",
    };
    statusText.textContent = msgs[s] || "";
    statusSpinner.classList.toggle("hidden", !["scanning", "classifying", "executing"].includes(s));

    // Layout
    app.classList.toggle("wide", s === "preview" || s === "executing" || s === "executed");

    // Drop zone
    dropzone.classList.toggle("hidden", s !== "idle");

    // Preview
    preview.classList.toggle("hidden", s === "idle" || s === "scanning");

    // Buttons
    const canPreview = s === "idle" && folderPath;
    btnPreview.disabled = !canPreview && s !== "preview";
    btnPreview.querySelector(".btn-label").textContent = s === "preview" ? "Reclassify" : "Preview";
    btnPreview.classList.toggle("hidden", s === "idle" && !folderPath);
    btnExecute.disabled = s !== "preview";
    btnUndo.disabled = s !== "executed";

    // Spinners
    const previewSpinners = btnPreview.querySelectorAll(".btn-spinner");
    previewSpinners.forEach((el) => el.classList.toggle("hidden", s !== "classifying"));
    const execSpinners = btnExecute.querySelectorAll(".btn-spinner");
    execSpinners.forEach((el) => el.classList.toggle("hidden", s !== "executing"));
  }

  // ─── License & Device Info ───
  async function fetchLicenseInfo() {
    if (!invoke) {
      // Demo browser fallback
      licenseData = {
        state: "Inactive",
        online_ai_enabled: false,
        expires_at: null,
      };
      deviceData = {
        device_id: "demo-device-uuid-1234-5678",
        public_key: "Ed25519-DEMO-KEY-x7k9p...",
      };
      updateLicenseUI();
      return;
    }

    try {
      const [lic, dev] = await Promise.all([
        invoke("license_status_cmd"),
        invoke("get_device_info_cmd"),
      ]);
      licenseData = lic;
      deviceData = dev;
      updateLicenseUI();
    } catch (err) {
      console.warn("Failed to load license info:", err);
    }
  }

  function updateLicenseUI() {
    if (!licenseData) return;

    const st = licenseData.state || "Inactive";
    const online = licenseData.online_ai_enabled || false;

    // Header badge
    if (st === "Active") {
      licenseBadgeText.textContent = "Active Subscription";
      licenseIndicatorDot.className = "indicator-dot dot-green";
      modalLicenseState.className = "status-pill state-active";
      modalLicenseState.textContent = "Active";
    } else if (st === "OfflineGrace") {
      licenseBadgeText.textContent = "Offline Grace";
      licenseIndicatorDot.className = "indicator-dot dot-amber";
      modalLicenseState.className = "status-pill state-grace";
      modalLicenseState.textContent = "Offline Grace (72h)";
    } else if (st === "Expired") {
      licenseBadgeText.textContent = "License Expired";
      licenseIndicatorDot.className = "indicator-dot dot-red";
      modalLicenseState.className = "status-pill state-expired";
      modalLicenseState.textContent = "Expired";
    } else {
      licenseBadgeText.textContent = "Local AI Mode";
      licenseIndicatorDot.className = "indicator-dot dot-gray";
      modalLicenseState.className = "status-pill state-inactive";
      modalLicenseState.textContent = "Free / Local Only";
    }

    // Modal fields
    modalOnlineEnabled.textContent = online ? "Enabled (Pro)" : "Disabled (Local Fallback)";
    modalExpiresAt.textContent = licenseData.expires_at
      ? new Date(licenseData.expires_at * 1000).toLocaleDateString()
      : "No Expiration (Local)";

    if (deviceData) {
      modalDeviceId.textContent = deviceData.device_id || "—";
      modalPublicKey.textContent = deviceData.public_key || "—";
    }
  }

  // ─── Modal Actions ───
  function openLicenseModal() {
    clearAlert();
    licenseModal.classList.remove("hidden");
    fetchLicenseInfo();
    activationInput.focus();
  }

  function closeLicenseModal() {
    licenseModal.classList.add("hidden");
  }

  async function activateLicense() {
    const code = activationInput.value.trim();
    if (!code) {
      showAlert("Please enter an activation code", "error");
      return;
    }

    btnActivate.disabled = true;
    btnActivateSpinner.classList.remove("hidden");
    clearAlert();

    if (!invoke) {
      await delay(800);
      btnActivate.disabled = false;
      btnActivateSpinner.classList.add("hidden");
      licenseData.state = "Active";
      licenseData.online_ai_enabled = true;
      licenseData.expires_at = Math.floor(Date.now() / 1000) + (365 * 86400);
      updateLicenseUI();
      showAlert("Demo license activated successfully!", "success");
      activationInput.value = "";
      return;
    }

    try {
      const res = await invoke("activate_license_cmd", { code });
      licenseData = res;
      updateLicenseUI();
      showAlert("Device bound and license activated successfully!", "success");
      activationInput.value = "";
    } catch (err) {
      showAlert("Activation failed: " + String(err), "error");
    } finally {
      btnActivate.disabled = false;
      btnActivateSpinner.classList.add("hidden");
    }
  }

  async function refreshLicense() {
    btnRefresh.disabled = true;
    btnRefreshSpinner.classList.remove("hidden");
    clearAlert();

    if (!invoke) {
      await delay(600);
      btnRefresh.disabled = false;
      btnRefreshSpinner.classList.add("hidden");
      showAlert("License status is up to date", "info");
      return;
    }

    try {
      const res = await invoke("refresh_license_cmd");
      licenseData = res;
      updateLicenseUI();
      showAlert("Entitlement refreshed successfully", "success");
    } catch (err) {
      showAlert("Refresh failed: " + String(err), "error");
    } finally {
      btnRefresh.disabled = false;
      btnRefreshSpinner.classList.add("hidden");
    }
  }

  // ─── AI Mode Switcher ───
  async function selectAiMode(mode) {
    currentAiMode = mode;
    document.querySelectorAll(".pill[data-mode]").forEach((btn) => {
      const isCurrent = btn.dataset.mode === mode;
      btn.classList.toggle("active", isCurrent);
      btn.setAttribute("aria-checked", isCurrent ? "true" : "false");
    });

    if (invoke) {
      try {
        await invoke("set_ai_mode_cmd", {
          mode: mode,
          threshold: mode === "hybrid" ? 0.75 : 0.5,
          optIn: mode !== "local",
        });
      } catch (err) {
        console.warn("Failed to set AI mode:", err);
      }
    }
  }

  // ─── Demo Data ───
  function generateDemoData(fileNames) {
    const demos = [
      ["vacation_photo.jpg", "Images", 0.87, "heuristic: ext .jpg \u2192 Images"],
      ["sunset_panorama.png", "Images", 0.86, "heuristic: ext .png \u2192 Images"],
      ["screenshot_2024.png", "Images", 0.86, "heuristic: ext .png \u2192 Images"],
      ["report_q4.pdf", "Documents", 0.82, "heuristic: ext .pdf \u2192 Documents"],
      ["meeting_notes.docx", "Documents", 0.82, "heuristic: ext .docx \u2192 Documents"],
      ["wedding_video.mp4", "Videos", 0.86, "heuristic: ext .mp4 \u2192 Videos"],
      ["podcast_ep42.mp3", "Audio", 0.84, "heuristic: ext .mp3 \u2192 Audio"],
      ["backup_2024.zip", "Archives", 0.83, "heuristic: ext .zip \u2192 Archives"],
      ["main.rs", "Code", 0.80, "heuristic: ext .rs \u2192 Code"],
      ["README.md", "Documents", 0.82, "heuristic: ext .md \u2192 Documents"],
      ["data_export.csv", "Documents", 0.82, "heuristic: ext .csv \u2192 Documents"],
    ];

    if (fileNames && fileNames.length > 0) {
      return fileNames.map((name) => {
        const ext = name.split(".").pop()?.toLowerCase() || "";
        const match = demos.find((d) => d[0].split(".").pop()?.toLowerCase() === ext);
        const r = match || ["", "General", 0.62, "heuristic: no ext \u2192 General"];
        return { file: name, category: r[1], folder: r[1], confidence: r[2], reason: r[3] };
      });
    }
    return demos.map((d) => ({
      file: d[0], category: d[1], folder: d[1], confidence: d[2], reason: d[3],
    }));
  }

  // ─── Render Preview Table ───
  function renderPreview(data) {
    previewData = data;
    // Clear widget handoff cache if this is a fresh render not from widget
    // (widget path sets window.__broomed_last_previews explicitly after calling renderPreview)
    previewBody.innerHTML = "";
    previewCount.textContent = data.length + " files";

    const cats = new Set(data.map((d) => d.category));
    const avg = data.reduce((s, d) => s + d.confidence, 0) / (data.length || 1);
    previewSummary.textContent =
      cats.size + " categories \u2022 Avg confidence " + Math.round(avg * 100) + "% (" + currentAiMode.toUpperCase() + " mode)";

    previewTableWrap.scrollTop = 0;

    let i = 0;
    function renderBatch() {
      const frag = document.createDocumentFragment();
      const end = Math.min(i + RENDER_BATCH, data.length);
      for (; i < end; i++) {
        const d = data[i];
        const pct = Math.round(d.confidence * 100);
        const cls = d.confidence >= CONF_HIGH ? "conf-high" : d.confidence >= CONF_MED ? "conf-med" : "conf-low";
        const tr = document.createElement("tr");
        tr.innerHTML =
          '<td class="col-file" title="' + esc(d.file) + '">' + esc(d.file) + "</td>" +
          '<td class="col-dest">' + esc(d.folder) + "</td>" +
          '<td class="col-conf"><span class="conf-badge ' + cls + '">' + pct + "%</span></td>" +
          '<td class="col-reason" title="' + esc(d.reason) + '">' + esc(d.reason) + "</td>";
        frag.appendChild(tr);
      }
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
    }

    const results = [];
    for (const d of previewData) {
      try {
        const r = await invoke("classify_cmd", {
          task: "ClassifyFile",
          input: d.path || d.file,
          provider: currentAiMode === "online" ? "online" : "bundled",
        });
        results.push({
          file: d.file, path: d.path,
          category: r.category,
          folder: r.suggested_folder || r.category,
          confidence: r.confidence,
          reason: r.reason,
        });
      } catch {
        results.push({ ...d, confidence: 0.5, reason: "classification fallback" });
      }
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
    }
  });

  document.querySelectorAll(".pill[data-mode]").forEach((btn) => {
    btn.addEventListener("click", () => {
      if (btn.disabled) return;
      selectAiMode(btn.dataset.mode);
    });
  });

  dropzone.addEventListener("click", pickFolder);
  dropzone.addEventListener("keydown", (e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      pickFolder();
    }
  });

  dropzone.addEventListener("dragover", (e) => {
    e.preventDefault();
    dropzone.classList.add("dragover");
  });

  dropzone.addEventListener("dragleave", () => {
    dropzone.classList.remove("dragover");
  });

  dropzone.addEventListener("drop", (e) => {
    e.preventDefault();
    dropzone.classList.remove("dragover");
    const items = e.dataTransfer?.items;
    if (items && items.length > 0) {
      const entry = items[0].webkitGetAsEntry?.();
      if (entry) scanFolder(entry.fullPath || entry.name);
    }
  });

  folderInput.addEventListener("change", (e) => {
    const files = e.target.files;
    if (files && files.length > 0) {
      const names = Array.from(files).map((f) => f.name);
      setState("classifying");
      setTimeout(() => {
        renderPreview(generateDemoData(names));
        setState("preview");
      }, 700);
    }
  });

  btnPreview.addEventListener("click", () => {
    if (state === "idle" && folderPath) scanFolder(folderPath);
    else if (state === "preview") classifyFiles();
  });
  btnExecute.addEventListener("click", executePlan);
  btnUndo.addEventListener("click", undoLast);
  btnRescan.addEventListener("click", () => {
    setState("idle");
  });

  // ─── Initialization ───
  setState("idle");
  fetchLicenseInfo();
})();
