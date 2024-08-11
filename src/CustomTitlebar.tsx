import { Component } from "solid-js";
import { getCurrentWindow } from "@tauri-apps/api/window";

export const CustomTitlebar: Component = () => {
  let window = getCurrentWindow();

  return (
    <div>
      <div
        data-tauri-drag-region=""
        class="select-none bg-gray-800 h-[24px] flex justify-end fixed top-0 left-0 right-0 items-center"
      >
        <div id="titlebar-minimize" class="mr-3" onClick={window.minimize}>
          <svg
            xmlns="http://www.w3.org/2000/svg"
            fill="none"
            viewBox="0 0 24 24"
            stroke-width="1.5"
            stroke="currentColor"
            class="size-5 stroke-gray-400 hover:stroke-white transition-colors"
          >
            <path stroke-linecap="round" stroke-linejoin="round" d="M5 12h14" />
          </svg>
        </div>
        <div id="titlebar-close" class="mr-1" onClick={window.close}>
          <svg
            xmlns="http://www.w3.org/2000/svg"
            fill="none"
            viewBox="0 0 24 24"
            stroke-width="1.5"
            stroke="currentColor"
            class="size-5 stroke-gray-400 hover:stroke-white transition-colors"
          >
            <path stroke-linecap="round" stroke-linejoin="round" d="M6 18 18 6M6 6l12 12" />
          </svg>
        </div>
      </div>
    </div>
  );
};
