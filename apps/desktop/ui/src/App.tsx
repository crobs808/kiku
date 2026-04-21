import { useEffect, useMemo, useRef, useState } from "react";
import {
  AsrLanguage,
  AsrProvider,
  IntegrationSettings,
  LanguageConfig,
  LiveTranscriptLine,
  ModelDownloadProgress,
  ModelInventoryItem,
  ModelOption,
  SessionSnapshot,
  SystemAudioPermissionStatus,
  cancelModelDownload,
  deleteModel,
  discardTranscript,
  getAudioLevel,
  getIntegrationSettings,
  getLanguageConfig,
  getModelCatalog,
  getModelDownloadProgress,
  getModelInventory,
  getSessionSnapshot,
  getStreamingTranslationEnabled,
  getSystemAudioPermissionStatus,
  getSourceState,
  openSystemAudioPermissionSettings,
  pollLiveTranscriptLines,
  requestSystemAudioPermissionAccess,
  restartApp,
  saveTranscript,
  setActiveModel,
  setIntegrationSettings as saveIntegrationSettings,
  setLanguageConfig as setAsrLanguageConfig,
  setStreamingTranslationEnabled as setStreamingTranslationMode,
  setMicEnabled,
  setSystemAudioEnabled,
  startModelDownloadById,
  startListening,
  stopListening
} from "./backend";
import { PhraseTestFlyout } from "./PhraseTestFlyout";
import kikuLogoMatte from "../../../../assets/kiku-app-logo-matte.png";

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

const defaultIntegrationSettings: IntegrationSettings = {
  asr_provider: "local",
  translation_provider: "local",
  google_api_key: ""
};

