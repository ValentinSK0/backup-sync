import { useEffect, useState, type ReactNode } from "react";
import {
  customCloudLogin,
  customCloudLogout,
  gdriveConnect,
  gdriveDisconnect,
  saveGdriveCredentials,
  uloztoLogin,
  uloztoLogout,
  updateSettings,
} from "../api";
import { CloudIcon } from "../components/icons";
import {
  Button,
  Card,
  PageHeader,
  Toggle,
  inputCls,
  labelCls,
} from "../components/ui";
import { useApp } from "../context";
import { useConfigAction } from "../hooks";
import type { CompressionLevel, ProviderStatus } from "../types";

function Section({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <Card className="p-5">
      <h2 className="text-sm font-semibold text-zinc-100">{title}</h2>
      {description && (
        <p className="mt-1 text-xs text-zinc-500">{description}</p>
      )}
      <div className="mt-4">{children}</div>
    </Card>
  );
}

function StatusRow({ status, label }: { status: ProviderStatus; label: string }) {
  return (
    <div className="flex items-center gap-2.5 rounded-lg border border-zinc-800 bg-zinc-950/60 px-3 py-2.5">
      <span
        className={`h-2.5 w-2.5 shrink-0 rounded-full ${status.connected ? "bg-emerald-400" : "bg-zinc-600"}`}
      />
      <div className="min-w-0 flex-1 text-sm">
        {status.connected ? (
          <span className="text-zinc-200">
            Pripojené{status.account ? ` ako ${status.account}` : ""}
          </span>
        ) : (
          <span className="text-zinc-500">Nepripojené — {label}</span>
        )}
        {status.endpoint && (
          <div className="truncate font-mono text-xs text-zinc-500">
            {status.endpoint}
          </div>
        )}
        {status.error && (
          <div className="break-words text-xs text-red-400">{status.error}</div>
        )}
      </div>
    </div>
  );
}

