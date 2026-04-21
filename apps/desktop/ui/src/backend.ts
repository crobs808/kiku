import { invoke } from "@tauri-apps/api/core";

export type SessionState =
  | "idle"
  | "model_missing"
  | "downloading_model"
  | "ready"
  | "listening"
  | "stopping"
  | "prompting_save_discard"
  | "saving_transcript"
  | "error";

export type SessionSnapshot = {
  state: SessionState;
  offline_mode_active: boolean;
  transcript_line_count: number;
  last_error: string | null;
};

export type SourceState = {
  mic_enabled: boolean;
  system_audio_enabled: boolean;
};

export type LiveTranscriptLine = {
  timestamp_ms: number;
  source: "mic" | "system_audio" | "mixed";
  text: string;
  mutation?: "append" | "replace_last";
};

export type ModelDownloadProgress = {
  in_progress: boolean;
  progress: number;
  downloaded_bytes: number;
  total_bytes: number | null;
  installed: boolean;
  model_id: string | null;
  model_name: string | null;
  last_error: string | null;
};

export type AsrLanguage = "english" | "japanese";

export type LanguageConfig = {
  source_language: AsrLanguage;
  target_language: AsrLanguage;
};

export type SystemAudioPermissionStatus = "granted" | "denied" | "unsupported";
export type AppRestartOutcome = "restarting" | "manual_required";
export type AsrProvider = "local" | "google_cloud";
export type TranslationProvider = "local" | "google_cloud";

