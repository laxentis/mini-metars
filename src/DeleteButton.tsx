import { Component } from "solid-js";

export const DeleteButton: Component<{ deleteFn: () => void }> = (props) => {
  return (
    <div class="flex w-4 h-5 items-center cursor-pointer" onClick={props.deleteFn}>
      <svg
        xmlns="http://www.w3.org/2000/svg"
        fill="none"
        viewBox="0 0 24 24"
        stroke-width="1.5"
        class="size-4 stroke-red-700 hover:stroke-red-500 transition-colors"
      >
        <path stroke-linecap="round" stroke-linejoin="round" d="M5 12h14" />
      </svg>
    </div>
  );
};
