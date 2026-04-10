import { useEffect, useMemo, useRef, useState } from "react";
import {
  AsrLanguage,
  LanguageConfig,
  LiveTranscriptLine,
  ModelDownloadProgress,
  ModelInventoryItem,
  ModelOption,
  SessionSnapshot,
  cancelModelDownload,
  deleteModel,
  discardTranscript,
  getAudioLevel,
  getLanguageConfig,
  getModelCatalog,
  getModelDownloadProgress,
  getModelInventory,
  getSessionSnapshot,
  getSourceState,
  pollLiveTranscriptLines,
  saveTranscript,
  setActiveModel,
  setLanguageConfig as setAsrLanguageConfig,
  setMicEnabled,
  setSystemAudioEnabled,
  startModelDownloadById,
  startListening,
  stopListening
} from "./backend";
import { CompanionShiba } from "./CompanionShiba";

const defaultSnapshot: SessionSnapshot = {
  state: "ready",
  offline_mode_active: false,
  transcript_line_count: 0,
  last_error: null
};

const defaultModelDownload: ModelDownloadProgress = {
  in_progress: false,
  progress: 0,
  downloaded_bytes: 0,
  total_bytes: null,
  installed: false,
  model_id: null,
  model_name: null,
  last_error: null
};

const defaultLanguageConfig: LanguageConfig = {
  source_language: "japanese",
  target_language: "english"
};

const METER_SEGMENTS = 44;

