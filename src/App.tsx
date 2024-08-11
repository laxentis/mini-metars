import "./styles.css";
import { Metar } from "./Metar.tsx";
import { batch, createSignal, For, Show } from "solid-js";
import { createStore } from "solid-js/store";
// @ts-ignore
import { autofocus } from "@solid-primitives/autofocus";
import { getCurrentWindow, PhysicalSize } from "@tauri-apps/api/window";
import { logIfDev } from "./logging.ts";
import { clsx } from "clsx";
import { createShortcut, KbdKey } from "@solid-primitives/keyboard";
import { loadProfileCmd, Profile, saveProfileCmd } from "./tauri.ts";
import { type } from "@tauri-apps/plugin-os";
import { CustomTitlebar } from "./CustomTitlebar.tsx";
import { DeleteButton } from "./DeleteButton.tsx";

function removeIndex<T>(array: readonly T[], index: number): T[] {
  return [...array.slice(0, index), ...array.slice(index + 1)];
}

export interface MainUiStore {
  showScroll: boolean;
  showInput: boolean;
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
  });

  let CtrlOrCmd: KbdKey = type() === "macos" || type() === "ios" ? "Meta" : "Control";
  let PlusOrEquals = type() === "macos" || type() === "ios" ? "=" : "+";

  // Create shortcuts for profile open and save
  createShortcut(
    [CtrlOrCmd, "O"],
    async () => {
      try {
        let p = await loadProfileCmd();
        await loadStationsFromProfile(p);
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
        let p: Profile = { name: "", stations: ids };
        await saveProfileCmd(p);
      } catch (error) {
        console.log(error);
      }
    },
    { preventDefault: true, requireReset: true }
  );

  // Create shortcuts to toggle input box
  const toggleInput = async () => {
    setMainUi("showScroll", false);
    setMainUi("showInput", (prev) => !prev);
    await resetWindowHeight();
    setMainUi("showScroll", true);
  };
  createShortcut([CtrlOrCmd, "+"], toggleInput, {
    preventDefault: true,
    requireReset: false,
  });
  createShortcut([CtrlOrCmd, "Shift", PlusOrEquals], toggleInput, {
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
      await window.setSize(
        new PhysicalSize(currentSize.width, (containerRef.offsetHeight + 24) * scaleFactor)
      );
    }
  }

  async function loadStationsFromProfile(p: Profile) {
    setMainUi("showScroll", false);
    setIds(p.stations);
    await resetWindowHeight();
    setMainUi("showScroll", true);
  }

  async function addStation(e: SubmitEvent) {
    e.preventDefault();
    setMainUi("showScroll", false);
    batch(() => {
      if (inputId().length >= 3 && inputId().length <= 4) {
        setIds(ids.length, inputId());
        setInputId("");
      }
    });
    await resetWindowHeight();
    setMainUi("showScroll", true);
  }

  async function removeStation(index: number) {
    setIds((ids) => removeIndex(ids, index));
    await resetWindowHeight();
  }

  return (
    <div>
      <Show when={useCustomTitlebar}>
        <CustomTitlebar />
      </Show>
      <div
        class={clsx({
          "h-screen": true,
          "pt-[24px]": useCustomTitlebar,
          "overflow-auto": mainUi.showScroll,
          "overflow-hidden": !mainUi.showScroll,
        })}
      >
        <div class="flex flex-col bg-black text-white" ref={containerRef}>
          <div class="flex flex-col grow">
            <For each={ids}>
              {(id, i) => (
                <div class="flex">
                  <Show when={mainUi.showInput}>
                    <DeleteButton deleteFn={async () => await removeStation(i())} />
                  </Show>
                  <Metar
                    requestedId={id}
                    resizeFn={resetWindowHeight}
                    mainUi={mainUi}
                    mainUiSetter={setMainUi}
                  />
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