export type IntegrationSettings = {
  asr_provider: AsrProvider;
  translation_provider: TranslationProvider;
  google_api_key: string;
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

export type ModelOption = {
  id: string;
  name: string;
  family: string;
  size: string;
  approx_wer: string;
  latency: string;
  language_focus: string;
  best_for: string;
  notes: string;
  availability: "available_now" | "planned";
  downloadable: boolean;
  recommended: boolean;
};

export type ModelInventoryItem = ModelOption & {
  installed: boolean;
  active: boolean;
};

const fallback: {
  snapshot: SessionSnapshot;
  sourceState: SourceState;
  audioLevel: number;
  modelDownload: ModelDownloadProgress;
  languageConfig: LanguageConfig;
  streamingTranslationEnabled: boolean;
  integrationSettings: IntegrationSettings;
  modelCatalog: ModelOption[];
} = {
  snapshot: {
    state: "ready",
    offline_mode_active: false,
    transcript_line_count: 0,
    last_error: null
  },
  sourceState: {
    mic_enabled: true,
    system_audio_enabled: false
  },
  audioLevel: 0,
  modelDownload: {
    in_progress: false,
    progress: 0,
    downloaded_bytes: 0,
    total_bytes: null,
    installed: true,
    model_id: "large-v3",
    model_name: "Whisper Large v3",
    last_error: null
  },
  languageConfig: {
    source_language: "japanese",
    target_language: "english"
  },
  streamingTranslationEnabled: false,
  integrationSettings: {
    asr_provider: "local",
    translation_provider: "local",
    google_api_key: ""
  },
  modelCatalog: [
    {
      id: "large-v3",
      name: "Whisper Large v3",
      family: "Whisper.cpp",
      size: "3.1 GB",
      approx_wer: "~5.0% (best)",
      latency: "Higher latency, highest quality",
      language_focus: "Japanese + multilingual",
      best_for: "Best Japanese->English live meeting accuracy",
      notes: "Default recommendation when quality matters most.",
      availability: "available_now",
      downloadable: true,
      recommended: true
    },
    {
      id: "large-v3-turbo",
      name: "Whisper Large v3 Turbo",
      family: "Whisper.cpp",
      size: "1.6 GB",
      approx_wer: "~5.6%",
      latency: "Fast high-quality preset",
      language_focus: "Multilingual with better speed",
      best_for: "Lower-latency meetings while keeping high translation quality",
      notes: "Recommended when Large v3 quality is needed with faster response.",
      availability: "available_now",
      downloadable: true,
      recommended: false
    },
    {
      id: "medium",
      name: "Whisper Medium",
      family: "Whisper.cpp",
      size: "1.5 GB",
      approx_wer: "~6.1%",
      latency: "Balanced quality vs speed",
      language_focus: "Japanese + English",
      best_for: "Balanced option for long meetings on most Macs",
      notes: "Good fallback when Large v3 feels too heavy.",
      availability: "available_now",
      downloadable: true,
      recommended: false
    },
    {
      id: "distil-large-v3",
      name: "Distil-Whisper Large v3",
      family: "Distil-Whisper (GGML)",
      size: "1.5 GB",
      approx_wer: "~5.8% (English benchmark)",
      latency: "Very fast large-model class",
      language_focus: "English-heavy meetings / mixed content",
      best_for: "Fast long meetings where low latency is critical",
      notes: "Runs through Whisper-compatible runtime with strong speed gains.",
      availability: "available_now",
      downloadable: true,
      recommended: false
    },
    {
      id: "small",
      name: "Whisper Small",
      family: "Whisper.cpp",
      size: "466 MB",
      approx_wer: "~7.6%",
      latency: "Faster startup and inference",
      language_focus: "English-centric / mixed Japanese",
      best_for: "Speed-first testing and lighter hardware sessions",
      notes: "Useful when responsiveness matters more than top accuracy.",
      availability: "available_now",
      downloadable: true,
      recommended: false
    },
    {
      id: "base",
      name: "Whisper Base",
      family: "Whisper.cpp",
      size: "142 MB",
      approx_wer: "~10.2%",
      latency: "Fastest / lowest quality",
      language_focus: "Quick setup and demos",
      best_for: "Smoke tests and low-resource prototypes",
      notes: "Expect noticeably weaker Japanese translation accuracy.",
      availability: "available_now",
      downloadable: true,
      recommended: false
    },
    {
      id: "kotoba-whisper-v2",
      name: "Kotoba-Whisper v2.0",
      family: "Kotoba / Whisper family",
      size: "3.1 GB",
      approx_wer: "~5.0% (JP-focused estimate)",
      latency: "Medium-high",
      language_focus: "Japanese-focused ASR",
      best_for: "Japanese-heavy meetings where Whisper variants miss terms",
      notes: "GGML Whisper-compatible Japanese-specialized model.",
      availability: "available_now",
      downloadable: true,
      recommended: false
    },
    {
      id: "reazonspeech-nemo",
      name: "ReazonSpeech NeMo (Candidate)",
      family: "NVIDIA NeMo",
      size: "~2-3 GB (varies by checkpoint)",
      approx_wer: "N/A in current build",
      latency: "Expected medium",
      language_focus: "Japanese ASR",
      best_for: "Future lower-latency Japanese transcription path",
      notes: "Planned non-Whisper backend option for Japanese meetings.",
      availability: "planned",
      downloadable: false,
      recommended: false
    },
    {
      id: "parakeet-ja",
      name: "Parakeet-TDT JA (Candidate)",
      family: "NVIDIA Parakeet",
      size: "~2-3 GB (estimated runtime package)",
      approx_wer: "N/A in current build",
      latency: "Expected medium-fast",
      language_focus: "Japanese ASR",
      best_for: "Future real-time Japanese captions with good throughput",
      notes: "Planned evaluation target for speed/quality balance.",
      availability: "planned",
      downloadable: false,
      recommended: false
    },
    {
      id: "seamless-m4t-v2",
      name: "SeamlessM4T v2 (Candidate)",
      family: "Meta Seamless",
      size: "~5+ GB",
      approx_wer: "Uses non-WER benchmark metrics",
      latency: "High compute",
      language_focus: "Multilingual speech translation",
      best_for: "Future multi-language expansion beyond EN/JA",
      notes: "Planned, with licensing/product review required before shipping.",
      availability: "planned",
      downloadable: false,
      recommended: false
    }
  ]
};

function inTauriRuntime(): boolean {
  const globalWindow = window as Window & {
    __TAURI__?: unknown;
    __TAURI_INTERNALS__?: unknown;
  };

  return Boolean(globalWindow.__TAURI__ || globalWindow.__TAURI_INTERNALS__);
}

export async function getSessionSnapshot(): Promise<SessionSnapshot> {
  if (!inTauriRuntime()) {
    return fallback.snapshot;
  }

  return invoke<SessionSnapshot>("get_session_snapshot");
}

export async function startListening(): Promise<SessionSnapshot> {
  if (!inTauriRuntime()) {
    fallback.snapshot = {
      ...fallback.snapshot,
      state: "listening",
      offline_mode_active: true
    };
    return fallback.snapshot;
  }

  return invoke<SessionSnapshot>("start_listening");
}

export async function stopListening(): Promise<SessionSnapshot> {
  if (!inTauriRuntime()) {
    fallback.snapshot = {
      ...fallback.snapshot,
      state: "prompting_save_discard",
      offline_mode_active: false
    };
    return fallback.snapshot;
  }

  return invoke<SessionSnapshot>("stop_listening");
}

export async function discardTranscript(): Promise<SessionSnapshot> {
  if (!inTauriRuntime()) {
    fallback.snapshot = {
      ...fallback.snapshot,
      state: "ready",
      transcript_line_count: 0
    };
    return fallback.snapshot;
  }

  return invoke<SessionSnapshot>("discard_transcript");
}

export async function saveTranscript(): Promise<string> {
  if (!inTauriRuntime()) {
    fallback.snapshot = {
      ...fallback.snapshot,
      state: "ready",
      transcript_line_count: 0
    };

    return "[00:00:03] [mic] Placeholder saved transcript line.";
  }

  return invoke<string>("save_transcript");
}

export async function appendTranscriptLine(timestampMs: number, text: string): Promise<void> {
  if (!inTauriRuntime()) {
    fallback.snapshot = {
      ...fallback.snapshot,
      transcript_line_count: fallback.snapshot.transcript_line_count + 1
    };
    return;
  }

  await invoke("append_transcript_line", { timestampMs, text });
}

export async function getSourceState(): Promise<SourceState> {
  if (!inTauriRuntime()) {
    return fallback.sourceState;
  }

  return invoke<SourceState>("get_source_state");
}

export async function setMicEnabled(enabled: boolean): Promise<SourceState> {
  if (!inTauriRuntime()) {
    fallback.sourceState = {
      ...fallback.sourceState,
      mic_enabled: enabled
    };
    return fallback.sourceState;
  }

  return invoke<SourceState>("set_mic_enabled", { enabled });
}

export async function setSystemAudioEnabled(enabled: boolean): Promise<SourceState> {
  if (!inTauriRuntime()) {
    fallback.sourceState = {
      ...fallback.sourceState,
      system_audio_enabled: enabled
    };
    return fallback.sourceState;
  }

  return invoke<SourceState>("set_system_audio_enabled", { enabled });
}

export async function getSystemAudioPermissionStatus(): Promise<SystemAudioPermissionStatus> {
  if (!inTauriRuntime()) {
    return "granted";
  }

  return invoke<SystemAudioPermissionStatus>("get_system_audio_permission_status");
}

export async function requestSystemAudioPermissionAccess(): Promise<SystemAudioPermissionStatus> {
  if (!inTauriRuntime()) {
    return "granted";
  }

  return invoke<SystemAudioPermissionStatus>("request_system_audio_permission_access");
}

export async function openSystemAudioPermissionSettings(): Promise<void> {
  if (!inTauriRuntime()) {
    return;
  }

  await invoke("open_system_audio_permission_settings");
}

export async function restartApp(): Promise<AppRestartOutcome> {
  if (!inTauriRuntime()) {
    return "manual_required";
  }

  return invoke<AppRestartOutcome>("restart_app");
}

export async function getLanguageConfig(): Promise<LanguageConfig> {
  if (!inTauriRuntime()) {
    return fallback.languageConfig;
  }

  return invoke<LanguageConfig>("get_language_config");
}

export async function setLanguageConfig(
  sourceLanguage: AsrLanguage,
  targetLanguage: AsrLanguage
): Promise<LanguageConfig> {
  if (!inTauriRuntime()) {
    fallback.languageConfig = {
      source_language: sourceLanguage,
      target_language: targetLanguage
    };
    return fallback.languageConfig;
  }

  return invoke<LanguageConfig>("set_language_config", {
    sourceLanguage,
    targetLanguage
  });
}

export async function getIntegrationSettings(): Promise<IntegrationSettings> {
  if (!inTauriRuntime()) {
    return normalizeIntegrationSettings(fallback.integrationSettings);
  }

  return normalizeIntegrationSettings(
    await invoke<IntegrationSettings>("get_integration_settings")
  );
}

export async function setIntegrationSettings(
  settings: IntegrationSettings
): Promise<IntegrationSettings> {
  const normalized = normalizeIntegrationSettings(settings);
  if (!inTauriRuntime()) {
    fallback.integrationSettings = normalized;
    return normalizeIntegrationSettings(fallback.integrationSettings);
  }

  return normalizeIntegrationSettings(
    await invoke<IntegrationSettings>("set_integration_settings", { settings: normalized })
  );
}

export async function getStreamingTranslationEnabled(): Promise<boolean> {
  if (!inTauriRuntime()) {
    return fallback.streamingTranslationEnabled;
  }

  return invoke<boolean>("get_streaming_translation_enabled");
}

export async function setStreamingTranslationEnabled(enabled: boolean): Promise<boolean> {
  if (!inTauriRuntime()) {
    fallback.streamingTranslationEnabled = enabled;
    return fallback.streamingTranslationEnabled;
  }

  return invoke<boolean>("set_streaming_translation_enabled", { enabled });
}

export async function getModelCatalog(): Promise<ModelOption[]> {
  if (!inTauriRuntime()) {
    return fallback.modelCatalog;
  }

  return invoke<ModelOption[]>("get_model_catalog");
}

export async function getModelInventory(): Promise<ModelInventoryItem[]> {
  if (!inTauriRuntime()) {
    return fallback.modelCatalog.map((model, idx) => ({
      ...model,
      installed: model.downloadable && idx === 0,
      active: model.downloadable && idx === 0
    }));
  }

  return invoke<ModelInventoryItem[]>("get_model_inventory");
}

export async function getAudioLevel(): Promise<number> {
  if (!inTauriRuntime()) {
    return fallback.audioLevel;
  }

  return invoke<number>("get_audio_level");
}

export async function pollLiveTranscriptLines(): Promise<LiveTranscriptLine[]> {
  if (!inTauriRuntime()) {
    return [];
  }

  return invoke<LiveTranscriptLine[]>("poll_live_transcript_lines");
}

export async function getModelDownloadProgress(): Promise<ModelDownloadProgress> {
  if (!inTauriRuntime()) {
    return fallback.modelDownload;
  }

  return invoke<ModelDownloadProgress>("get_model_download_progress");
}

export async function startModelDownload(): Promise<ModelDownloadProgress> {
  if (!inTauriRuntime()) {
    fallback.modelDownload = {
      ...fallback.modelDownload,
      in_progress: false,
      progress: 1,
      installed: true,
      model_id: "large-v3",
      model_name: "Whisper Large v3",
      last_error: null
    };
    fallback.snapshot = {
      ...fallback.snapshot,
      state: "ready"
    };
    return fallback.modelDownload;
  }

  return invoke<ModelDownloadProgress>("start_model_download");
}

export async function startModelDownloadById(modelId: string): Promise<ModelDownloadProgress> {
  if (!inTauriRuntime()) {
    const selected = fallback.modelCatalog.find((model) => model.id === modelId);
    if (!selected?.downloadable) {
      fallback.modelDownload = {
        ...fallback.modelDownload,
        in_progress: false,
        progress: 0,
        downloaded_bytes: 0,
        total_bytes: null,
        last_error: `${selected?.name ?? modelId} is planned and not downloadable in this build.`
      };
      return fallback.modelDownload;
    }
    fallback.modelDownload = {
      ...fallback.modelDownload,
      in_progress: false,
      progress: 1,
      installed: true,
      model_id: selected?.id ?? modelId,
      model_name: selected?.name ?? modelId,
      last_error: null
    };
    return fallback.modelDownload;
  }

  return invoke<ModelDownloadProgress>("start_model_download", { modelId });
}

export async function cancelModelDownload(): Promise<ModelDownloadProgress> {
  if (!inTauriRuntime()) {
    fallback.modelDownload = {
      ...fallback.modelDownload,
      in_progress: false,
      progress: 0,
      downloaded_bytes: 0,
      total_bytes: null,
      last_error: null
    };
    return fallback.modelDownload;
  }

  return invoke<ModelDownloadProgress>("cancel_model_download");
}

export async function setActiveModel(modelId: string): Promise<ModelInventoryItem[]> {
  if (!inTauriRuntime()) {
    return fallback.modelCatalog.map((model) => ({
      ...model,
      installed: model.downloadable && model.id === "large-v3",
      active: model.downloadable && model.id === modelId
    }));
  }

  return invoke<ModelInventoryItem[]>("set_active_model", { modelId });
}

export async function deleteModel(modelId: string): Promise<ModelInventoryItem[]> {
  if (!inTauriRuntime()) {
    return fallback.modelCatalog.map((model, idx) => ({
      ...model,
      installed: model.downloadable && model.id !== modelId && idx === 0,
      active: model.downloadable && model.id !== modelId && idx === 0
    }));
  }

  return invoke<ModelInventoryItem[]>("delete_model", { modelId });
}
