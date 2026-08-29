/**
 * Broomed Target Window Overlay Script
 */
(() => {
  "use strict";

  const folderTitle = document.getElementById("folder-title");
  const dropRipple  = document.getElementById("drop-ripple");

  function getEventApi() {
    if (typeof window.__TAURI__ !== "undefined" && window.__TAURI__.event) return window.__TAURI__.event;
    if (typeof window.__TAURI_INTERNALS__ !== "undefined" && window.__TAURI_INTERNALS__.event) return window.__TAURI_INTERNALS__.event;
    return null;
  }

  function setupListeners() {
    const evt = getEventApi();
    if (!evt || !evt.listen) {
      setTimeout(setupListeners, 100);
      return;
    }

    evt.listen("broomed:update-target-overlay", (e) => {
      const payload = e.payload;
      if (payload && payload.folderName && folderTitle) {
        folderTitle.textContent = payload.folderName;
      }
    });

    evt.listen("broomed:confirm-target-drop", () => {
      if (dropRipple) {
        dropRipple.classList.remove("hidden", "active");
        void dropRipple.offsetWidth;
        dropRipple.classList.add("active");
        setTimeout(() => {
          dropRipple.classList.add("hidden");
          dropRipple.classList.remove("active");
        }, 500);
      }
    });
  }

  setupListeners();
})();
