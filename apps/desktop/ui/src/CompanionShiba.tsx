import { useEffect, useMemo, useRef, useState } from "react";

type CompanionMode =
  | "sleeping"
  | "idle"
  | "watch-transcript"
  | "listen-user"
  | "system-monitor"
  | "active"
  | "fetch";

type CompanionShibaProps = {
  isListening: boolean;
  audioLevel: number;
  micEnabled: boolean;
  systemEnabled: boolean;
  transcriptLineCount: number;
};

const WATCH_TRANSCRIPT_MS = 2600;
const FETCH_ROUTINE_MS = 7400;
const ROUTINE_COOLDOWN_MS = 84000;
const QUIET_FOR_SLEEP_MS = 17000;

export function CompanionShiba({
  isListening,
  audioLevel,
  micEnabled,
  systemEnabled,
  transcriptLineCount
}: CompanionShibaProps) {
  const [mode, setMode] = useState<CompanionMode>("sleeping");
  const [frame, setFrame] = useState(0);
  const lastActivityMsRef = useRef(Date.now());
  const watchTranscriptUntilMsRef = useRef(0);
  const fetchUntilMsRef = useRef(0);
  const lastFetchMsRef = useRef(0);
  const lastTranscriptLineCountRef = useRef(transcriptLineCount);

  useEffect(() => {
    if (!isListening) {
      setMode("sleeping");
      lastActivityMsRef.current = Date.now();
      watchTranscriptUntilMsRef.current = 0;
      fetchUntilMsRef.current = 0;
      return;
    }

    if (audioLevel >= 0.014) {
      lastActivityMsRef.current = Date.now();
    }
  }, [audioLevel, isListening]);

  useEffect(() => {
    if (transcriptLineCount > lastTranscriptLineCountRef.current) {
      const now = Date.now();
      lastActivityMsRef.current = now;
      watchTranscriptUntilMsRef.current = now + WATCH_TRANSCRIPT_MS;
    }

    lastTranscriptLineCountRef.current = transcriptLineCount;
  }, [transcriptLineCount]);

  useEffect(() => {
    if (!isListening) {
      setFrame(0);
      return undefined;
    }

    const frameIntervalId = window.setInterval(() => {
      setFrame((current) => (current + 1) % 3);
    }, 320);

    return () => {
      window.clearInterval(frameIntervalId);
    };
  }, [isListening]);

  useEffect(() => {
    const timerId = window.setInterval(() => {
      const now = Date.now();

      if (!isListening) {
        setMode("sleeping");
        return;
      }

      if (now < fetchUntilMsRef.current) {
        setMode("fetch");
        return;
      }

      if (now < watchTranscriptUntilMsRef.current) {
        setMode("watch-transcript");
        return;
      }

      const quietMs = now - lastActivityMsRef.current;
      const canStartFetch =
        quietMs > 11000 &&
        now - lastFetchMsRef.current > ROUTINE_COOLDOWN_MS &&
        micEnabled;

      if (canStartFetch) {
        fetchUntilMsRef.current = now + FETCH_ROUTINE_MS;
        lastFetchMsRef.current = now;
        setMode("fetch");
        return;
      }

      if (systemEnabled && !micEnabled) {
        if (quietMs >= QUIET_FOR_SLEEP_MS) {
          setMode("sleeping");
          return;
        }

        setMode("system-monitor");
        return;
      }

      if (micEnabled && audioLevel >= 0.11) {
        setMode("listen-user");
        return;
      }

      if ((micEnabled && audioLevel >= 0.035) || (systemEnabled && audioLevel >= 0.028)) {
        setMode("active");
        return;
      }

      if (quietMs >= QUIET_FOR_SLEEP_MS) {
        setMode("sleeping");
        return;
      }

      setMode("idle");
    }, 260);

    return () => {
      window.clearInterval(timerId);
    };
  }, [audioLevel, isListening, micEnabled, systemEnabled]);

  const modeLabel = useMemo(() => {
    switch (mode) {
      case "sleeping":
        return "Napping";
      case "watch-transcript":
        return "Watching transcript";
      case "listen-user":
        return "Listening to mic";
      case "system-monitor":
        return "Monitoring system audio";
      case "active":
        return "Listening";
      case "fetch":
        return "Fetch routine";
      default:
        return "Standing by";
    }
  }, [mode]);

  const eyesClosed = mode === "sleeping";
  const gazeShiftX = useMemo(() => {
    if (mode === "watch-transcript" || mode === "system-monitor") {
      return -1;
    }
    if (mode === "listen-user" || mode === "active") {
      return 1;
    }
    return 0;
  }, [mode]);
  const earsPerked = mode === "listen-user" || mode === "active";
  const tailWagging = mode === "active" || mode === "listen-user" || mode === "watch-transcript";
  const showToy = mode === "fetch";

  return (
    <aside
      className={`companion-widget pixel mode-${mode} frame-${frame}`}
      aria-label="Shiba companion"
    >
      <div className="companion-scene pixel-scene">
        <svg
          viewBox="0 0 160 100"
          role="img"
          aria-label="Retro pixel Shiba companion"
          shapeRendering="crispEdges"
        >
          <rect x="0" y="0" width="160" height="100" fill="#0f2432" />
          <rect x="0" y="78" width="160" height="22" fill="#112b3c" />

          <g opacity="0.95">
            <rect x="98" y="74" width="48" height="16" fill="#3a5c78" />
            <rect x="102" y="70" width="40" height="6" fill="#5d7f9f" />
            <rect x="106" y="72" width="32" height="4" fill="#86abc5" />
          </g>

          {showToy ? (
            <g className="pixel-toy">
              <rect x="86" y="84" width="6" height="6" fill="#e95f3f" />
              <rect x="88" y="86" width="2" height="2" fill="#ffd9c0" />
            </g>
          ) : null}

          <g
            className={`pixel-shiba ${earsPerked ? "perk-ears" : ""} ${tailWagging ? "wag-tail" : ""}`}
            transform="translate(20 16)"
          >
            <g className="pixel-tail">
              <rect x="56" y="24" width="24" height="14" fill="#19111f" />
              <rect x="80" y="20" width="12" height="6" fill="#19111f" />
              <rect x="48" y="30" width="14" height="10" fill="#19111f" />
              <rect x="58" y="22" width="24" height="14" fill="#f27f3f" />
              <rect x="82" y="20" width="8" height="8" fill="#f4a15f" />
              <rect x="66" y="24" width="14" height="10" fill="#ede8d7" />
            </g>

            <rect x="20" y="28" width="44" height="26" fill="#19111f" />
            <rect x="24" y="30" width="38" height="22" fill="#f49a53" />
            <rect x="24" y="40" width="22" height="20" fill="#ede8d7" />

            <rect x="4" y="12" width="28" height="26" fill="#19111f" />
            <rect x="6" y="14" width="24" height="22" fill="#f49a53" />

            <g className="pixel-ears">
              <rect x="8" y={earsPerked ? 3 : 5} width="8" height="10" fill="#19111f" />
              <rect x="10" y={earsPerked ? 5 : 7} width="4" height="6" fill="#f27f3f" />
              <rect x="22" y={earsPerked ? 2 : 4} width="8" height="10" fill="#19111f" />
              <rect x="24" y={earsPerked ? 4 : 6} width="4" height="6" fill="#f27f3f" />
            </g>

            <rect x="8" y="24" width="18" height="12" fill="#ede8d7" />
            <rect x="12" y="26" width="10" height="8" fill="#f0ebde" />
            <rect x="13" y="30" width="8" height="3" fill="#19111f" />

            {eyesClosed ? (
              <g>
                <rect x="12" y="20" width="5" height="2" fill="#19111f" />
                <rect x="21" y="20" width="5" height="2" fill="#19111f" />
              </g>
            ) : (
              <g>
                <rect x="11" y="19" width="6" height="5" fill="#19111f" />
                <rect x="20" y="19" width="6" height="5" fill="#19111f" />
                <rect x={13 + gazeShiftX} y="20" width="2" height="2" fill="#f7f5ed" />
                <rect x={22 + gazeShiftX} y="20" width="2" height="2" fill="#f7f5ed" />
              </g>
            )}

            <rect x="31" y="52" width="10" height="18" fill="#19111f" />
            <rect x="33" y="54" width="6" height="14" fill="#a8aec7" />
            <rect x="52" y="52" width="12" height="18" fill="#19111f" />
            <rect x="54" y="54" width="8" height="14" fill="#a8aec7" />
            <rect x="20" y="52" width="10" height="18" fill="#19111f" />
            <rect x="22" y="54" width="6" height="14" fill="#a8aec7" />

            <rect x="31" y="68" width="9" height="3" fill="#19111f" />
            <rect x="53" y="68" width="10" height="3" fill="#19111f" />
            <rect x="22" y="68" width="8" height="3" fill="#19111f" />
          </g>
        </svg>
      </div>
      <p className="companion-label">Shiba companion: {modeLabel}</p>
    </aside>
  );
}
