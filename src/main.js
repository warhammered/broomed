(() => {
  "use strict";

  // ─── Constants ───
  const RENDER_BATCH = 200;
  const CONF_HIGH = 0.90;
  const CONF_MED = 0.70;

  // ─── State ───
  let state = "idle";
  let provider = "bundled";
  let previewData = [];
  let folderPath = null;

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

  // ─── Tauri bridge ───
  const isTauri = typeof window.__TAURI__ !== "undefined";
  const invoke = isTauri ? window.__TAURI__.core.invoke : null;

  // ─── Helpers ───
  const delay = (ms) => new Promise((r) => setTimeout(r, ms));
  const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
  const fileName = (p) => p.split(/[/\\]/).pop() || p;

  // ─── State machine ───
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
      classifying: "Classifying\u2026",
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

    // Preview spinner
    const previewSpinners = btnPreview.querySelectorAll(".btn-spinner");
    previewSpinners.forEach((el) => el.classList.toggle("hidden", s !== "classifying"));
    const execSpinners = btnExecute.querySelectorAll(".btn-spinner");
    execSpinners.forEach((el) => el.classList.toggle("hidden", s !== "executing"));
  }

  // ─── Demo data ───
  function generateDemoData(fileNames) {
    const demos = [
      ["vacation_photo.jpg", "Images", 0.87, "heuristic: ext .jpg \u2192 Images"],
      ["sunset_panorama.png", "Images", 0.86, "heuristic: ext .png \u2192 Images"],
      ["screenshot_2024.png", "Images", 0.86, "heuristic: ext .png \u2192 Images"],
      ["logo.svg", "Images", 0.85, "heuristic: ext .svg \u2192 Images"],
      ["portrait.heic", "Images", 0.85, "heuristic: ext .heic \u2192 Images"],
      ["report_q4.pdf", "Documents", 0.82, "heuristic: ext .pdf \u2192 Documents"],
      ["meeting_notes.docx", "Documents", 0.82, "heuristic: ext .docx \u2192 Documents"],
      ["budget_2024.xlsx", "Documents", 0.82, "heuristic: ext .xlsx \u2192 Documents"],
      ["thesis_draft.txt", "Documents", 0.82, "heuristic: ext .txt \u2192 Documents"],
      ["presentation.pptx", "Documents", 0.82, "heuristic: ext .pptx \u2192 Documents"],
      ["invoice_march.pdf", "Documents", 0.82, "heuristic: ext .pdf \u2192 Documents"],
      ["wedding_video.mp4", "Videos", 0.86, "heuristic: ext .mp4 \u2192 Videos"],
      ["tutorial_react.mov", "Videos", 0.86, "heuristic: ext .mov \u2192 Videos"],
      ["screen_recording.webm", "Videos", 0.85, "heuristic: ext .webm \u2192 Videos"],
      ["podcast_ep42.mp3", "Audio", 0.84, "heuristic: ext .mp3 \u2192 Audio"],
      ["voice_memo.m4a", "Audio", 0.84, "heuristic: ext .m4a \u2192 Audio"],
      ["background_music.wav", "Audio", 0.84, "heuristic: ext .wav \u2192 Audio"],
      ["song.flac", "Audio", 0.84, "heuristic: ext .flac \u2192 Audio"],
      ["project_backup.zip", "Archives", 0.83, "heuristic: ext .zip \u2192 Archives"],
      ["photos_2023.tar.gz", "Archives", 0.72, "heuristic: ext .gz \u2192 Archives"],
      ["disk_image.iso", "Archives", 0.83, "heuristic: ext .iso \u2192 Archives"],
      ["app.tsx", "Code", 0.80, "heuristic: ext .tsx \u2192 Code"],
      ["styles.css", "Code", 0.80, "heuristic: ext .css \u2192 Code"],
      ["server.py", "Code", 0.80, "heuristic: ext .py \u2192 Code"],
      ["config.toml", "Code", 0.80, "heuristic: ext .toml \u2192 Code"],
      ["README.md", "Documents", 0.82, "heuristic: ext .md \u2192 Documents"],
      ["data_export.csv", "Documents", 0.82, "heuristic: ext .csv \u2192 Documents"],
      ["unknown_file", "General", 0.62, "heuristic: no ext \u2192 General"],
    ];
    // If real file names provided, use them; otherwise use demo set
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

  // ─── Render preview table ───
  function renderPreview(data) {
    previewData = data;
    previewBody.innerHTML = "";
    previewCount.textContent = data.length + " files";

    const cats = new Set(data.map((d) => d.category));
    const avg = data.reduce((s, d) => s + d.confidence, 0) / data.length;
    previewSummary.textContent =
      cats.size + " categories \u2022 Avg confidence " + Math.round(avg * 100) + "%";

    // Scroll to top
    previewTableWrap.scrollTop = 0;

    let i = 0;
    function renderBatch() {
      const frag = document.createDocumentFragment();
      const end = Math.min(i + RENDER_BATCH, data.length);
      for (; i < end; i++) {
        const d = data[i];
        const pct = Math.round(d.confidence * 100);
        const cls = d.confidence >= CONF_HIGH ? "high" : d.confidence >= CONF_MED ? "med" : "low";
        const tr = document.createElement("tr");
        tr.style.animationDelay = Math.min(i * 15, 300) + "ms";
        tr.innerHTML =
          '<td class="col-file" title="' + esc(d.file) + '">' + esc(d.file) + "</td>" +
          '<td class="col-dest">' + esc(d.folder) + "</td>" +
          '<td class="col-conf"><div class="conf-cell">' +
            '<div class="conf-bar" aria-label="' + pct + '% confidence">' +
              '<div class="conf-fill ' + cls + '" style="width:' + pct + '%"></div>' +
            "</div>" +
            '<span class="conf-val">' + pct + "%</span>" +
          "</div></td>" +
          '<td class="col-reason" title="' + esc(d.reason) + '">' + esc(d.reason) + "</td>";
        frag.appendChild(tr);
      }
      previewBody.appendChild(frag);
      if (i < data.length) requestAnimationFrame(renderBatch);
    }
    renderBatch();
  }

  // ─── Scan + classify ───
  async function scanFolder(path) {
    folderPath = path;
    setState("scanning");

    if (!invoke) {
      // Demo: simulate scan delay
      await delay(1200);
      setState("classifying");
      await delay(800);
      renderPreview(generateDemoData());
      setState("preview");
      return;
    }

    try {
      const files = await invoke("scan_directory_cmd", { base: path, maxFiles: 10000 });
      if (!files || files.length === 0) {
        setState("idle");
        statusText.textContent = "No files found";
        return;
      }

      setState("classifying");
      const results = [];
      for (const f of files) {
        try {
          const r = await invoke("classify_cmd", { task: "ClassifyFile", input: f, provider });
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
            confidence: 0.5, reason: "classification failed",
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

  // ─── Classify (re-trigger from preview) ───
  async function classifyFiles() {
    setState("classifying");

    if (!invoke) {
      await delay(800);
      renderPreview(generateDemoData());
      setState("preview");
      return;
    }

    // Re-classify using stored paths
    const results = [];
    for (const d of previewData) {
      try {
        const r = await invoke("classify_cmd", { task: "ClassifyFile", input: d.path || d.file, provider });
        results.push({
          file: d.file, path: d.path,
          category: r.category,
          folder: r.suggested_folder || r.category,
          confidence: r.confidence,
          reason: r.reason,
        });
      } catch {
        results.push({ ...d, confidence: 0.5, reason: "classification failed" });
      }
    }
    renderPreview(results);
    setState("preview");
  }

  // ─── Execute plan ───
  async function executePlan() {
    setState("executing");

    if (!invoke) {
      await delay(1500);
      setState("executed");
      return;
    }

    try {
      const files = previewData.map((d) => d.path || d.file).filter(Boolean);
      const base = folderPath || ".";
      // plan then execute — ponytail minimal, reuses core pipeline (provider wired for cloud)
      const previews = await invoke("plan_organize", { files, base, task: "ClassifyFile", threshold: 0.5, provider });
      if (!previews || previews.length === 0) {
        setState("error");
        statusText.textContent = "Nothing to execute";
        return;
      }
      await invoke("execute_plan_cmd", { previews, dbPath: null });
      setState("executed");
    } catch (err) {
      setState("error");
      statusText.textContent = "Execute failed: " + String(err);
    }
  }

  // ─── Undo last ───
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

  // ─── File picker ───
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

  // ─── Provider selector ───
  document.querySelectorAll(".pill[data-provider]").forEach((btn) => {
    btn.addEventListener("click", () => {
      if (btn.disabled) return;
      document.querySelectorAll(".pill[data-provider]").forEach((b) => {
        b.classList.remove("active");
        b.setAttribute("aria-checked", "false");
      });
      btn.classList.add("active");
      btn.setAttribute("aria-checked", "true");
      provider = btn.dataset.provider;
    });
  });

  // ─── Drop zone events ───
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
      }, 800);
    }
  });

  // ─── Button events ───
  btnPreview.addEventListener("click", () => {
    if (state === "idle" && folderPath) scanFolder(folderPath);
    else if (state === "preview") classifyFiles();
  });
  btnExecute.addEventListener("click", executePlan);
  btnUndo.addEventListener("click", undoLast);
  btnRescan.addEventListener("click", () => {
    setState("idle");
  });

  // ─── Init ───
  setState("idle");
})();
