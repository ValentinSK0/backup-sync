import { useEffect, useRef, useState } from "react";
import {
  clearGameArtwork,
  listArtworkImages,
  searchArtworkGames,
  selectGameArtwork,
} from "../api";
import { artworkSourceLabel, errMsg } from "../format";
import { useConfigAction } from "../hooks";
import type { ArtworkGame, ArtworkImage, Game } from "../types";
import { GamepadIcon, SpinnerIcon } from "./icons";
import { Modal } from "./Modal";
import { Button, inputCls, labelCls } from "./ui";

function normalize(name: string) {
  return name
    .replace(/ \([^)]*(?:video|computer) game[^)]*\)$/i, "")
    .toLocaleLowerCase()
    .replace(/[^\p{L}\p{N}]/gu, "");
}

function gameKey(game: ArtworkGame) {
  return `${game.source}:${game.id}`;
}

export default function GameArtworkPicker({
  game,
  onClose,
}: {
  game: Game;
  onClose: () => void;
}) {
  const { busy, run } = useConfigAction();
  const [query, setQuery] = useState(game.name);
  const [games, setGames] = useState<ArtworkGame[] | null>(null);
  const [selectedGame, setSelectedGame] = useState<ArtworkGame | null>(null);
  const [kind, setKind] = useState<"grid" | "hero">(
    game.artwork?.kind ?? "grid",
  );
  const [page, setPage] = useState(0);
  const [images, setImages] = useState<ArtworkImage[] | null>(null);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState("");
  const [searchWarnings, setSearchWarnings] = useState<string[]>([]);
  const [imageError, setImageError] = useState("");
  const [reload, setReload] = useState(0);
  const searchRequest = useRef(0);
  const imageKind = selectedGame?.source === "wikipedia" ? "grid" : kind;

  function isCurrentImage(image: ArtworkImage) {
    return game.artwork?.source === selectedGame?.source &&
      game.artwork?.gameId === selectedGame?.id &&
      game.artwork?.imageId === image.id &&
      game.artwork?.kind === imageKind;
  }

  async function search(name: string) {
    const request = ++searchRequest.current;
    setSearching(true);
    setSearchError("");
    setSearchWarnings([]);
    setImageError("");
    setGames(null);
    setSelectedGame(null);
    setImages(null);
    try {
      const result = await searchArtworkGames(name);
      if (request !== searchRequest.current) return;
      const found = result.games;
      setGames(found);
      setSearchWarnings(result.warnings);
      const previous = found.find(
        (result) => result.source === game.artwork?.source && result.id === game.artwork?.gameId,
      );
      const exact = found.filter(
        (result) => normalize(result.name) === normalize(name),
      );
      const steamExact = exact.filter((result) => result.source === "steam");
      const preferred = steamExact.length > 0 ? steamExact : exact;
      setSelectedGame(previous ?? (preferred.length === 1 ? preferred[0] : null));
      setPage(0);
    } catch (error) {
      if (request === searchRequest.current) setSearchError(errMsg(error));
    } finally {
      if (request === searchRequest.current) setSearching(false);
    }
  }

  // This component mounts only after the user presses "Vybrať obrázok".
  useEffect(() => {
    void search(game.name);
    return () => {
      searchRequest.current += 1;
    };
    // Loading this picker is an explicit user action.
  }, [game.id]);

  useEffect(() => {
    if (!selectedGame) return;
    let cancelled = false;
    setImages(null);
    setImageError("");
    void listArtworkImages(selectedGame.id, imageKind, page, selectedGame.source).then(
      (found) => {
        if (!cancelled) setImages(found);
      },
      (error) => {
        if (!cancelled) setImageError(errMsg(error));
      },
    );
    return () => {
      cancelled = true;
    };
  }, [selectedGame, imageKind, page, reload]);

  async function choose(image: ArtworkImage) {
    if (!selectedGame) return;
    const saved = await run(
      `image:${image.id}`,
      () => selectGameArtwork(game.id, selectedGame.id, imageKind, page, image.id, selectedGame.source),
      "Obrázok hry bol uložený.",
    );
    if (saved) onClose();
  }

  async function clear() {
    const saved = await run(
      "clear",
      () => clearGameArtwork(game.id),
      "Pôvodné pozadie bolo obnovené.",
    );
    if (saved) onClose();
  }

  return (
    <Modal
      title={`Vybrať obrázok · ${game.name}`}
      onClose={() => {
        if (!busy) onClose();
      }}
      wide
    >
      <div className="flex flex-col gap-4">
        <>
            <form
              className="flex items-end gap-2"
              onSubmit={(event) => {
                event.preventDefault();
                if (query.trim() && !busy) void search(query.trim());
              }}
            >
              <div className="min-w-0 flex-1">
                <label className={labelCls} htmlFor="artwork-search">
                  Názov hry
                </label>
                <input
                  id="artwork-search"
                  className={inputCls}
                  value={query}
                  maxLength={200}
                  disabled={busy !== null}
                  onChange={(event) => setQuery(event.currentTarget.value)}
                />
              </div>
              <Button
                type="submit"
                loading={searching}
                disabled={busy !== null || !query.trim()}
              >
                Vyhľadať
              </Button>
            </form>

            <p className="text-xs text-zinc-500">
              Hľadáme na Steame aj vo Wikipédii vrátane hier mimo Steamu. Bez prihlásenia a API kľúča.
            </p>

            {searchError && (
              <p role="alert" className="text-xs text-red-400">
                {searchError}
              </p>
            )}
            {searchWarnings.map((warning) => (
              <p key={warning} role="status" className="text-xs text-amber-400">
                {warning} Zobrazené sú výsledky z dostupného zdroja.
              </p>
            ))}
            {games?.length === 0 && (
              <p className="text-sm text-zinc-400">
                Hra sa nenašla. Skús jej oficiálny alebo kratší názov.
              </p>
            )}
            {games && games.length > 0 && (
              <div>
                <label className={labelCls} htmlFor="artwork-game">
                  Hra v databáze
                </label>
                <select
                  id="artwork-game"
                  className={inputCls}
                  disabled={busy !== null}
                  value={selectedGame ? gameKey(selectedGame) : ""}
                  onChange={(event) => {
                    setSelectedGame(
                      games.find(
                        (result) =>
                          gameKey(result) === event.currentTarget.value,
                      ) ?? null,
                    );
                    setPage(0);
                  }}
                >
                  <option value="" disabled>
                    Vyber správnu hru…
                  </option>
                  {games.map((result) => (
                    <option key={gameKey(result)} value={gameKey(result)}>
                      {result.name} · {artworkSourceLabel(result.source)}
                    </option>
                  ))}
                </select>
              </div>
            )}

            {selectedGame && (
              <>
                {selectedGame.source === "steam" && (
                <div
                  className="flex gap-2"
                  role="group"
                  aria-label="Typ obrázkov"
                >
                  {(
                    [
                      ["grid", "Obaly a pozadia"],
                      ["hero", "Screenshoty"],
                    ] as const
                  ).map(([value, label]) => (
                    <Button
                      key={value}
                      variant={kind === value ? "primary" : "secondary"}
                      aria-pressed={kind === value}
                      disabled={busy !== null}
                      onClick={() => {
                        setKind(value);
                        setPage(0);
                      }}
                    >
                      {label}
                    </Button>
                  ))}
                </div>
                )}
                {imageError ? (
                  <div
                    role="alert"
                    className="rounded-lg border border-red-500/20 bg-red-500/5 p-3"
                  >
                    <p className="text-xs text-red-400">{imageError}</p>
                    <Button
                      className="mt-2"
                      onClick={() => setReload((value) => value + 1)}
                    >
                      Skúsiť znova
                    </Button>
                  </div>
                ) : images === null ? (
                  <div
                    role="status"
                    className="flex items-center justify-center gap-2 py-12 text-sm text-zinc-400"
                  >
                    <SpinnerIcon className="h-4 w-4" /> Načítavajú sa obrázky…
                  </div>
                ) : images.length === 0 ? (
                  <p className="py-6 text-center text-sm text-zinc-400">
                    {selectedGame.source === "wikipedia"
                      ? "Wikipedia pre túto hru nemá dostupný obal. Vyber inú hru alebo edíciu zo zoznamu."
                      : `Žiadne ďalšie obrázky. Skús ${kind === "grid" ? "Screenshoty" : "Obaly a pozadia"} alebo inú hru.`}
                  </p>
                ) : (
                  <div className="grid max-h-[38vh] grid-cols-2 gap-3 overflow-y-auto overscroll-contain pr-1">
                    {images.map((image) => (
                      <button
                        type="button"
                        key={image.id}
                        disabled={busy !== null}
                        className={`group overflow-hidden rounded-xl border bg-zinc-950 text-left transition-colors focus-visible:outline-2 focus-visible:outline-indigo-400 disabled:opacity-50 ${isCurrentImage(image) ? "border-indigo-400" : "border-zinc-700 hover:border-indigo-400"}`}
                        onClick={() => void choose(image)}
                        aria-label={`Vybrať obrázok ${image.id} zo zdroja ${artworkSourceLabel(selectedGame.source)}`}
                      >
                        <div className="relative aspect-[16/9] overflow-hidden bg-zinc-800">
                          <GamepadIcon className="absolute left-1/2 top-1/2 h-9 w-9 -translate-x-1/2 -translate-y-1/2 text-zinc-600" />
                          <img
                            key={image.thumb}
                            src={image.thumb}
                            alt=""
                            loading="lazy"
                            referrerPolicy="no-referrer"
                            className={`relative h-full w-full ${selectedGame.source === "wikipedia" ? "object-contain" : "object-cover"}`}
                            onError={(event) => {
                              event.currentTarget.style.visibility = "hidden";
                            }}
                          />
                          {busy === `image:${image.id}` && (
                            <div className="absolute inset-0 flex items-center justify-center bg-black/60">
                              <SpinnerIcon className="h-6 w-6 text-white" />
                            </div>
                          )}
                        </div>
                        <div className="px-3 py-2">
                          <p className="truncate text-xs text-zinc-200">
                            {image.author.name || artworkSourceLabel(selectedGame.source)}
                          </p>
                          <p className="mt-0.5 text-[10px] text-zinc-500">
                            {image.width && image.height ? `${image.width} × ${image.height}` : artworkSourceLabel(selectedGame.source)}
                            {isCurrentImage(image)
                              ? " · Aktuálny obrázok"
                              : " · Vybrať obrázok"}
                          </p>
                        </div>
                      </button>
                    ))}
                  </div>
                )}
                {selectedGame.source === "steam" && (
                <div className="flex items-center justify-between gap-2 text-xs text-zinc-500">
                  <Button
                    disabled={busy !== null || page === 0}
                    onClick={() => setPage((value) => value - 1)}
                  >
                    Predchádzajúce
                  </Button>
                  <span>Strana {page + 1}</span>
                  <Button
                    disabled={busy !== null || !images || images.length < 50}
                    onClick={() => setPage((value) => value + 1)}
                  >
                    Ďalšie obrázky
                  </Button>
                </div>
                )}
                <p className="text-xs text-zinc-500">
                  Zdroj obrázkov · {artworkSourceLabel(selectedGame.source)}. Kliknutím na obrázok sa výber uloží.
                </p>
              </>
            )}
        </>
        <div className="flex flex-wrap justify-between gap-2 border-t border-zinc-800 pt-4">
          <Button
            variant="ghost"
            disabled={busy !== null}
            loading={busy === "clear"}
            onClick={() => void clear()}
          >
            Použiť pôvodné pozadie
          </Button>
          <Button disabled={busy !== null} onClick={onClose}>
            Späť k nastaveniam hry
          </Button>
        </div>
      </div>
    </Modal>
  );
}
