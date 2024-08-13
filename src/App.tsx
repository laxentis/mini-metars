import "./styles.css";
import { Metar } from "./Metar.tsx";
import { batch, createMemo, createSignal, For, onMount, Show } from "solid-js";
import { createStore } from "solid-js/store";
// @ts-ignore
import { autofocus } from "@solid-primitives/autofocus";
import { getCurrentWindow, PhysicalSize } from "@tauri-apps/api/window";
import { logIfDev } from "./logging.ts";
import { clsx } from "clsx";
import { createShortcut, KbdKey } from "@solid-primitives/keyboard";
import {
  loadProfileCmd,
  loadSettingsInitialCmd,
  Profile,
  saveProfileAsCmd,
  saveProfileCmd,
  saveSettingsCmd,
  Settings,
} from "./tauri.ts";
import { type } from "@tauri-apps/plugin-os";
import { CustomTitlebar } from "./CustomTitlebar.tsx";
import { DeleteButton } from "./DeleteButton.tsx";

function removeIndex<T>(array: readonly T[], index: number): T[] {
  return [...array.slice(0, index), ...array.slice(index + 1)];
}

export interface MainUiStore {
  showScroll: boolean;
  showInput: boolean;
  showTitlebar: boolean;
}

function App() {
  // Window basics
  let containerRef: HTMLDivElement | undefined;
  let window = getCurrentWindow();
  let useCustomTitlebar = type() === "windows";

  // Prevent right-click in prod
  if (import.meta.env.PROD) {
    document.addEventListener("contextmenu", (event) => event.preventDefault());
  }

  // Main signals for IDs and input
  const [inputId, setInputId] = createSignal("");
  const [ids, setIds] = createStore<string[]>([]);
  const [mainUi, setMainUi] = createStore<MainUiStore>({
    showScroll: true,
    showInput: true,
    showTitlebar: true,
  });

  // Settings store
  const [settings, setSettings] = createStore<Settings>({
    loadMostRecentProfileOnOpen: true,
    alwaysOnTop: true,
    autoResize: true,
  });

  let CtrlOrCmd: KbdKey = type() === "macos" || type() === "ios" ? "Meta" : "Control";

  let currentProfileState = createMemo<Profile>(() => {
    return {
      name: "",
      stations: ids,
      showTitlebar: mainUi.showTitlebar,
      showInput: mainUi.showInput,
    };
  });

  // Create shortcuts for profile open and save
  createShortcut(
    [CtrlOrCmd, "O"],
    async () => {
      try {
        let p = await loadProfileCmd();
        await loadProfile(p);
        await saveSettingsCmd(settings);
      } catch (error) {
        console.log(error);
      }
    },
    { preventDefault: true, requireReset: true }
  );
  createShortcut(
    [CtrlOrCmd, "S"],
    async () => {
      try {
        await saveProfileCmd(currentProfileState());
        await saveSettingsCmd(settings);
      } catch (error) {
        console.log(error);
      }
    },
    { preventDefault: true, requireReset: true }
  );
  createShortcut(
    [CtrlOrCmd, "Shift", "S"],
    async () => {
      try {
        await saveProfileAsCmd(currentProfileState());
        await saveSettingsCmd(settings);
      } catch (error) {
        console.log(error);
      }
    },
    { preventDefault: true, requireReset: true }
  );

  // Create shortcuts to toggle input box
  createShortcut(
    [CtrlOrCmd, "D"],
    async () =>
      await applyFnAndResize(() => {
        if (ids.length > 0) {
          setMainUi("showInput", (prev) => !prev);
        }
      }),
    {
      preventDefault: true,
      requireReset: false,
    }
  );

  // Create shortcut to hide custom titlebar, Windows only
  createShortcut(
    [CtrlOrCmd, "B"],
    async () =>
      await applyFnAndResize(() => {
        if (useCustomTitlebar) {
          setMainUi("showTitlebar", (prev) => !prev);
        }
      }),
    {
      preventDefault: true,
      requireReset: false,
    }
  );

  // Create shortcut to minize Window
  createShortcut([CtrlOrCmd, "M"], async () => await window.minimize(), {
    preventDefault: true,
    requireReset: false,
  });

  async function resetWindowHeight() {
    if (containerRef !== undefined) {
      let currentSize = await window.innerSize();
      logIfDev("Current window size", currentSize);
      logIfDev("containerRef height", containerRef.offsetHeight);
      let scaleFactor = await window.scaleFactor();
      logIfDev("Scale factor", scaleFactor);
      let offset = mainUi.showTitlebar ? (type() === "macos" ? 30 : 24) : 0;
      await window.setSize(
        new PhysicalSize(currentSize.width, (containerRef.offsetHeight + offset) * scaleFactor)
      );
    }
  }

  async function applyFnAndResize(fn: () => void) {
    setMainUi("showScroll", false);
    fn();
    await resetWindowHeight();
    setMainUi("showScroll", true);
  }

  async function loadProfile(p: Profile) {
    if (p.window === null) {
      await applyFnAndResize(() => {
        batch(() => {
          setIds(p.stations);
          setMainUi("showInput", p.showInput);
          setMainUi("showTitlebar", p.showTitlebar);
        });
      });
    } else {
      batch(() => {
        setIds(p.stations);
        setMainUi("showInput", p.showInput);
        setMainUi("showTitlebar", p.showTitlebar);
      });
    }
  }

  async function addStation(e: SubmitEvent) {
    e.preventDefault();
    await applyFnAndResize(() =>
      batch(() => {
        if (inputId().length >= 3 && inputId().length <= 4) {
          setIds(ids.length, inputId());
          setInputId("");
        }
      })
    );
  }

  async function removeStation(index: number) {
    await applyFnAndResize(() => setIds((ids) => removeIndex(ids, index)));
  }

  onMount(async () => {
    let res = await loadSettingsInitialCmd();
    setSettings(res.settings);
    if (res.profile && settings.loadMostRecentProfileOnOpen) {
      await loadProfile(res.profile!);
    }
  });

  return (
    <div>
      <Show when={useCustomTitlebar && mainUi.showTitlebar}>
        <CustomTitlebar />
      </Show>
      <div
        class={clsx({
          "h-screen overflow-x-hidden": true,
          "pt-[24px]": useCustomTitlebar && mainUi.showTitlebar,
          "overflow-y-auto": mainUi.showScroll,
          "overflow-y-hidden": !mainUi.showScroll,
        })}
      >
        <div class="flex flex-col bg-black text-white" ref={containerRef}>
          <div class="flex flex-col grow">
            <For each={ids}>
              {(id, i) => (
                <div class="flex">
                  <Show when={mainUi.showInput}>
                    <DeleteButton onClick={async () => await removeStation(i())} />
                  </Show>
                  <Metar requestedId={id} resizeAfterFn={applyFnAndResize} mainUi={mainUi} />
                </div>
              )}
            </For>
            <Show when={mainUi.showInput}>
              <form onSubmit={async (e) => addStation(e)}>
                <input
                  id="stationId"
                  name="stationId"
                  type="text"
                  class="w-16 text-white font-mono bg-gray-900 mx-1 my-1 border-gray-700 border focus:outline-none focus:border-gray-500 px-1 rounded"
                  value={inputId()}
                  onInput={(e) => setInputId(e.currentTarget.value)}
                  use:autofocus
                  autofocus
                  formNoValidate
                  autocomplete="off"
                />
              </form>
            </Show>
          </div>
        </div>
      </div>
    </div>
  );
}

export default App;
