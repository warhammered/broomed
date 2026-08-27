/**
 * Mascot — controller + renderer + bubble in one file.
 * State priority: error > attention > working > idle.
 * `notifySuccess` triggers a 1.2s transient "success" then returns to persistent.
 */

const STATES = { IDLE: "idle", WORKING: "working", ATTENTION: "attention", ERROR: "error", SUCCESS: "success" };
const PRIORITY = { error: 5, attention: 4, success: 3, working: 2, idle: 1 };
const DEBOUNCE_MS = 120;
const TRANSIENT_MS = 1200;

function mapSignals(app) {
  const busy = Boolean(app.scanning || app.organizing || app.classifying || app.executing);
  return { working: busy, attention: Boolean(app.preview && app.lowConfidence), error: Boolean(app.error) };
}

function resolveState(signals) {
  if (signals.error) return STATES.ERROR;
  if (signals.attention) return STATES.ATTENTION;
  if (signals.working) return STATES.WORKING;
  return STATES.IDLE;
}

export function initMascot(mascotEl, bubbleEl, inputEl, closeBtnEl) {
  if (!mascotEl) throw new Error("initMascot: mascot element required");

  let persistent = STATES.IDLE;
  let transient = null;
  let transientTimer = null;
  let debounceTimer = null;
  let bubbleOpen = false;
  const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  function render(state) {
    mascotEl.className = "mascot " + state;
  }

  function currentState() { return transient || persistent; }

  function openBubble() {
    if (!bubbleEl) return;
    bubbleEl.classList.remove("hidden");
    bubbleOpen = true;
    if (inputEl) inputEl.focus();
  }

  function closeBubble() {
    if (!bubbleEl) return;
    bubbleEl.classList.add("hidden");
    bubbleOpen = false;
    mascotEl.focus();
  }

  function toggleBubble() { (bubbleOpen ? closeBubble : openBubble)(); }

  function commitTransient() {
    transient = null;
    transientTimer = null;
    render(persistent);
  }

  function setAppState(app) {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      const signals = mapSignals(app);
      const next = resolveState(signals);
      persistent = next;
      if (transient && (PRIORITY[next] > PRIORITY[transient] || next === STATES.WORKING)) {
        clearTimeout(transientTimer);
        commitTransient();
      }
      render(currentState());
    }, DEBOUNCE_MS);
  }

  function notifySuccess() {
    clearTimeout(debounceTimer);
    persistent = resolveState({ working: false, attention: false, error: false });
    transient = STATES.SUCCESS;
    render(STATES.SUCCESS);
    clearTimeout(transientTimer);
    transientTimer = setTimeout(commitTransient, TRANSIENT_MS);
  }

  mascotEl.addEventListener("click", toggleBubble);
  mascotEl.addEventListener("keydown", (e) => {
    if (e.key === "Enter" || e.key === " ") { e.preventDefault(); toggleBubble(); }
  });

  if (closeBtnEl) closeBtnEl.addEventListener("click", (e) => { e.stopPropagation(); closeBubble(); });

  function onEscape(e) {
    if (e.key === "Escape" && bubbleOpen) { e.stopPropagation(); closeBubble(); }
  }
  bubbleEl && bubbleEl.addEventListener("keydown", onEscape);
  if (inputEl) {
    inputEl.addEventListener("keydown", onEscape);
    inputEl.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        const val = inputEl.value.trim();
        if (val) {
          window.dispatchEvent(new CustomEvent("mascot-query", { detail: { text: val } }));
          inputEl.value = "";
        }
      }
    });
  }
  window.addEventListener("keydown", onEscape);
  document.addEventListener("click", (e) => {
    if (bubbleOpen && bubbleEl && !bubbleEl.contains(e.target) && !mascotEl.contains(e.target)) closeBubble();
  });

  // Initial render
  render(STATES.IDLE);

  // Keep `reducedMotion` listener (no-op currently) for future use — costs nothing.
  window.matchMedia("(prefers-reduced-motion: reduce)").addEventListener("change", () => {});

  return {
    setAppState,
    notifySuccess,
    getReducedMotion: () => reducedMotion,
    openBubble,
    closeBubble,
    destroy() {
      clearTimeout(debounceTimer);
      clearTimeout(transientTimer);
    },
  };
}
