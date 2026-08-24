/**
 * MascotState — pure state machine with priority resolution + transient handling.
 * No DOM, no side effects, no filesystem logic.
 *
 * Priority hierarchy:
 * ERROR (5) > ATTENTION (4) > SUCCESS (3, transient) > WORKING (2) > IDLE (1)
 */

export const MascotState = {
  IDLE: "idle",
  WORKING: "working",
  SUCCESS: "success",
  ATTENTION: "attention",
  ERROR: "error",
};

const PRIORITY = {
  [MascotState.ERROR]: 5,
  [MascotState.ATTENTION]: 4,
  [MascotState.SUCCESS]: 3,
  [MascotState.WORKING]: 2,
  [MascotState.IDLE]: 1,
};

const TRANSIENT_DURATION = 1200; // ms (covers 8 frames @ 100ms + settle)

export function createMascotStateMachine() {
  let persistent = MascotState.IDLE;
  let transient = null; // non-null while transient state is active
  let timer = null;
  let onTransientEnd = null;

  function resolve(signals) {
    // signals: { error, attention, working }
    let next;
    if (signals.error) next = MascotState.ERROR;
    else if (signals.attention) next = MascotState.ATTENTION;
    else if (signals.working) next = MascotState.WORKING;
    else next = MascotState.IDLE;

    // Update the underlying persistent state
    persistent = next;

    // If an urgent state (error or new working) arrives while transient is active, preempt transient
    if (transient) {
      if (PRIORITY[next] > PRIORITY[transient] || next === MascotState.WORKING) {
        clearTimeout(timer);
        timer = null;
        transient = null;
      }
    }

    return getCurrent();
  }

  function triggerSuccess() {
    // Transient: show success animation, then transition to persistent state
    transient = MascotState.SUCCESS;
    if (timer) clearTimeout(timer);

    timer = setTimeout(() => {
      transient = null;
      timer = null;
      if (onTransientEnd) onTransientEnd(persistent);
    }, TRANSIENT_DURATION);

    return getCurrent();
  }

  function getCurrent() {
    return transient || persistent;
  }

  function reset() {
    if (timer) clearTimeout(timer);
    timer = null;
    transient = null;
    persistent = MascotState.IDLE;
  }

  return {
    resolve,
    triggerSuccess,
    getCurrent,
    reset,
    get persistent() {
      return persistent;
    },
    get transient() {
      return transient;
    },
    set onTransientEnd(fn) {
      onTransientEnd = fn;
    },
  };
}
