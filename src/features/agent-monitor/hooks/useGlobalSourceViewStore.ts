import { useEffect, useReducer } from "react";

import { subscribeGlobalSourceSnapshot } from "@services/events";
import { getGlobalSourceSnapshot } from "@services/tauri";
import {
  EMPTY_GLOBAL_SOURCE_SNAPSHOT,
  type GlobalSourceSnapshot,
} from "../global-source/types";

function acceptNewerSnapshot(
  current: GlobalSourceSnapshot,
  incoming: GlobalSourceSnapshot,
) {
  return incoming.revision > current.revision ? incoming : current;
}

export function useGlobalSourceViewStore() {
  const [snapshot, ingestSnapshot] = useReducer(
    acceptNewerSnapshot,
    EMPTY_GLOBAL_SOURCE_SNAPSHOT,
  );

  useEffect(() => {
    let mounted = true;
    const unsubscribe = subscribeGlobalSourceSnapshot((incoming) => {
      if (mounted) ingestSnapshot(incoming);
    });
    void getGlobalSourceSnapshot()
      .then((initial) => {
        if (mounted) ingestSnapshot(initial);
      })
      .catch((error) => {
        console.warn("Unable to read Global Source snapshot", error);
      });
    return () => {
      mounted = false;
      unsubscribe();
    };
  }, []);

  return { snapshot };
}

