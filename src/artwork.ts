import { useEffect, useState } from "react";
import { loadGameArtwork } from "./api";
import { errMsg } from "./format";
import type { Game } from "./types";

const pendingImages = new Map<string, Promise<string | null>>();

export function useGameArtwork(game: Game, automatic = false) {
  const enabled =
    Boolean(game.artwork) ||
    (automatic && game.artworkCheckedName !== `steam:${game.name}`);
  const key = JSON.stringify([
    game.id,
    game.name,
    game.artwork?.url,
    automatic,
    enabled,
  ]);
  const [result, setResult] = useState({
    key: "",
    src: null as string | null,
    error: "",
  });

  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    let request = pendingImages.get(key);
    if (!request) {
      request = loadGameArtwork(game.id, automatic);
      pendingImages.set(key, request);
      const cleanup = () => pendingImages.delete(key);
      void request.then(cleanup, cleanup);
    }
    void request.then(
      (src) => {
        if (!cancelled) setResult({ key, src, error: "" });
      },
      (error) => {
        if (!cancelled) setResult({ key, src: null, error: errMsg(error) });
      },
    );
    return () => {
      cancelled = true;
    };
  }, [key, enabled, game.id, automatic]);

  return result.key === key && enabled ? result : { src: null, error: "" };
}
