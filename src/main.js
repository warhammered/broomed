(() => {
  "use strict";

  const $ = (s) => document.querySelector(s);
  const mascot = $("#mascot");
  const statusText = $("#status-text");
  const dropzone = $("#dropzone");
  const folderInput = $("#folder-input");
  const results = $("#results");
  const resultsList = $("#results-list");

  // ─── Tauri bridge ───
  const isTauri = typeof window.__TAURI__ !== "undefined";
  const invoke = isTauri ? window.__TAURI__.core.invoke : null;

  // ─── Mascot state ───
  function setMascotState(state) {
    mascot.className = `mascot ${state.toLowerCase()}`;
  }

  function setStatus(text) {
    statusText.textContent = text;
  }

  // ─── Scan flow ───
  async function scanFolder(path) {
    setMascotState("Scanning");
    setStatus("Scanning files\u2026");

    if (!invoke) {
      // Fallback: simulate scan when not in Tauri
      await new Promise((r) => setTimeout(r, 1200));
      showDemoResults();
      return;
    }

    try {
      const files = await invoke("scan_directory_cmd", {
        base: path,
        maxFiles: 500,
      });

      if (files && files.length > 0) {
        setMascotState("Thinking");
        setStatus("Analyzing\u2026");

        // Parse intent for each unique extension group
        const extMap = {};
        for (const f of files) {
          const ext = f.split(".").pop()?.toLowerCase() || "other";
          extMap[ext] = (extMap[ext] || 0) + 1;
        }

        await new Promise((r) => setTimeout(r, 800));
        showResults(extMap, files.length);
      } else {
        setMascotState("Idle");
        setStatus("No files found");
      }
    } catch (err) {
      setMascotState("Error");
      setStatus("Scan failed: " + String(err));
    }
  }

  function showResults(extMap, total) {
    setMascotState("Found");
    setStatus(`${total} files scanned`);

    resultsList.innerHTML = "";
    for (const [ext, count] of Object.entries(extMap).sort(
      (a, b) => b[1] - a[1]
    )) {
      const li = document.createElement("li");
      li.innerHTML = `
        <span class="result-icon">&#9679;</span>
        <span class="result-text">.${ext} &times; ${count}</span>
      `;
      resultsList.appendChild(li);
    }
    results.classList.remove("hidden");
  }

  function showDemoResults() {
    setMascotState("Thinking");
    setStatus("Analyzing\u2026");

    setTimeout(() => {
      const extMap = {
        jpg: 42,
        png: 18,
        pdf: 7,
        docx: 12,
        mp4: 3,
        txt: 23,
      };
      showResults(extMap, 105);
    }, 900);
  }

  // ─── File selection ───
  async function pickFolder() {
    if (!invoke) {
      // Demo mode: prompt for a path string
      const path = prompt("Enter folder path to scan:", "C:\\Users\\Demo\\Downloads");
      if (path) scanFolder(path);
      return;
    }

    try {
      const result = await invoke("browse_directory_cmd");
      if (result) scanFolder(result);
    } catch {
      setMascotState("Error");
      setStatus("Could not open folder picker");
    }
  }

  // ─── Events ───
  dropzone.addEventListener("click", pickFolder);
  dropzone.addEventListener("keydown", (e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      pickFolder();
    }
  });

  // Drag & drop
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
      // Try to get the path from dropped items (works in Tauri)
      const entry = items[0].webkitGetAsEntry?.();
      if (entry) {
        // Fallback: use the entry name as demo
        scanFolder(entry.fullPath || entry.name);
      }
    }
  });

  // Hidden input fallback
  folderInput.addEventListener("change", (e) => {
    const files = e.target.files;
    if (files && files.length > 0) {
      // Group by extension
      const extMap = {};
      for (const f of files) {
        const ext = f.name.split(".").pop()?.toLowerCase() || "other";
        extMap[ext] = (extMap[ext] || 0) + 1;
      }
      setMascotState("Thinking");
      setStatus("Analyzing\u2026");
      setTimeout(() => showResults(extMap, files.length), 900);
    }
  });

  // ─── Init ───
  setMascotState("Idle");
  setStatus("Ready");
})();
