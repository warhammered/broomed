/**
 * Mascot — public entry point. Wires controller + renderer + bubble.
 */
import { createMascotController } from "./mascotController.js";
import { createMascotRenderer } from "./mascotRenderer.js";

export function initMascot(mascotEl, bubbleEl, inputEl, closeBtnEl) {
  const renderer = createMascotRenderer(mascotEl);
  let bubbleOpen = false;

  // ─── Bubble logic ───
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

  function toggleBubble() {
    if (bubbleOpen) closeBubble();
    else openBubble();
  }

  // Mascot click/focus → open bubble
  mascotEl.addEventListener("click", toggleBubble);
  mascotEl.addEventListener("keydown", (e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      toggleBubble();
    }
  });

  // Close button
  if (closeBtnEl) {
    closeBtnEl.addEventListener("click", (e) => {
      e.stopPropagation();
      closeBubble();
    });
  }

  // Escape closes bubble (works from bubble or input focus)
  function onEscape(e) {
    if (e.key === "Escape" && bubbleOpen) {
      e.stopPropagation();
      closeBubble();
    }
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

  // Click outside bubble to close
  document.addEventListener("click", (e) => {
    if (bubbleOpen && bubbleEl && !bubbleEl.contains(e.target) && !mascotEl.contains(e.target)) {
      closeBubble();
    }
  });

  // ─── Controller ───
  const controller = createMascotController({
    onStateChange: (state) => renderer.render(state),
  });

  // Initial render
  renderer.render("idle");

  return {
    setAppState: controller.setAppState,
    notifySuccess: controller.notifySuccess,
    getCurrentState: controller.getCurrent,
    openBubble,
    closeBubble,
    destroy() {
      controller.destroy();
    },
  };
}
