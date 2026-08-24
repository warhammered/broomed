import { describe, test, expect } from "bun:test";
import { MascotState, createMascotStateMachine } from "./mascotState.js";
import { createMascotController } from "./mascotController.js";

describe("MascotStateMachine", () => {
  test("initial state is idle", () => {
    const sm = createMascotStateMachine();
    expect(sm.getCurrent()).toBe(MascotState.IDLE);
    expect(sm.persistent).toBe(MascotState.IDLE);
  });

  test("priority: error > attention > working > idle", () => {
    const sm = createMascotStateMachine();

    expect(sm.resolve({ working: true })).toBe(MascotState.WORKING);
    expect(sm.resolve({ working: true, attention: true })).toBe(MascotState.ATTENTION);
    expect(sm.resolve({ working: true, attention: true, error: true })).toBe(MascotState.ERROR);
    expect(sm.resolve({ error: false, attention: true, working: false })).toBe(MascotState.ATTENTION);
    expect(sm.resolve({ error: false, attention: false, working: false })).toBe(MascotState.IDLE);
  });

  test("transient success returns to persistent state", async () => {
    const sm = createMascotStateMachine();
    sm.resolve({ working: true });
    expect(sm.getCurrent()).toBe(MascotState.WORKING);

    sm.resolve({ working: false }); // persistent becomes idle
    const current = sm.triggerSuccess();
    expect(current).toBe(MascotState.SUCCESS);
    expect(sm.getCurrent()).toBe(MascotState.SUCCESS);
    expect(sm.persistent).toBe(MascotState.IDLE);

    let endedState = null;
    sm.onTransientEnd = (state) => {
      endedState = state;
    };

    // Wait for transient duration (~1200ms)
    await new Promise((r) => setTimeout(r, 1300));
    expect(sm.getCurrent()).toBe(MascotState.IDLE);
    expect(endedState).toBe(MascotState.IDLE);
  });

  test("transient success is interrupted by urgent error", () => {
    const sm = createMascotStateMachine();
    sm.triggerSuccess();
    expect(sm.getCurrent()).toBe(MascotState.SUCCESS);

    sm.resolve({ error: true });
    expect(sm.getCurrent()).toBe(MascotState.ERROR);
    expect(sm.transient).toBeNull();
  });
});

describe("MascotController", () => {
  test("maps signals correctly and debounces", async () => {
    let lastState = null;
    const controller = createMascotController({
      onStateChange: (s) => {
        lastState = s;
      },
    });

    controller.setAppState({ scanning: true });
    await new Promise((r) => setTimeout(r, 150));
    expect(lastState).toBe(MascotState.WORKING);

    controller.setAppState({ scanning: false, preview: true, lowConfidence: true });
    await new Promise((r) => setTimeout(r, 150));
    expect(lastState).toBe(MascotState.ATTENTION);

    controller.setAppState({ scanning: false, error: true });
    await new Promise((r) => setTimeout(r, 150));
    expect(lastState).toBe(MascotState.ERROR);

    controller.destroy();
  });

  test("notifySuccess transitions immediately to success and ends at idle", async () => {
    const changes = [];
    const controller = createMascotController({
      onStateChange: (s) => {
        changes.push(s);
      },
    });

    controller.notifySuccess();
    expect(changes).toContain(MascotState.SUCCESS);

    await new Promise((r) => setTimeout(r, 1300));
    expect(changes[changes.length - 1]).toBe(MascotState.IDLE);

    controller.destroy();
  });
});
