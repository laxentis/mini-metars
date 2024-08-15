import {
  batch,
  Component,
  createMemo,
  createSignal,
  For,
  onCleanup,
  onMount,
  Show,
} from "solid-js";
import { lookupStationCmd, updateAtisCmd, updateMetarCmd } from "./tauri.ts";
import { logIfDev } from "./logging.ts";
import { createStore } from "solid-js/store";
import { MainUiStore } from "./App.tsx";
import { clsx } from "clsx";

interface MetarProps {
  requestedId: string;
  mainUi: MainUiStore;
  resizeAfterFn: (fn: () => void) => void;
}

function getRandomInt(min: number, max: number) {
  const minCeiled = Math.ceil(min);
  const maxFloored = Math.floor(max);
  return Math.floor(Math.random() * (maxFloored - minCeiled) + minCeiled); // The maximum is exclusive and the minimum is inclusive
}

export const Metar: Component<MetarProps> = (props) => {
  const [icaoId, setIcaoId] = createSignal("");
  const [currentTimestamp, setCurrentTimestamp] = createSignal<Date>();
  const [validId, setValidId] = createSignal(false);

  // UI Display Signals
  const [displayId, setDisplayId] = createSignal("");
  const [wind, setWind] = createSignal("");
  const [rawMetar, setRawMetar] = createSignal("");
  const [altimeter, setAltimeter] = createStore<{ inHg: number; hpa: number }>({
    inHg: 0.0,
    hpa: 0.0,
  });
  const altimeterString = createMemo(() => {
    if (props.mainUi.units === "inHg") {
      return altimeter.inHg == 0 ? "" : altimeter.inHg.toFixed(2);
    } else {
      return altimeter.hpa == 0 ? "" : altimeter.hpa.toFixed(0);
    }
  });
  const [showFullMetar, setShowFullMetar] = createSignal(false);
  const [atisLetter, setAtisLetter] = createSignal("-");
  const [atisTexts, setAtisTexts] = createStore<string[]>([]);
  const [showAtisTexts, setShowAtisTexts] = createSignal(false);

  // Update handle
  const [metarTimerHandle, setMetarTimerHandle] = createSignal<number | undefined>(undefined);
  const [letterTimerHandle, setLetterTimerHandle] = createSignal<number | undefined>(undefined);

  const fetchAndUpdateStation = async () => {
    try {
      logIfDev("Looking up requested ID", props.requestedId);
      let station = await lookupStationCmd(props.requestedId);
      setIcaoId(station.icaoId);
      setDisplayId(station.faaId !== "-" ? station.faaId : station.icaoId);
      setValidId(true);
    } catch (error) {
      setDisplayId(props.requestedId);
      console.log(error);
    }
  };

  const updateMetar = async () => {
    if (!validId()) {
      return;
    }

    try {
      logIfDev("Starting update check for id", icaoId());
      let res = await updateMetarCmd(icaoId());
      logIfDev("Retrieved METAR", icaoId(), res);
      let newTimestamp = new Date(res.metar.obsTime);
      if (currentTimestamp() === undefined || newTimestamp > currentTimestamp()!) {
        logIfDev("New METAR found", icaoId());
        setCurrentTimestamp(newTimestamp);
        setAltimeter(res.altimeter);
        setWind(res.windString);
        setRawMetar(res.metar.rawOb);
      } else {
        logIfDev("Fetched METAR same as displayed", icaoId(), currentTimestamp());
      }
    } catch (error) {
      console.log(error);
    }
  };

  const updateAtis = async () => {
    if (!validId()) {
      return;
    }

    try {
      logIfDev("Starting ATIS letter fetch for id", icaoId());
      let res = await updateAtisCmd(icaoId());
      logIfDev("Retrieved ATIS Letter", res);
      setAtisLetter(res.letter);
      setAtisTexts(res.texts);
    } catch (error) {
      console.log(error);
    }
  };

  onMount(async () => {
    try {
      await fetchAndUpdateStation();
      if (validId()) {
        await updateMetar();
        setMetarTimerHandle(setInterval(updateMetar, 1000 * getRandomInt(120, 150)));

        await updateAtis();
        setLetterTimerHandle(setInterval(updateAtis, 1000 * getRandomInt(20, 30)));
      }
    } catch (error) {
      console.log(error);
    }
  });

  onCleanup(() => {
    if (metarTimerHandle() !== undefined) {
      clearInterval(metarTimerHandle());
    }

    if (letterTimerHandle() !== undefined) {
      clearInterval(letterTimerHandle());
    }
  });

  const toggleShowMetar = () => {
    props.resizeAfterFn(() => {
      batch(() => {
        if (showFullMetar()) {
          setShowFullMetar(false);
        } else {
          setShowFullMetar(true);
          setShowAtisTexts(false);
        }
      });
    });
  };

  const toggleShowAtisTexts = () => {
    props.resizeAfterFn(() => {
      batch(() => {
        if (atisTexts.length === 0) {
          setShowAtisTexts(false);
          return;
        }

        if (showAtisTexts()) {
          setShowAtisTexts(false);
        } else {
          setShowAtisTexts(true);
          setShowFullMetar(false);
        }
      });
    });
  };

  let fullTextClass = createMemo(() => {
    return clsx({
      "text-xs mb-1 text-gray-400 pr-1": true,
      "w-[calc(100vw-1.25rem)]": props.mainUi.showInput,
      "w-screen": !props.mainUi.showInput,
    });
  });

  return (
    <div class="flex flex-col mx-1 select-none cursor-pointer">
      <div class="flex font-mono text-sm">
        <div class="w-8">{displayId()}</div>
        <div class="w-10 text-center" onClick={toggleShowAtisTexts}>
          {atisLetter()}
        </div>
        <div class="w-16 text-center" onClick={toggleShowMetar}>
          {altimeterString()}
        </div>
        <div class="flex-grow" onClick={toggleShowMetar}>
          {wind()}
        </div>
      </div>
      <Show when={showFullMetar() && rawMetar() !== ""}>
        <div class={fullTextClass()}>{rawMetar()}</div>
      </Show>
      <Show when={showAtisTexts()}>
        <For each={atisTexts}>{(atisText) => <div class={fullTextClass()}>{atisText}</div>}</For>
      </Show>
    </div>
  );
};
