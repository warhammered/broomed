/**
 * MascotRenderer — Controls mascot state classes on the frontend using broomed.svg.
 * Supports 5 states: idle, working, success, attention, error.
 * Fully supports prefers-reduced-motion.
 */

export function createMascotRenderer(mascotEl) {
  if (!mascotEl) throw new Error("MascotRenderer: mascot element required");

  let currentState = "idle";
  let reducedMotion = false;

  function checkReducedMotion() {
    reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  }

  function render(state) {
    if (state === currentState) return;
    currentState = state;

    // Update CSS class on wrapper
    mascotEl.className = "mascot " + state;
  }

  function getCurrentState() {
    return currentState;
  }

  function getReducedMotion() {
    return reducedMotion;
  }

  checkReducedMotion();
  const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
  mq.addEventListener("change", () => {
    checkReducedMotion();
  });

  return {
    render,
    getCurrentState,
    getReducedMotion,
    destroy() {},
  };
}