export function App() {
  const [snapshot, setSnapshot] = useState<SessionSnapshot>(defaultSnapshot);
  const [modelDownload, setModelDownload] = useState<ModelDownloadProgress>(defaultModelDownload);
  const [languageConfig, setLanguageConfigState] = useState<LanguageConfig>(defaultLanguageConfig);
  const [modelCatalog, setModelCatalog] = useState<ModelOption[]>([]);
  const [modelInventory, setModelInventory] = useState<ModelInventoryItem[]>([]);
  const [selectedModelId, setSelectedModelId] = useState("large-v3");
  const [error, setError] = useState<string | null>(null);
  const [sessionActionPending, setSessionActionPending] = useState<"start" | "stop" | null>(null);
  const [modelDownloadPending, setModelDownloadPending] = useState(false);
  const [modelCancelPending, setModelCancelPending] = useState(false);
  const [modelActionPending, setModelActionPending] = useState<false | "switch" | "delete">(false);
  const [modelManagerOpen, setModelManagerOpen] = useState(false);
  const [modelPromptDismissed, setModelPromptDismissed] = useState(false);
  const [savedTranscript, setSavedTranscript] = useState<string>("");
  const [micEnabled, setMicEnabledLocal] = useState(true);
  const [systemEnabled, setSystemEnabledLocal] = useState(false);
  const [audioLevel, setAudioLevel] = useState(0);
  const [transcriptLines, setTranscriptLines] = useState<string[]>([]);
  const [pendingLineCount, setPendingLineCount] = useState(0);
  const captionPanelRef = useRef<HTMLElement | null>(null);
  const isNearBottomRef = useRef(true);

  const isListening = snapshot.state === "listening";
  const isAwaitingDecision = snapshot.state === "prompting_save_discard";
  const isModelMissing = snapshot.state === "model_missing";
  const isDownloadingModel = snapshot.state === "downloading_model" || modelDownload.in_progress;
  const isModelPromptVisible =
    modelManagerOpen || isDownloadingModel || (isModelMissing && !modelPromptDismissed);
  const isStopping = snapshot.state === "stopping" || sessionActionPending === "stop";
  const installedModels = useMemo(
    () => modelInventory.filter((model) => model.installed),
    [modelInventory]
  );
  const hasEnabledSource = micEnabled || systemEnabled;
  const hasInstalledModel =
    installedModels.length > 0 ||
    snapshot.state === "ready" ||
    snapshot.state === "listening" ||
    snapshot.state === "stopping" ||
    snapshot.state === "prompting_save_discard" ||
    snapshot.state === "saving_transcript";
  const noModelInstalled = !hasInstalledModel;
  const readyStatusActive = snapshot.state === "ready" || snapshot.state === "listening";
  const controlsLocked = isAwaitingDecision || sessionActionPending !== null;
  const startDisabled =
    controlsLocked ||
    isStopping ||
    isModelMissing ||
    isDownloadingModel ||
    !hasEnabledSource ||
    !hasInstalledModel;
  const modelProgressPercent = Math.round(Math.max(0, Math.min(1, modelDownload.progress)) * 100);
  const activeModelId = useMemo(
    () => modelInventory.find((model) => model.active)?.id ?? null,
    [modelInventory]
  );
  const currentModelName =
    modelInventory.find((model) => model.id === activeModelId)?.name ??
    modelDownload.model_name ??
    "No model";
  const selectedCatalogModel =
    modelCatalog.find((model) => model.id === selectedModelId) ?? null;
  const selectedModelDownloadable = selectedCatalogModel?.downloadable ?? false;
  const isModelControlsDisabled = isListening || isDownloadingModel || modelActionPending !== false;

  useEffect(() => {
    if (!isModelMissing && !isDownloadingModel) {
      setModelPromptDismissed(false);
    }
  }, [isModelMissing, isDownloadingModel]);

  useEffect(() => {
    const suppressContextMenu = (event: MouseEvent) => {
      event.preventDefault();
    };

    const suppressDevShortcuts = (event: KeyboardEvent) => {
      const key = event.key.toLowerCase();
      const inspectShortcut =
        key === "f12" ||
        ((event.ctrlKey || event.metaKey) &&
          event.shiftKey &&
          (key === "i" || key === "j" || key === "c")) ||
        (event.metaKey && event.altKey && (key === "i" || key === "c"));
      const refreshShortcut =
        key === "f5" || ((event.ctrlKey || event.metaKey) && key === "r");

      if (inspectShortcut || refreshShortcut) {
        event.preventDefault();
        event.stopPropagation();
      }
    };

    window.addEventListener("contextmenu", suppressContextMenu);
    window.addEventListener("keydown", suppressDevShortcuts, true);

    return () => {
      window.removeEventListener("contextmenu", suppressContextMenu);
      window.removeEventListener("keydown", suppressDevShortcuts, true);
    };
  }, []);

  useEffect(() => {
    void refreshSnapshot();
    void refreshSources();
    void refreshModelDownloadProgress();
    void refreshLanguageConfig();
    void refreshModelCatalog();
    void refreshModelInventory();
  }, []);

  useEffect(() => {
    if (!isModelPromptVisible) {
      return undefined;
    }

    const intervalId = window.setInterval(() => {
      void Promise.all([
        refreshSnapshot(),
        refreshModelDownloadProgress(),
        refreshModelInventory()
      ]);
    }, 260);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [isModelPromptVisible]);

  useEffect(() => {
    if (!isListening) {
      setAudioLevel(0);
      return undefined;
    }

    const intervalId = window.setInterval(() => {
      void Promise.all([getAudioLevel(), pollLiveTranscriptLines()])
        .then(([level, lines]) => {
          const normalizedLevel = Math.max(0, Math.min(1, level));
          setAudioLevel(normalizedLevel);
          appendLiveTranscriptLines(lines);
        })
        .catch((requestError) => {
          setError(String(requestError));
        });
    }, 180);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [isListening]);

  useEffect(() => {
    if (snapshot.state !== "stopping") {
      return undefined;
    }

    const intervalId = window.setInterval(() => {
      void refreshSnapshot();
    }, 200);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [snapshot.state]);

  async function refreshSnapshot(): Promise<void> {
    try {
      setSnapshot(await getSessionSnapshot());
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
    }
  }

  async function refreshSources(): Promise<void> {
    try {
      const sources = await getSourceState();
      setMicEnabledLocal(sources.mic_enabled);
      setSystemEnabledLocal(sources.system_audio_enabled);
    } catch (requestError) {
      setError(String(requestError));
    }
  }

  async function refreshModelDownloadProgress(): Promise<void> {
    try {
      setModelDownload(await getModelDownloadProgress());
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
    }
  }

  async function refreshLanguageConfig(): Promise<void> {
    try {
      setLanguageConfigState(await getLanguageConfig());
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
    }
  }

  async function refreshModelCatalog(): Promise<void> {
    try {
      const catalog = await getModelCatalog();
      setModelCatalog(catalog);
      setSelectedModelId((current) => {
        if (catalog.some((model) => model.id === current)) {
          return current;
        }
        const recommended = catalog.find((model) => model.recommended)?.id;
        return recommended ?? catalog[0]?.id ?? current;
      });
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
    }
  }

  async function refreshModelInventory(): Promise<void> {
    try {
      const inventory = await getModelInventory();
      setModelInventory(inventory);
      const active = inventory.find((model) => model.active)?.id;
      if (active) {
        setSelectedModelId(active);
      }
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
    }
  }

  async function onStart(): Promise<void> {
    if (sessionActionPending !== null || isModelMissing || isDownloadingModel) {
      return;
    }

    setSessionActionPending("start");
    try {
      setSnapshot(await startListening());
      setSavedTranscript("");
      setTranscriptLines([]);
      setPendingLineCount(0);
      window.requestAnimationFrame(() => scrollToLatest());
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
      await refreshSnapshot();
    } finally {
      setSessionActionPending(null);
    }
  }

  async function onStop(): Promise<void> {
    if (sessionActionPending !== null) {
      return;
    }

    setSessionActionPending("stop");
    try {
      setSnapshot(await stopListening());
      setPendingLineCount(0);
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
      await refreshSnapshot();
    } finally {
      setSessionActionPending(null);
    }
  }

  async function onDiscard(): Promise<void> {
    try {
      setSnapshot(await discardTranscript());
      setSavedTranscript("");
      setTranscriptLines([]);
      setPendingLineCount(0);
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
    }
  }

  async function onSave(): Promise<void> {
    try {
      const transcript = await saveTranscript();
      setSavedTranscript(transcript);
      await refreshSnapshot();
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
    }
  }

  async function onDownloadModel(): Promise<void> {
    if (
      modelDownloadPending ||
      isDownloadingModel ||
      selectedModelId.length === 0 ||
      !selectedModelDownloadable
    ) {
      return;
    }

    setModelDownloadPending(true);
    try {
      setModelDownload(await startModelDownloadById(selectedModelId));
      await Promise.all([refreshSnapshot(), refreshModelInventory()]);
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
      await refreshSnapshot();
      await Promise.all([refreshModelDownloadProgress(), refreshModelInventory()]);
    } finally {
      setModelDownloadPending(false);
    }
  }

  async function onCancelModelDownload(): Promise<void> {
    if (!isDownloadingModel || modelCancelPending) {
      return;
    }

    setModelCancelPending(true);
    try {
      setModelDownload(await cancelModelDownload());
      await Promise.all([refreshSnapshot(), refreshModelDownloadProgress(), refreshModelInventory()]);
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
      await Promise.all([refreshSnapshot(), refreshModelDownloadProgress(), refreshModelInventory()]);
    } finally {
      setModelCancelPending(false);
    }
  }

  async function onActivateModel(modelId: string): Promise<void> {
    if (modelId.length === 0 || modelActionPending !== false || isListening || isDownloadingModel) {
      return;
    }

    setModelActionPending("switch");
    try {
      setModelInventory(await setActiveModel(modelId));
      setSelectedModelId(modelId);
      await refreshSnapshot();
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
      await Promise.all([refreshSnapshot(), refreshModelInventory()]);
    } finally {
      setModelActionPending(false);
    }
  }

  async function onDeleteModel(modelId: string): Promise<void> {
    if (modelActionPending !== false || isListening || isDownloadingModel) {
      return;
    }

    setModelActionPending("delete");
    try {
      setModelInventory(await deleteModel(modelId));
      await refreshSnapshot();
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
      await Promise.all([refreshSnapshot(), refreshModelInventory()]);
    } finally {
      setModelActionPending(false);
    }
  }

  function closeModelManagerModal(): void {
    setModelManagerOpen(false);
    if (isModelMissing) {
      setModelPromptDismissed(true);
    }
  }

  async function onSourceLanguageChange(nextSource: AsrLanguage): Promise<void> {
    try {
      setLanguageConfigState(
        await setAsrLanguageConfig(nextSource, languageConfig.target_language)
      );
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
      await refreshLanguageConfig();
    }
  }

  async function onSwapLanguages(): Promise<void> {
    if (isListening) {
      return;
    }

    try {
      setLanguageConfigState(
        await setAsrLanguageConfig(
          languageConfig.target_language,
          languageConfig.source_language
        )
      );
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
      await refreshLanguageConfig();
    }
  }

  async function onTargetLanguageChange(nextTarget: AsrLanguage): Promise<void> {
    try {
      setLanguageConfigState(
        await setAsrLanguageConfig(languageConfig.source_language, nextTarget)
      );
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
      await refreshLanguageConfig();
    }
  }

  function appendLiveTranscriptLines(lines: LiveTranscriptLine[]): void {
    if (lines.length === 0) {
      return;
    }

    const shouldAutoScroll = isNearBottomRef.current;
    const formatted = lines.map(
      (line) => `[${formatTimestamp(line.timestamp_ms)}] [${formatSource(line.source)}] ${line.text}`
    );

    setTranscriptLines((existing) => [...existing.slice(-180), ...formatted]);
    if (shouldAutoScroll) {
      window.requestAnimationFrame(() => scrollToLatest());
    } else {
      setPendingLineCount((count) => count + lines.length);
    }
  }

  async function onToggleMic(): Promise<void> {
    try {
      const next = await setMicEnabled(!micEnabled);
      setMicEnabledLocal(next.mic_enabled);
      setSystemEnabledLocal(next.system_audio_enabled);
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
    }
  }

  async function onToggleSystem(): Promise<void> {
    try {
      const next = await setSystemAudioEnabled(!systemEnabled);
      setMicEnabledLocal(next.mic_enabled);
      setSystemEnabledLocal(next.system_audio_enabled);
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
    }
  }

  const micLitSegments = Math.round(audioLevel * METER_SEGMENTS);
  const micMeterSegments = useMemo(
    () => createMeterSegments(micLitSegments, micEnabled, isListening),
    [isListening, micEnabled, micLitSegments]
  );
  const systemMeterSegments = useMemo(
    () => createMeterSegments(0, systemEnabled, isListening),
    [isListening, systemEnabled]
  );
  const targetLanguageOptions = useMemo(
    () => ["english", "japanese"] as AsrLanguage[],
    []
  );

  function scrollToLatest(): void {
    const panel = captionPanelRef.current;
    if (!panel) {
      return;
    }

    panel.scrollTop = panel.scrollHeight;
    isNearBottomRef.current = true;
    setPendingLineCount(0);
  }

  function handleCaptionScroll(): void {
    const panel = captionPanelRef.current;
    if (!panel) {
      return;
    }

    const nearBottom = panel.scrollHeight - panel.scrollTop - panel.clientHeight < 32;
    isNearBottomRef.current = nearBottom;
    if (nearBottom && pendingLineCount > 0) {
      setPendingLineCount(0);
    }
  }

  return (
    <>
      <main
        className={`layout ${isAwaitingDecision || isModelPromptVisible ? "modal-active" : ""}`}
      >
        <header className="topbar">
          <div className="topbar-left">
            <div className="brand">
              <span className="glyph" aria-hidden>
                聴
              </span>
              <div>
                <p className="title">Kiku</p>
                <p className="subtitle">Local Live Captions</p>
              </div>
            </div>
            <div className="status-inline">
              <div
                className={`status-toggle ${readyStatusActive ? "ready-active" : "standby-active"}`}
                aria-label="readiness status"
              >
                <span className={`status-toggle-item ${readyStatusActive ? "active" : ""}`}>Ready</span>
                <span className={`status-toggle-item ${!readyStatusActive ? "active" : ""}`}>
                  Standby
                </span>
              </div>
              <span
                className={`status-chip ${snapshot.offline_mode_active ? "good" : "warn"}`}
              >
                {snapshot.offline_mode_active ? "Offline Mode" : "Standby"}
              </span>
            </div>
            <div className="lang-inline">
              <label className="lang-field">
                <span>Input</span>
                <select
                  value={languageConfig.source_language}
                  onChange={(event) =>
                    void onSourceLanguageChange(event.currentTarget.value as AsrLanguage)
                  }
                  disabled={isListening}
                >
                  <option value="japanese">Japanese</option>
                  <option value="english">English</option>
                </select>
              </label>
              <button
                className="lang-swap"
                onClick={() => void onSwapLanguages()}
                disabled={isListening}
                aria-label="Swap input and output languages"
                title="Swap languages"
              >
                ⇄
              </button>
              <label className="lang-field">
                <span>Output</span>
                <select
                  value={languageConfig.target_language}
                  onChange={(event) =>
                    void onTargetLanguageChange(event.currentTarget.value as AsrLanguage)
                  }
                  disabled={isListening}
                >
                  {targetLanguageOptions.map((language) => (
                    <option key={language} value={language}>
                      {formatLanguageLabel(language)}
                    </option>
                  ))}
                </select>
              </label>
            </div>
          </div>
          <div className="controls">
            <div className="model-inline">
              <span className="model-inline-label">Model</span>
              <select
                value={activeModelId ?? ""}
                onChange={(event) => void onActivateModel(event.currentTarget.value)}
                disabled={isModelControlsDisabled || installedModels.length === 0}
              >
                {installedModels.length > 0 ? (
                  installedModels.map((model) => (
                    <option key={model.id} value={model.id}>
                      {model.name}
                    </option>
                  ))
                ) : (
                  <option value="">No model installed</option>
                )}
              </select>
              <button
                className="button secondary"
                onClick={() => setModelManagerOpen(true)}
                disabled={isAwaitingDecision}
              >
                Models
              </button>
            </div>
            <button
              className="button primary"
              onClick={isListening ? onStop : onStart}
              disabled={startDisabled}
            >
              {isDownloadingModel
                ? "Downloading Model..."
                : isModelMissing
                ? "Model Required"
                : sessionActionPending === "start"
                ? "Starting..."
                : sessionActionPending === "stop" || isStopping
                  ? "Stopping..."
                  : isListening
                    ? "Stop Listening"
                    : "Start Listening"}
            </button>
            <button
              className={`button toggle ${micEnabled ? "active" : ""}`}
              onClick={onToggleMic}
              disabled={controlsLocked || isStopping}
            >
              🎙 Mic
            </button>
            <button
              className={`button toggle ${systemEnabled ? "active" : ""}`}
              onClick={onToggleSystem}
              disabled={controlsLocked || isStopping}
            >
              🔊 System
            </button>
          </div>
        </header>

        <section className="prototype-note" aria-label="prototype note">
          {isDownloadingModel ? (
            <p>Model download in progress. Kiku will enable listening automatically when setup completes.</p>
          ) : isModelMissing ? (
            <p>Model setup is required before listening can start. Use the in-app download prompt.</p>
          ) : (
            <p>
              {currentModelName} is active. Live {formatLanguageLabel(languageConfig.source_language)} to{" "}
              {formatLanguageLabel(languageConfig.target_language)} transcription is ready.
            </p>
          )}
        </section>

        <section className="caption-wrap">
          <section
            className={`caption-panel ${noModelInstalled ? "locked" : ""}`}
            aria-label="caption stream"
            ref={captionPanelRef}
            onScroll={handleCaptionScroll}
          >
            {transcriptLines.length > 0 ? (
              transcriptLines.map((line, idx) => <p key={`${line}-${idx}`}>{line}</p>)
            ) : (
              <p className="caption-empty">
                {isListening
                  ? "Listening for speech..."
                  : isDownloadingModel
                    ? "Preparing speech model..."
                  : isModelMissing
                    ? "Model setup is required before listening can start."
                    : "Start Listening to begin a session."}
              </p>
            )}
          </section>
          {noModelInstalled ? (
            <div className="caption-model-overlay">
              <p>No speech model installed. Go to Model Manager to download a model first.</p>
            </div>
          ) : null}
          {pendingLineCount > 0 ? (
            <button className="new-lines-banner" onClick={scrollToLatest}>
              ↓ New messages ({pendingLineCount})
            </button>
          ) : null}
        </section>

        <section className="visualizer" aria-label="activity widget">
          <div className="visualizer-main">
            <div
              className={`pulse ${isListening ? "live" : ""}`}
              style={
                isListening
                  ? {
                      transform: `scale(${0.92 + audioLevel * 0.55})`,
                      opacity: 0.42 + audioLevel * 0.58
                    }
                  : undefined
              }
            />
            <div>
              <p className="widget-label">listen / understand / translate</p>
              <p className="meter-lines">Lines: {snapshot.transcript_line_count}</p>
              <div className="meter-stack">
                <div className={`meter-row ${micEnabled ? "" : "disabled"}`}>
                  <span className="meter-label">MIC</span>
                  <div className="audio-meter" aria-label="microphone level meter">
                    {micMeterSegments.map((segment) => (
                      <span
                        key={`mic-${segment.idx}`}
                        className={`meter-segment ${segment.zone} ${segment.active ? "active" : ""}`}
                      />
                    ))}
                  </div>
                </div>
                <div className={`meter-row ${systemEnabled ? "" : "disabled"}`}>
                  <span className="meter-label">SYS</span>
                  <div className="audio-meter" aria-label="system audio level meter">
                    {systemMeterSegments.map((segment) => (
                      <span
                        key={`sys-${segment.idx}`}
                        className={`meter-segment ${segment.zone} ${segment.active ? "active" : ""}`}
                      />
                    ))}
                  </div>
                </div>
              </div>
              <p className="widget-subtitle">{isListening ? "Live input level" : "Waiting for input"}</p>
            </div>
          </div>
          <CompanionShiba
            isListening={isListening}
            audioLevel={audioLevel}
            micEnabled={micEnabled}
            systemEnabled={systemEnabled}
            transcriptLineCount={transcriptLines.length}
          />
        </section>

        {savedTranscript ? (
          <section className="saved-output">
            <p className="saved-title">Last Saved Transcript Preview</p>
            <pre>{savedTranscript}</pre>
          </section>
        ) : null}

        {error || snapshot.last_error ? (
          <p className="error">{error ?? snapshot.last_error}</p>
        ) : null}
      </main>

      {isAwaitingDecision ? (
        <section className="decision-modal-backdrop" role="presentation">
          <div
            className="decision-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="session-stop-title"
          >
            <h2 id="session-stop-title">Session Stopped</h2>
            <p>Save transcript before continuing.</p>
            <div className="decision-buttons">
              <button className="button secondary" onClick={onDiscard}>
                Discard
              </button>
              <button className="button primary" onClick={onSave}>
                Save
              </button>
            </div>
          </div>
        </section>
      ) : null}

      {isModelPromptVisible ? (
        <section className="decision-modal-backdrop" role="presentation">
          <div
            className="decision-modal model-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="model-manager-title"
          >
            <h2 id="model-manager-title">Model Manager</h2>
            <p>
              Choose a speech model profile for live captions. Cards tagged{" "}
              <strong>Available Now</strong> can be downloaded in this build;{" "}
              <strong>Planned</strong> cards are roadmap candidates.
            </p>
            {modelDownload.last_error ? (
              <p className="model-download-error">{modelDownload.last_error}</p>
            ) : null}
            {isDownloadingModel ? (
              <div className="model-download-live">
                <p>
                  Downloading {modelDownload.model_name ?? selectedCatalogModel?.name ?? "selected model"}...
                </p>
                <div className="model-progress-track" aria-label="model download progress">
                  <span style={{ width: `${modelProgressPercent}%` }} />
                </div>
                <p className="model-progress-meta">
                  {modelProgressPercent}% complete
                  {modelDownload.total_bytes ? (
                    <>
                      {" "}
                      ({formatBytes(modelDownload.downloaded_bytes)} /{" "}
                      {formatBytes(modelDownload.total_bytes)})
                    </>
                  ) : null}
                </p>
              </div>
            ) : null}

            <div className="model-list-scroll">
              {modelCatalog.map((model) => {
                const inventory = modelInventory.find((item) => item.id === model.id);
                const installed = inventory?.installed ?? false;
                const active = inventory?.active ?? false;

                return (
                  <div
                    key={model.id}
                    className={`model-card ${selectedModelId === model.id ? "selected" : ""} ${isDownloadingModel ? "disabled" : ""} ${model.downloadable ? "" : "planned"}`}
                    onClick={() => {
                      if (!isDownloadingModel) {
                        setSelectedModelId(model.id);
                      }
                    }}
                    role="button"
                    tabIndex={0}
                    onKeyDown={(event) => {
                      if ((event.key === "Enter" || event.key === " ") && !isDownloadingModel) {
                        event.preventDefault();
                        setSelectedModelId(model.id);
                      }
                    }}
                  >
                    <div className="model-card-head">
                      <p>{model.name}</p>
                      <div className="model-badges">
                        {model.recommended ? <span className="model-badge rec">Recommended</span> : null}
                        <span className={`model-badge ${model.downloadable ? "now" : "planned"}`}>
                          {model.downloadable ? "Available Now" : "Planned"}
                        </span>
                        {active ? <span className="model-badge active">Active</span> : null}
                        {installed ? <span className="model-badge ok">Installed</span> : null}
                      </div>
                    </div>
                    <p className="model-best-for">{model.best_for}</p>
                    <p className="model-meta">
                      {model.family} • {model.language_focus}
                    </p>
                    <div className="model-stats">
                      <span>Accuracy: {model.approx_wer}</span>
                      <span>Latency: {model.latency}</span>
                      <span>Size: {model.size}</span>
                    </div>
                    <p className="model-note">{model.notes}</p>
                    {installed ? (
                      <div className="model-actions">
                        {!active ? (
                          <button
                            className="button secondary"
                            onClick={(event) => {
                              event.stopPropagation();
                              void onActivateModel(model.id);
                            }}
                            disabled={isModelControlsDisabled}
                          >
                            Use This Model
                          </button>
                        ) : null}
                        <button
                          className="button secondary danger"
                          onClick={(event) => {
                            event.stopPropagation();
                            void onDeleteModel(model.id);
                          }}
                          disabled={isModelControlsDisabled}
                        >
                          Delete
                        </button>
                      </div>
                    ) : !model.downloadable ? (
                      <p className="model-coming-soon">
                        Planned model: not installable in this prototype build yet.
                      </p>
                    ) : null}
                  </div>
                );
              })}
            </div>

            <div className="decision-buttons">
              <button
                className="button primary"
                onClick={onDownloadModel}
                disabled={
                  modelDownloadPending ||
                  isDownloadingModel ||
                  selectedModelId.length === 0 ||
                  !selectedModelDownloadable
                }
              >
                {modelDownloadPending
                  ? "Starting Download..."
                  : isDownloadingModel
                    ? "Downloading..."
                    : !selectedModelDownloadable
                      ? "Not Downloadable Yet"
                    : `Download ${selectedCatalogModel?.name ?? "Model"}`}
              </button>
              {isDownloadingModel ? (
                <button
                  className="button secondary danger"
                  onClick={() => void onCancelModelDownload()}
                  disabled={modelCancelPending}
                >
                  {modelCancelPending ? "Cancelling..." : "Cancel Download"}
                </button>
              ) : null}
              <button
                className="button secondary"
                onClick={closeModelManagerModal}
                disabled={isDownloadingModel || modelCancelPending}
              >
                {isModelMissing ? "Later" : "Done"}
              </button>
            </div>
          </div>
        </section>
      ) : null}
    </>
  );
}

function formatTimestamp(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  return `${hours.toString().padStart(2, "0")}:${minutes
    .toString()
    .padStart(2, "0")}:${seconds.toString().padStart(2, "0")}`;
}

function formatSource(source: LiveTranscriptLine["source"]): string {
  if (source === "system_audio") {
    return "sys";
  }
  if (source === "mixed") {
    return "mix";
  }
  return "mic";
}

function formatLanguageLabel(language: AsrLanguage): string {
  return language === "japanese" ? "Japanese" : "English";
}

function createMeterSegments(litSegments: number, enabled: boolean, listening: boolean) {
  return Array.from({ length: METER_SEGMENTS }, (_, idx) => {
    const ratio = idx / (METER_SEGMENTS - 1);
    const zone = ratio < 0.66 ? "green" : ratio < 0.88 ? "yellow" : "red";
    const active = enabled && listening && idx < litSegments;
    return { idx, zone, active };
  });
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  if (bytes < 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}