export default function Settings() {
  const { config, setConfig, toast } = useApp();
  const { busy, run } = useConfigAction();
  const settings = config.settings;

  // --- Sekcia 1: Zálohovanie (draft + uložiť) ---
  const [autoBackupFolders, setAutoBackupFolders] = useState(
    settings.autoBackupFolders,
  );
  const [autoUpload, setAutoUpload] = useState(settings.autoUploadSaves);
  const [compression, setCompression] = useState<CompressionLevel>(
    settings.compressionLevel,
  );
  const [debounce, setDebounce] = useState(String(settings.debounceSeconds));

  // --- Sekcia 2: Google Drive OAuth údaje (draft + uložiť) ---
  const [clientId, setClientId] = useState(settings.gdriveClientId);
  const [clientSecret, setClientSecret] = useState("");

  useEffect(() => {
    setAutoBackupFolders(settings.autoBackupFolders);
    setAutoUpload(settings.autoUploadSaves);
    setCompression(settings.compressionLevel);
    setDebounce(String(settings.debounceSeconds));
    setClientId(settings.gdriveClientId);
    setClientSecret("");
  }, [settings]);

  const backupDirty =
    autoBackupFolders !== settings.autoBackupFolders ||
    autoUpload !== settings.autoUploadSaves ||
    compression !== settings.compressionLevel ||
    debounce !== String(settings.debounceSeconds);

  const googleDirty =
    clientId.trim() !== settings.gdriveClientId || clientSecret.length > 0;

  function saveBackup() {
    const seconds = Number(debounce);
    if (
      !Number.isFinite(seconds) ||
      seconds < 1 ||
      seconds > 3600 ||
      !Number.isInteger(seconds)
    ) {
      toast("error", "Oneskorenie musí byť celé číslo od 1 do 3600 sekúnd.");
      return;
    }
    void run(
      "saveBackup",
      () =>
        updateSettings({
          ...settings,
          autoBackupFolders,
          autoUploadSaves: autoUpload,
          compressionLevel: compression,
          debounceSeconds: seconds,
        }),
      "Nastavenia zálohovania uložené.",
    );
  }

  async function saveGoogleCredentials() {
    const normalizedClientId = clientId.trim();
    const normalizedSecret = clientSecret.trim();
    if (!normalizedClientId) {
      toast("error", "Vyplňte OAuth Client ID.");
      return;
    }
    if (
      (!settings.gdriveClientSecretConfigured ||
        normalizedClientId !== settings.gdriveClientId) &&
      !normalizedSecret
    ) {
      toast("error", "Vyplňte OAuth Client Secret.");
      return;
    }
    const saved = await run(
      "saveGoogleCredentials",
      () => saveGdriveCredentials(normalizedClientId, normalizedSecret),
      "Google OAuth údaje uložené.",
    );
    if (saved) setClientSecret("");
  }

  // --- Sekcia 3: Vlastný cloud (login formulár) ---
  const [endpoint, setEndpoint] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [loggingIn, setLoggingIn] = useState(false);

  async function handleLogin() {
    if (!endpoint.trim() || !username.trim() || !password) {
      toast("error", "Vyplňte endpoint, meno aj heslo.");
      return;
    }
    setLoggingIn(true);
    try {
      const cfg = await customCloudLogin(
        endpoint.trim(),
        username.trim(),
        password,
      );
      setConfig(cfg);
      toast("success", "Prihlásenie do vlastného cloudu úspešné.");
      setEndpoint("");
      setUsername("");
    } catch (e) {
      toast("error", typeof e === "string" ? e : String(e));
    } finally {
      // heslo nikdy nedržíme v state dlhšie, než je nutné
      setPassword("");
      setLoggingIn(false);
    }
  }

  // --- Sekcia 4: Ulož.to Disk (login + aplikačný token) ---
  const [uloztoUser, setUloztoUser] = useState("");
  const [uloztoToken, setUloztoToken] = useState("");
  const [uloztoAppKey, setUloztoAppKey] = useState("");
  const [uloztoLoggingIn, setUloztoLoggingIn] = useState(false);

  async function handleUloztoLogin() {
    if (!uloztoUser.trim() || !uloztoToken.trim()) {
      toast("error", "Vyplňte Ulož.to login aj aplikačný token.");
      return;
    }
    setUloztoLoggingIn(true);
    try {
      const cfg = await uloztoLogin(
        uloztoUser.trim(),
        uloztoToken.trim(),
        uloztoAppKey.trim(),
      );
      setConfig(cfg);
      toast("success", "Prihlásenie do Ulož.to úspešné.");
      setUloztoUser("");
    } catch (e) {
      toast("error", typeof e === "string" ? e : String(e));
    } finally {
      // tokeny nedržíme v state dlhšie, než je nutné
      setUloztoToken("");
      setUloztoAppKey("");
      setUloztoLoggingIn(false);
    }
  }

  const gdrive = config.providers.gdrive;
  const custom = config.providers.customCloud;
  const ulozto = config.providers.ulozto;

  return (
    <div>
      <PageHeader
        title="Nastavenia"
        subtitle="Zálohovanie, pripojenie ku Google Drive a vlastnému cloudu."
      />

      <div className="flex max-w-2xl flex-col gap-5">
        <Section
          title="Zálohovanie"
          description="Správanie automatického nahrávania a kompresie záloh."
        >
          <div className="flex flex-col gap-4">
            <div className="flex items-center justify-between gap-4">
              <div>
                <div className="text-sm text-zinc-200">
                  Automaticky zálohovať priečinky po zmene
                </div>
                <div className="text-xs text-zinc-500">
                  Globálne nastavenie — priečinky ho môžu individuálne prepísať.
                </div>
              </div>
              <Toggle
                checked={autoBackupFolders}
                onChange={setAutoBackupFolders}
              />
            </div>

            <div className="flex items-center justify-between gap-4">
              <div>
                <div className="text-sm text-zinc-200">
                  Automaticky nahrať save po zmene
                </div>
                <div className="text-xs text-zinc-500">
                  Globálne nastavenie — hry ho môžu individuálne prepísať.
                </div>
              </div>
              <Toggle checked={autoUpload} onChange={setAutoUpload} />
            </div>

            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className={labelCls} htmlFor="compression">
                  Kompresia
                </label>
                <select
                  id="compression"
                  className={inputCls}
                  value={compression}
                  onChange={(e) =>
                    setCompression(e.currentTarget.value as CompressionLevel)
                  }
                >
                  <option value="fast">Rýchla</option>
                  <option value="best">Úsporná</option>
                </select>
              </div>
              <div>
                <label className={labelCls} htmlFor="debounce">
                  Oneskorenie po zmene (sekundy)
                </label>
                <input
                  id="debounce"
                  type="number"
                  min={1}
                  max={3600}
                  step={1}
                  className={inputCls}
                  value={debounce}
                  onChange={(e) => setDebounce(e.currentTarget.value)}
                />
              </div>
            </div>

            <div className="flex justify-end">
              <Button
                variant="primary"
                onClick={saveBackup}
                disabled={!backupDirty}
                loading={busy === "saveBackup"}
              >
                Uložiť
              </Button>
            </div>
          </div>
        </Section>

        <Section
          title="Google Drive"
          description="OAuth údaje typu „Desktop app“. Client Secret sa bezpečne uloží vo Windows Credential Manager, nie v configu."
        >
          <div className="flex flex-col gap-4">
            <StatusRow status={gdrive} label="prihláste sa cez Google" />

            <div>
              <label className={labelCls} htmlFor="gdrive-client-id">
                OAuth Client ID
              </label>
              <input
                id="gdrive-client-id"
                className={`${inputCls} font-mono !text-xs`}
                value={clientId}
                onChange={(e) => setClientId(e.currentTarget.value)}
                placeholder="xxxxxxxx.apps.googleusercontent.com"
                spellCheck={false}
              />
              <label
                className={`${labelCls} mt-3`}
                htmlFor="gdrive-client-secret"
              >
                OAuth Client Secret
              </label>
              <input
                id="gdrive-client-secret"
                type="password"
                className={`${inputCls} font-mono !text-xs`}
                value={clientSecret}
                onChange={(e) => setClientSecret(e.currentTarget.value)}
                placeholder={
                  settings.gdriveClientSecretConfigured
                    ? "Uložený — prázdne pole ho nemení"
                    : "Zadajte Client Secret"
                }
                autoComplete="new-password"
                spellCheck={false}
              />
              <p className="mt-1.5 text-xs text-zinc-500">
                {settings.gdriveClientSecretConfigured
                  ? "Client Secret je uložený v zabezpečenom úložisku Windows."
                  : "Client Secret chýba. Bez neho sa tento Google OAuth klient neprihlási."}
              </p>
              <div className="mt-2 flex justify-end">
                <Button
                  variant="secondary"
                  onClick={() => void saveGoogleCredentials()}
                  disabled={!googleDirty}
                  loading={busy === "saveGoogleCredentials"}
                >
                  Uložiť OAuth údaje
                </Button>
              </div>
            </div>

            <div className="flex items-center gap-2">
              {gdrive.connected ? (
                <Button
                  variant="secondary"
                  loading={busy === "gdriveDisconnect"}
                  onClick={() =>
                    run(
                      "gdriveDisconnect",
                      gdriveDisconnect,
                      "Google Drive odpojený.",
                    )
                  }
                >
                  Odpojiť
                </Button>
              ) : (
                <>
                  <Button
                    variant="primary"
                    loading={busy === "gdriveConnect"}
                    disabled={
                      !settings.gdriveClientId ||
                      !settings.gdriveClientSecretConfigured ||
                      googleDirty
                    }
                    onClick={() =>
                      run(
                        "gdriveConnect",
                        gdriveConnect,
                        "Google Drive pripojený.",
                      )
                    }
                  >
                    <CloudIcon className="h-4 w-4" />
                    Pripojiť
                  </Button>
                  {(!settings.gdriveClientId ||
                    !settings.gdriveClientSecretConfigured ||
                    googleDirty) && (
                    <span className="text-xs text-zinc-500">
                      Najprv uložte Client ID aj Client Secret.
                    </span>
                  )}
                </>
              )}
            </div>
          </div>
        </Section>

        <Section
          title="Ulož.to Disk"
          description="Prihlásenie loginom a aplikačným tokenom (Ulož.to → Nastavenia → aplikačný login token)."
        >
          <div className="flex flex-col gap-4">
            <StatusRow status={ulozto} label="prihláste sa nižšie" />

            {ulozto.connected ? (
              <div>
                <Button
                  variant="secondary"
                  loading={busy === "uloztoLogout"}
                  onClick={() =>
                    run(
                      "uloztoLogout",
                      uloztoLogout,
                      "Odhlásenie z Ulož.to úspešné.",
                    )
                  }
                >
                  Odhlásiť
                </Button>
              </div>
            ) : (
              <form
                className="flex flex-col gap-3"
                onSubmit={(e) => {
                  e.preventDefault();
                  void handleUloztoLogin();
                }}
              >
                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className={labelCls} htmlFor="ul-login">
                      Login
                    </label>
                    <input
                      id="ul-login"
                      className={inputCls}
                      value={uloztoUser}
                      onChange={(e) => setUloztoUser(e.currentTarget.value)}
                      autoComplete="username"
                      spellCheck={false}
                    />
                  </div>
                  <div>
                    <label className={labelCls} htmlFor="ul-token">
                      Aplikačný token
                    </label>
                    <input
                      id="ul-token"
                      type="password"
                      className={inputCls}
                      value={uloztoToken}
                      onChange={(e) => setUloztoToken(e.currentTarget.value)}
                      autoComplete="off"
                    />
                  </div>
                </div>
                <div>
                  <label className={labelCls} htmlFor="ul-appkey">
                    API kľúč aplikácie (voliteľný)
                  </label>
                  <input
                    id="ul-appkey"
                    type="password"
                    className={inputCls}
                    value={uloztoAppKey}
                    onChange={(e) => setUloztoAppKey(e.currentTarget.value)}
                    placeholder="prázdne = verejný testovací kľúč"
                    autoComplete="off"
                  />
                  <p className="mt-1.5 text-xs text-zinc-500">
                    Vlastný API kľúč sa žiada od podpory Ulož.to. Bez neho sa
                    použije verejný testovací kľúč, ktorý sa môže kedykoľvek
                    zmeniť.
                  </p>
                </div>
                <div className="flex justify-end">
                  <Button
                    variant="primary"
                    type="submit"
                    loading={uloztoLoggingIn}
                  >
                    Prihlásiť
                  </Button>
                </div>
              </form>
            )}
          </div>
        </Section>

        <Section
          title="Vlastný cloud"
          description="Pripojenie k vlastnému úložisku cez endpoint URL."
        >
          <div className="flex flex-col gap-4">
            <StatusRow status={custom} label="prihláste sa nižšie" />

            {custom.connected ? (
              <div>
                <Button
                  variant="secondary"
                  loading={busy === "customLogout"}
                  onClick={() =>
                    run(
                      "customLogout",
                      customCloudLogout,
                      "Odhlásenie z vlastného cloudu úspešné.",
                    )
                  }
                >
                  Odhlásiť
                </Button>
              </div>
            ) : (
              <form
                className="flex flex-col gap-3"
                onSubmit={(e) => {
                  e.preventDefault();
                  void handleLogin();
                }}
              >
                <div>
                  <label className={labelCls} htmlFor="cc-endpoint">
                    Endpoint URL
                  </label>
                  <input
                    id="cc-endpoint"
                    className={inputCls}
                    value={endpoint}
                    onChange={(e) => setEndpoint(e.currentTarget.value)}
                    placeholder="https://cloud.example.com"
                    spellCheck={false}
                  />
                </div>
                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className={labelCls} htmlFor="cc-username">
                      Používateľské meno
                    </label>
                    <input
                      id="cc-username"
                      className={inputCls}
                      value={username}
                      onChange={(e) => setUsername(e.currentTarget.value)}
                      autoComplete="username"
                    />
                  </div>
                  <div>
                    <label className={labelCls} htmlFor="cc-password">
                      Heslo
                    </label>
                    <input
                      id="cc-password"
                      type="password"
                      className={inputCls}
                      value={password}
                      onChange={(e) => setPassword(e.currentTarget.value)}
                      autoComplete="current-password"
                    />
                  </div>
                </div>
                <div className="flex justify-end">
                  <Button variant="primary" type="submit" loading={loggingIn}>
                    Prihlásiť
                  </Button>
                </div>
              </form>
            )}
          </div>
        </Section>
      </div>
    </div>
  );
}