function normalizeIntegrationSettings(
  settings: Partial<IntegrationSettings> | null | undefined
): IntegrationSettings {
  return {
    asr_provider: settings?.asr_provider === "google_cloud" ? "google_cloud" : "local",
    translation_provider:
      settings?.translation_provider === "google_cloud" ? "google_cloud" : "local",
    google_api_key: typeof settings?.google_api_key === "string" ? settings.google_api_key : ""
  };
}

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
  const [stopConfirmOpen, setStopConfirmOpen] = useState(false);
  const [systemPermissionModalOpen, setSystemPermissionModalOpen] = useState(false);
  const [systemPermissionStatus, setSystemPermissionStatus] =
    useState<SystemAudioPermissionStatus>("unsupported");
  const [systemPermissionNeedsRestart, setSystemPermissionNeedsRestart] = useState(false);
  const [permissionActionPending, setPermissionActionPending] = useState<
    null | "request" | "open" | "refresh" | "restart"
  >(null);
  const [savedTranscript, setSavedTranscript] = useState<string>("");
  const [micEnabled, setMicEnabledLocal] = useState(true);
  const [systemEnabled, setSystemEnabledLocal] = useState(false);
  const [audioLevel, setAudioLevel] = useState(0);
  const [streamingTranslationEnabled, setStreamingTranslationEnabledState] = useState(false);
  const [streamingModePending, setStreamingModePending] = useState(false);
  const [integrationModalOpen, setIntegrationModalOpen] = useState(false);
  const [integrationSettingsDraft, setIntegrationSettingsDraft] =
    useState<IntegrationSettings>(defaultIntegrationSettings);
  const [integrationSaving, setIntegrationSaving] = useState(false);
  const [phraseFlyoutOpen, setPhraseFlyoutOpen] = useState(true);
  const [transcriptLines, setTranscriptLines] = useState<string[]>([]);
  const [pendingLineCount, setPendingLineCount] = useState(0);
  const [followLive, setFollowLive] = useState(true);
  const captionPanelRef = useRef<HTMLElement | null>(null);
  const isNearBottomRef = useRef(true);
  const followLiveRef = useRef(true);
  const manualScrollArmedRef = useRef(false);
  const suppressScrollEventsRef = useRef(false);
  const systemPermissionStatusRef = useRef<SystemAudioPermissionStatus>("unsupported");
  const systemPermissionNeedsRestartRef = useRef(false);

  const isListening = snapshot.state === "listening";
  const isAwaitingDecision = snapshot.state === "prompting_save_discard";
  const isStopConfirmVisible = stopConfirmOpen && isListening;
  const isSystemPermissionModalVisible =
    systemPermissionModalOpen && systemPermissionStatus !== "unsupported";
  const isIntegrationModalVisible = integrationModalOpen;
  const isModelMissing = snapshot.state === "model_missing";
  const isDownloadingModel = snapshot.state === "downloading_model" || modelDownload.in_progress;
  const baseModelPromptVisible =
    modelManagerOpen || isDownloadingModel || (isModelMissing && !modelPromptDismissed);
  const isModelPromptVisible = !isSystemPermissionModalVisible && baseModelPromptVisible;
  const isStopping = snapshot.state === "stopping" || sessionActionPending === "stop";
  const installedModels = useMemo(
    () => modelInventory.filter((model) => model.installed),
    [modelInventory]
  );
  const hasEnabledSource = micEnabled || systemEnabled;
  const systemPermissionBlocksListening =
    systemEnabled && (systemPermissionStatus === "denied" || systemPermissionNeedsRestart);
  const hasInstalledModel =
    installedModels.length > 0 ||
    snapshot.state === "ready" ||
    snapshot.state === "listening" ||
    snapshot.state === "stopping" ||
    snapshot.state === "prompting_save_discard" ||
    snapshot.state === "saving_transcript";
  const noModelInstalled = !hasInstalledModel;
  const sessionStatus = getSessionStatus(snapshot.state);
  const controlsLocked =
    isAwaitingDecision ||
    isStopConfirmVisible ||
    isSystemPermissionModalVisible ||
    isIntegrationModalVisible ||
    sessionActionPending !== null;
  const startActionDisabled =
    controlsLocked ||
    isStopping ||
    isModelMissing ||
    isDownloadingModel ||
    systemPermissionBlocksListening ||
    !hasEnabledSource ||
    !hasInstalledModel;
  const canRestartForSystemPermission =
    systemPermissionStatus === "granted" && systemPermissionNeedsRestart;
  const stopActionDisabled = controlsLocked || isStopping;
  const listenButtonDisabled = isListening ? stopActionDisabled : startActionDisabled;
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
  const isModelSwitchDisabled = isDownloadingModel || modelActionPending !== false;
  const isModelDeleteDisabled = isListening || isDownloadingModel || modelActionPending !== false;
  const integrationRequiresKey =
    integrationSettingsDraft.asr_provider === "google_cloud" ||
    integrationSettingsDraft.translation_provider === "google_cloud";
  const integrationApiKey = integrationSettingsDraft.google_api_key ?? "";
  const sourceModeSummary = getListeningSourceSummary(micEnabled, systemEnabled);
  const activeSystemPermissionWarning =
    systemEnabled && systemPermissionStatus === "denied"
      ? "System audio permission is not enabled yet. Use the startup prompt to enable Screen & System Audio Recording for Kiku."
      : systemEnabled && systemPermissionNeedsRestart
        ? "Screen & System Audio Recording was enabled. Restart Kiku to activate system audio capture."
        : null;
  const sourceModeReadyMessage = hasEnabledSource
    ? `${currentModelName} is active. Live ${formatLanguageLabel(languageConfig.source_language)} to ${formatLanguageLabel(languageConfig.target_language)} transcription is ready.`
    : "Enable Mic and/or System to start listening.";

  useEffect(() => {
    if (!isModelMissing && !isDownloadingModel) {
      setModelPromptDismissed(false);
    }
  }, [isModelMissing, isDownloadingModel]);

  useEffect(() => {
    if (!isListening) {
      setStopConfirmOpen(false);
    }
  }, [isListening]);

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
    void refreshIntegrationSettings();
    void refreshStreamingTranslationMode();
    void refreshModelCatalog();
    void refreshModelInventory(true);
    void ensureSystemAudioPermissionReadyOnStartup();
  }, []);

  useEffect(() => {
    if (!isModelPromptVisible) {
      return undefined;
    }

    let inFlight = false;
    const intervalId = window.setInterval(() => {
      if (inFlight) {
        return;
      }
      inFlight = true;
      void Promise.all([
        refreshSnapshot(),
        refreshModelDownloadProgress(),
        refreshModelInventory()
      ]).finally(() => {
        inFlight = false;
      });
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

    let inFlight = false;
    const intervalId = window.setInterval(() => {
      if (inFlight) {
        return;
      }
      inFlight = true;
      void Promise.allSettled([getAudioLevel(), pollLiveTranscriptLines()])
        .then(([levelResult, linesResult]) => {
          if (levelResult.status === "fulfilled") {
            const normalizedLevel = Math.max(0, Math.min(1, levelResult.value));
            setAudioLevel(normalizedLevel);
          } else {
            setError(String(levelResult.reason));
          }

          if (linesResult.status === "fulfilled") {
            appendLiveTranscriptLines(linesResult.value);
          } else {
            setError(String(linesResult.reason));
          }
        })
        .finally(() => {
          inFlight = false;
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

    let inFlight = false;
    const intervalId = window.setInterval(() => {
      if (inFlight) {
        return;
      }
      inFlight = true;
      void refreshSnapshot().finally(() => {
        inFlight = false;
      });
    }, 200);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [snapshot.state]);

  function setSystemPermissionNeedsRestartState(next: boolean): void {
    systemPermissionNeedsRestartRef.current = next;
    setSystemPermissionNeedsRestart(next);
  }

  function applySystemAudioPermissionStatus(next: SystemAudioPermissionStatus): void {
    const previous = systemPermissionStatusRef.current;
    systemPermissionStatusRef.current = next;
    setSystemPermissionStatus(next);

    if (next === "unsupported") {
      setSystemPermissionModalOpen(false);
      setSystemPermissionNeedsRestartState(false);
      return;
    }

    if (next === "denied") {
      setSystemPermissionModalOpen(true);
      return;
    }

    if (previous === "denied" && next === "granted") {
      setSystemPermissionNeedsRestartState(true);
      setSystemPermissionModalOpen(true);
      return;
    }

    if (systemPermissionNeedsRestartRef.current) {
      setSystemPermissionModalOpen(true);
    } else {
      setSystemPermissionModalOpen(false);
    }
  }

  async function refreshSystemAudioPermissionStatus(): Promise<SystemAudioPermissionStatus | null> {
    try {
      const status = await getSystemAudioPermissionStatus();
      applySystemAudioPermissionStatus(status);
      setError(null);
      return status;
    } catch (requestError) {
      setError(String(requestError));
      return null;
    }
  }

  async function ensureSystemAudioPermissionReadyOnStartup(): Promise<void> {
    const status = await refreshSystemAudioPermissionStatus();
    if (status !== "denied") {
      return;
    }

    try {
      const requested = await requestSystemAudioPermissionAccess();
      applySystemAudioPermissionStatus(requested);
    } catch (requestError) {
      setError(String(requestError));
    }
  }

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

  async function refreshIntegrationSettings(): Promise<void> {
    try {
      const settings = await getIntegrationSettings();
      setIntegrationSettingsDraft(normalizeIntegrationSettings(settings));
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
    }
  }

  async function refreshStreamingTranslationMode(): Promise<void> {
    try {
      setStreamingTranslationEnabledState(await getStreamingTranslationEnabled());
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

  async function refreshModelInventory(syncSelectionToActive = false): Promise<void> {
    try {
      const inventory = await getModelInventory();
      setModelInventory(inventory);
      if (syncSelectionToActive) {
        const active = inventory.find((model) => model.active)?.id;
        if (active) {
          setSelectedModelId(active);
        }
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
      setFollowLiveState(true);
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
    if (sessionActionPending !== null || !isListening) {
      return;
    }

    setStopConfirmOpen(true);
  }

  function onKeepListening(): void {
    setStopConfirmOpen(false);
  }

  async function onConfirmStop(nextStep: "discard" | "save"): Promise<void> {
    if (sessionActionPending !== null || !isListening) {
      return;
    }

    setSessionActionPending("stop");
    try {
      setSnapshot(await stopListening());
      setPendingLineCount(0);

      if (nextStep === "discard") {
        setSnapshot(await discardTranscript());
        setSavedTranscript("");
        setTranscriptLines([]);
        setPendingLineCount(0);
      } else {
        const transcript = await saveTranscript();
        setSavedTranscript(transcript);
        await refreshSnapshot();
      }

      setFollowLiveState(true);
      setStopConfirmOpen(false);
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
      setFollowLiveState(true);
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
    if (modelId.length === 0 || modelActionPending !== false || isDownloadingModel) {
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
      const inventory = await deleteModel(modelId);
      setModelInventory(inventory);
      const active = inventory.find((model) => model.active)?.id;
      if (active) {
        setSelectedModelId(active);
      }
      await refreshSnapshot();
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
      await Promise.all([refreshSnapshot(), refreshModelInventory(true)]);
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

  async function onOpenIntegrationModal(): Promise<void> {
    await refreshIntegrationSettings();
    setIntegrationModalOpen(true);
  }

  function onDismissIntegrationModal(): void {
    setIntegrationModalOpen(false);
  }

  async function onSaveIntegrationSettings(): Promise<void> {
    if (integrationSaving) {
      return;
    }

    setIntegrationSaving(true);
    try {
      const normalizedDraft = normalizeIntegrationSettings(integrationSettingsDraft);
      const updated = await saveIntegrationSettings(normalizedDraft);
      setIntegrationSettingsDraft(normalizeIntegrationSettings(updated));
      await refreshSnapshot();
      setError(null);
      setIntegrationModalOpen(false);
    } catch (requestError) {
      setError(String(requestError));
    } finally {
      setIntegrationSaving(false);
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

    const shouldAutoScroll = followLiveRef.current;
    const appendedLineCount = lines.filter((line) => (line.mutation ?? "append") === "append").length;

    setTranscriptLines((existing) => {
      const next = [...existing];
      for (const line of lines) {
        const formatted = `[${formatTimestamp(line.timestamp_ms)}] [${formatSource(line.source)}] ${line.text}`;
        const mutation = line.mutation ?? "append";
        if (mutation === "replace_last" && next.length > 0) {
          next[next.length - 1] = formatted;
        } else {
          next.push(formatted);
        }
      }
      return next.slice(-180);
    });

    if (shouldAutoScroll) {
      window.requestAnimationFrame(() => scrollToLatest());
    } else if (appendedLineCount > 0) {
      setPendingLineCount((count) => count + appendedLineCount);
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
    if (!systemEnabled) {
      const permissionStatus = await refreshSystemAudioPermissionStatus();
      if (permissionStatus === "denied") {
        setSystemPermissionModalOpen(true);
        setError(
          "System audio capture needs Screen & System Audio Recording permission. Enable it in macOS settings, then restart Kiku."
        );
        return;
      }
      if (systemPermissionNeedsRestartRef.current) {
        setSystemPermissionModalOpen(true);
        setError(
          "System audio permission was enabled while Kiku was running. Restart Kiku before starting system audio capture."
        );
        return;
      }
    }

    try {
      const next = await setSystemAudioEnabled(!systemEnabled);
      setMicEnabledLocal(next.mic_enabled);
      setSystemEnabledLocal(next.system_audio_enabled);
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
    }
  }

  async function onToggleStreamingTranslation(): Promise<void> {
    if (streamingModePending) {
      return;
    }

    setStreamingModePending(true);
    try {
      const next = await setStreamingTranslationMode(!streamingTranslationEnabled);
      setStreamingTranslationEnabledState(next);
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
      await refreshStreamingTranslationMode();
    } finally {
      setStreamingModePending(false);
    }
  }

  async function onRequestSystemPermission(): Promise<void> {
    setPermissionActionPending("request");
    try {
      const status = await requestSystemAudioPermissionAccess();
      applySystemAudioPermissionStatus(status);
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
    } finally {
      setPermissionActionPending(null);
    }
  }

  async function onOpenSystemPermissionSettings(): Promise<void> {
    setPermissionActionPending("open");
    try {
      await openSystemAudioPermissionSettings();
      setError(null);
    } catch (requestError) {
      setError(String(requestError));
    } finally {
      setPermissionActionPending(null);
    }
  }

  async function onRefreshSystemPermissionStatus(): Promise<void> {
    setPermissionActionPending("refresh");
    try {
      await refreshSystemAudioPermissionStatus();
    } finally {
      setPermissionActionPending(null);
    }
  }

  async function onRestartForSystemPermission(): Promise<void> {
    setPermissionActionPending("restart");
    try {
      const outcome = await restartApp();
      if (outcome === "manual_required") {
        setError(
          "Automatic restart is unavailable in this development run. Close the Kiku window, then run `pnpm start` again."
        );
        setPermissionActionPending(null);
      }
    } catch (requestError) {
      setError(String(requestError));
      setPermissionActionPending(null);
    }
  }

  function onDismissSystemPermissionModal(): void {
    setSystemPermissionModalOpen(false);
  }

  const micLitSegments = Math.round(audioLevel * METER_SEGMENTS);
  const systemLitSegments = Math.round(audioLevel * METER_SEGMENTS);
  const micMeterSegments = useMemo(
    () => createMeterSegments(micLitSegments, micEnabled, isListening),
    [isListening, micEnabled, micLitSegments]
  );
  const systemMeterSegments = useMemo(
    () => createMeterSegments(systemLitSegments, systemEnabled, isListening),
    [isListening, systemEnabled, systemLitSegments]
  );
  const targetLanguageOptions = useMemo(
    () => ["english", "japanese"] as AsrLanguage[],
    []
  );

  function setFollowLiveState(enabled: boolean): void {
    followLiveRef.current = enabled;
    setFollowLive(enabled);
  }

  function scrollToLatest(): void {
    const panel = captionPanelRef.current;
    if (!panel) {
      return;
    }

    suppressScrollEventsRef.current = true;
    panel.scrollTop = panel.scrollHeight;
    window.requestAnimationFrame(() => {
      suppressScrollEventsRef.current = false;
    });
    isNearBottomRef.current = true;
    setFollowLiveState(true);
    setPendingLineCount(0);
  }

  function handleCaptionScroll(): void {
    if (suppressScrollEventsRef.current) {
      return;
    }

    const panel = captionPanelRef.current;
    if (!panel) {
      return;
    }

    const nearBottom = panel.scrollHeight - panel.scrollTop - panel.clientHeight < 32;
    isNearBottomRef.current = nearBottom;
    if (nearBottom) {
      if (followLiveRef.current && pendingLineCount > 0) {
        setPendingLineCount(0);
      }
      manualScrollArmedRef.current = false;
      return;
    }

    if (manualScrollArmedRef.current) {
      manualScrollArmedRef.current = false;
      if (followLiveRef.current) {
        setFollowLiveState(false);
      }
    }
  }

  function armManualScroll(): void {
    manualScrollArmedRef.current = true;
  }

  useEffect(() => {
    followLiveRef.current = followLive;
  }, [followLive]);

  return (
    <>
      <main
        className={`layout ${isAwaitingDecision || isStopConfirmVisible || isSystemPermissionModalVisible || isIntegrationModalVisible || isModelPromptVisible ? "modal-active" : ""} ${phraseFlyoutOpen ? "phrase-flyout-open" : ""}`}
      >
        <header className="topbar">
          <div className="topbar-left">
            <div className="brand">
              <img src={kikuLogoMatte} className="brand-logo" alt="Kiku logo" draggable={false} />
              <div>
                <p className="title">Kiku</p>
                <p className="subtitle">Local Live Captions</p>
              </div>
            </div>
            <div className="status-inline">
              <span className={`status-chip ${sessionStatus.tone}`}>
                {sessionStatus.label}
              </span>
              <span className={`status-chip ${snapshot.offline_mode_active ? "good" : "muted"}`}>
                {snapshot.offline_mode_active ? "Privacy: Offline Active" : "Privacy: Offline Idle"}
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
                disabled={isModelSwitchDisabled || installedModels.length === 0}
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
              <button
                className="button secondary"
                onClick={() => void onOpenIntegrationModal()}
                disabled={isAwaitingDecision || integrationSaving}
              >
                Cloud
              </button>
              <button
                className={`button toggle ${phraseFlyoutOpen ? "active" : ""}`}
                onClick={() => setPhraseFlyoutOpen((open) => !open)}
                disabled={isAwaitingDecision}
              >
                Phrase Lab
              </button>
            </div>
            <button
              className="button primary"
              onClick={isListening ? onStop : onStart}
              disabled={listenButtonDisabled}
            >
              {isListening
                ? sessionActionPending === "stop" || isStopping
                  ? "Stopping..."
                  : isStopConfirmVisible
                    ? "Confirm Stop..."
                    : "Stop Listening"
                : isModelMissing
                  ? "Model Required"
                  : systemPermissionBlocksListening
                    ? "Permission Required"
                  : isDownloadingModel
                    ? "Downloading Model..."
                    : sessionActionPending === "start"
                      ? "Starting..."
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
            <p>
              {isListening
                ? "Model download in progress in the background. Live captions continue; switch models when download completes."
                : "Model download in progress. Listening will be available once setup completes."}
            </p>
          ) : isModelMissing ? (
            <p>Model setup is required before listening can start. Use the in-app download prompt.</p>
          ) : (
            <p>
              {sourceModeSummary} {activeSystemPermissionWarning ?? sourceModeReadyMessage}
            </p>
          )}
        </section>

        <section className="caption-wrap">
          <section
            className={`caption-panel ${noModelInstalled ? "locked" : ""}`}
            aria-label="caption stream"
            ref={captionPanelRef}
            onScroll={handleCaptionScroll}
            onMouseDown={armManualScroll}
            onWheel={armManualScroll}
            onTouchStart={armManualScroll}
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
          {!followLive ? (
            <button className="new-lines-banner" onClick={scrollToLatest}>
              {pendingLineCount > 0
                ? `Return to live (${pendingLineCount} new)`
                : "Return to live"}
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
              <div className="streaming-toggle-row">
                <p className="streaming-toggle-label">Streaming translation</p>
                <button
                  className={`streaming-toggle-button ${streamingTranslationEnabled ? "active" : ""}`}
                  onClick={() => void onToggleStreamingTranslation()}
                  disabled={streamingModePending}
                  aria-pressed={streamingTranslationEnabled}
                  title="Emit quicker partial captions and auto-correct recent line as context improves."
                >
                  {streamingModePending
                    ? "Updating..."
                    : streamingTranslationEnabled
                      ? "On"
                      : "Off"}
                </button>
              </div>
              <p className="streaming-toggle-note">
                {streamingTranslationEnabled
                  ? "Fast partial output + auto-corrections"
                  : "Sentence-stable output (fewer mid-line changes)"}
              </p>
              <p className="widget-subtitle">{isListening ? "Live input level" : "Waiting for input"}</p>
            </div>
          </div>
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

        <PhraseTestFlyout open={phraseFlyoutOpen} onClose={() => setPhraseFlyoutOpen(false)} />
      </main>

      {isSystemPermissionModalVisible ? (
        <section className="decision-modal-backdrop" role="presentation">
          <div
            className="decision-modal permission-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="system-permission-title"
          >
            <h2 id="system-permission-title">Enable System Audio Permission</h2>
            <p>
              To capture app playback audio on macOS, Kiku needs access under{" "}
              <strong>Privacy &amp; Security &gt; Screen &amp; System Audio Recording</strong>.
            </p>
            {systemPermissionNeedsRestart ? (
              <p className="permission-warning">
                Permission was granted while Kiku was running. Restart Kiku now so system audio
                capture can initialize correctly.
              </p>
            ) : (
              <p className="permission-warning">
                System audio capture is blocked until this permission is enabled.
              </p>
            )}
            <div className="permission-flow">
              <section className="permission-step">
                <p className="permission-step-title">Step 1. Open macOS Settings</p>
                <p className="permission-step-note">
                  Open Privacy &amp; Security. If deep-linking does not jump directly, navigate
                  there manually.
                </p>
                <div className="permission-step-actions">
                  <button
                    className="button secondary"
                    onClick={() => void onOpenSystemPermissionSettings()}
                    disabled={permissionActionPending !== null}
                  >
                    Open Settings
                  </button>
                  {systemPermissionStatus === "denied" ? (
                    <button
                      className="button secondary"
                      onClick={() => void onRequestSystemPermission()}
                      disabled={permissionActionPending !== null}
                    >
                      Show Permission Prompt
                    </button>
                  ) : null}
                </div>
              </section>

              <section className="permission-step">
                <p className="permission-step-title">Step 2. Enable Kiku Access</p>
                <p className="permission-step-note">
                  Turn on Kiku under Screen &amp; System Audio Recording, then return here.
                </p>
                <div className="permission-step-actions">
                  <button
                    className="button secondary"
                    onClick={() => void onRefreshSystemPermissionStatus()}
                    disabled={permissionActionPending !== null}
                  >
                    Check Permission Again
                  </button>
                </div>
              </section>

              <section className="permission-step">
                <p className="permission-step-title">Step 3. Restart Kiku</p>
                <p className="permission-step-note">
                  macOS applies this permission after app relaunch.
                </p>
                <div className="permission-step-actions">
                  <button
                    className="button secondary"
                    onClick={() => void onRestartForSystemPermission()}
                    disabled={!canRestartForSystemPermission || permissionActionPending !== null}
                  >
                    Restart Kiku
                  </button>
                </div>
                {!canRestartForSystemPermission ? (
                  <p className="permission-step-hint">
                    Complete steps 1 and 2 first.
                  </p>
                ) : null}
              </section>
            </div>

            <div className="decision-buttons permission-footer">
              <button
                className="button secondary"
                onClick={onDismissSystemPermissionModal}
                disabled={permissionActionPending === "restart"}
              >
                Not Now
              </button>
            </div>
          </div>
        </section>
      ) : null}

      {isIntegrationModalVisible ? (
        <section className="decision-modal-backdrop" role="presentation">
          <div
            className="decision-modal integration-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="integration-settings-title"
          >
            <h2 id="integration-settings-title">Cloud Provider Settings</h2>
            <p>
              Configure ASR and translation providers and set your Google API key.
            </p>
            <label className="integration-field">
              <span>ASR Provider</span>
              <select
                value={integrationSettingsDraft.asr_provider}
                onChange={(event) =>
                  setIntegrationSettingsDraft((current) => ({
                    ...current,
                    asr_provider: event.currentTarget.value as AsrProvider
                  }))
                }
              >
                <option value="local">Local Whisper</option>
                <option value="google_cloud">Google Cloud Speech-to-Text</option>
              </select>
            </label>
            <label className="integration-field">
              <span>Translation Provider</span>
              <select
                value={integrationSettingsDraft.translation_provider}
                onChange={(event) =>
                  setIntegrationSettingsDraft((current) => ({
                    ...current,
                    translation_provider: event.currentTarget.value as IntegrationSettings["translation_provider"]
                  }))
                }
              >
                <option value="local">Local / Stub</option>
                <option value="google_cloud">Google Cloud Translate</option>
              </select>
            </label>
            <label className="integration-field">
              <span>Google API Key</span>
              <input
                type="password"
                value={integrationApiKey}
                onChange={(event) =>
                  setIntegrationSettingsDraft((current) => ({
                    ...current,
                    google_api_key: event.currentTarget.value
                  }))
                }
                placeholder="AIza..."
                autoComplete="off"
              />
            </label>
            <p className="integration-note">
              Key is currently stored in runtime memory for this dev session. For persistent setup,
              export env vars before `pnpm start`.
            </p>
            <div className="decision-buttons">
              <button
                className="button secondary"
                onClick={onDismissIntegrationModal}
                disabled={integrationSaving}
              >
                Cancel
              </button>
              <button
                className="button primary"
                onClick={() => void onSaveIntegrationSettings()}
                disabled={
                  integrationSaving ||
                  (integrationRequiresKey && integrationApiKey.trim().length === 0)
                }
              >
                {integrationSaving ? "Saving..." : "Save Providers"}
              </button>
            </div>
          </div>
        </section>
      ) : null}

      {isStopConfirmVisible ? (
        <section className="decision-modal-backdrop" role="presentation">
          <div
            className="decision-modal stop-confirm-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="session-stop-confirm-title"
          >
            <h2 id="session-stop-confirm-title">Stop Session?</h2>
            <p>
              Choose what to do with this session now. If this was accidental, keep listening and
              continue live captions.
            </p>
            <div className="decision-buttons">
              <button className="button secondary" onClick={onKeepListening}>
                Oops! Keep Listening
              </button>
              <button
                className="button secondary danger"
                onClick={() => void onConfirmStop("discard")}
                disabled={sessionActionPending === "stop"}
              >
                Stop + Discard
              </button>
              <button
                className="button primary"
                onClick={() => void onConfirmStop("save")}
                disabled={sessionActionPending === "stop"}
              >
                Stop + Save
              </button>
            </div>
          </div>
        </section>
      ) : null}

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
                            disabled={isModelSwitchDisabled}
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
                          disabled={isModelDeleteDisabled}
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

function getSessionStatus(state: SessionSnapshot["state"]): {
  label: string;
  tone: "good" | "warn" | "neutral" | "muted";
} {
  switch (state) {
    case "listening":
      return { label: "Session: Listening", tone: "good" };
    case "ready":
      return { label: "Session: Ready", tone: "neutral" };
    case "downloading_model":
      return { label: "Session: Installing Model", tone: "neutral" };
    case "model_missing":
      return { label: "Session: Model Required", tone: "warn" };
    case "stopping":
      return { label: "Session: Stopping", tone: "warn" };
    case "prompting_save_discard":
      return { label: "Session: Awaiting Save", tone: "warn" };
    case "saving_transcript":
      return { label: "Session: Saving", tone: "neutral" };
    case "error":
      return { label: "Session: Error", tone: "warn" };
    default:
      return { label: "Session: Idle", tone: "muted" };
  }
}

function getListeningSourceSummary(micEnabled: boolean, systemEnabled: boolean): string {
  if (micEnabled && systemEnabled) {
    return "Mic + System mode is active. Kiku will capture both microphone input and macOS playback audio.";
  }
  if (micEnabled) {
    return "Mic mode is active. Kiku will capture speech from your microphone.";
  }
  if (systemEnabled) {
    return "System mode is active. Kiku will capture playback audio directly from macOS.";
  }
  return "No listening source is currently active.";
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
