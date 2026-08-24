/**
 * MascotController — receives Broomed app signals, maps to mascot states.
 * No filesystem logic. Debounces rapid-fire signals.
 */
import { MascotState, createMascotStateMachine } from "./mascotState.js";

const DEBOUNCE_MS = 120;

export function createMascotController({ onStateChange }) {
  const machine = createMascotStateMachine();
  let debounceTimer = null;

  // Map Broomed app signals to the mascot states
  function mapSignals(app) {
    // app: { scanning, organizing, classifying, executing, preview, error, lowConfidence }
    const busy = Boolean(app.scanning || app.organizing || app.classifying || app.executing);
    const needsAttention = Boolean(app.preview && app.lowConfidence);
    const hasError = Boolean(app.error);

    return {
      working: busy,
      attention: needsAttention,
      error: hasError,
    };
  }

  function setAppState(app) {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      const signals = mapSignals(app);
      const state = machine.resolve(signals);
      if (onStateChange) onStateChange(state, machine.persistent);
    }, DEBOUNCE_MS);
  }

  function notifySuccess() {
    clearTimeout(debounceTimer);
    // Baseline quiescent state when operation successfully completed
    machine.resolve({ working: false, attention: false, error: false });
    const state = machine.triggerSuccess();
    if (onStateChange) onStateChange(state, machine.persistent);
  }

  machine.onTransientEnd = (persistentState) => {
    if (onStateChange) onStateChange(persistentState, persistentState);
  };

  return {
    setAppState,
    notifySuccess,
    getCurrent: machine.getCurrent,
    reset: machine.reset,
    destroy() {
      clearTimeout(debounceTimer);
      machine.reset();
    },
  };
}
