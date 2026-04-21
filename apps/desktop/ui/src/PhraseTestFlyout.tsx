import { useEffect, useMemo, useRef, useState } from "react";
import { JAPANESE_PHRASEBOOK, JapanesePhrase } from "./japanesePhrasebook";

const PHRASEBOOK_AUDIO_HOST = "https://nemoapps.com";
const PHRASEBOOK_SOURCE = "https://nemoapps.com/phrasebooks/japanese";

type PhraseTestFlyoutProps = {
  open: boolean;
  onClose: () => void;
};

function getPhraseAudioUrl(audioPath: string): string {
  if (audioPath.startsWith("http://") || audioPath.startsWith("https://")) {
    return audioPath;
  }

  return `${PHRASEBOOK_AUDIO_HOST}${audioPath}`;
}

export function PhraseTestFlyout({ open, onClose }: PhraseTestFlyoutProps) {
  const [playingPhraseId, setPlayingPhraseId] = useState<string | null>(null);
  const [playbackError, setPlaybackError] = useState<string | null>(null);
  const [searchTerm, setSearchTerm] = useState("");
  const audioRef = useRef<HTMLAudioElement | null>(null);

  const filteredPhrases = useMemo(() => {
    const normalizedTerm = searchTerm.trim().toLowerCase();
    if (normalizedTerm.length === 0) {
      return JAPANESE_PHRASEBOOK;
    }

    return JAPANESE_PHRASEBOOK.filter((phrase) => {
      const japanese = phrase.japanese.toLowerCase();
      const romaji = phrase.romaji.toLowerCase();
      const english = phrase.english.toLowerCase();

      return (
        japanese.includes(normalizedTerm) ||
        romaji.includes(normalizedTerm) ||
        english.includes(normalizedTerm)
      );
    });
  }, [searchTerm]);

  useEffect(() => {
    if (open) {
      return;
    }

    const audio = audioRef.current;
    if (!audio) {
      return;
    }

    audio.pause();
    audio.currentTime = 0;
    setPlayingPhraseId(null);
  }, [open]);

  useEffect(() => {
    return () => {
      const audio = audioRef.current;
      if (!audio) {
        return;
      }

      audio.pause();
      audio.removeAttribute("src");
      audio.load();
    };
  }, []);

  async function onPlayPhrase(phrase: JapanesePhrase): Promise<void> {
    const audio = audioRef.current;
    if (!audio) {
      return;
    }

    setPlaybackError(null);

    if (playingPhraseId === phrase.id && !audio.paused) {
      audio.pause();
      audio.currentTime = 0;
      setPlayingPhraseId(null);
      return;
    }

    const nextSource = getPhraseAudioUrl(phrase.audioPath);
    if (audio.getAttribute("src") !== nextSource) {
      audio.setAttribute("src", nextSource);
      audio.load();
    }

    audio.currentTime = 0;

    try {
      await audio.play();
      setPlayingPhraseId(phrase.id);
    } catch {
      setPlayingPhraseId(null);
      setPlaybackError(
        `Could not play \"${phrase.english}\". Check your network connection and try again.`
      );
    }
  }

  function onAudioEnded(): void {
    setPlayingPhraseId(null);
  }

  function onAudioError(): void {
    setPlayingPhraseId(null);
    setPlaybackError("Audio failed to load for this phrase.");
  }

  return (
    <aside
      className={`phrase-flyout ${open ? "open" : ""}`}
      aria-hidden={!open}
      aria-label="Japanese phrase playback tester"
    >
      <div className="phrase-flyout-header">
        <div>
          <p className="phrase-flyout-title">Japanese Phrase Lab</p>
          <p className="phrase-flyout-subtitle">Play audio and verify live transcript accuracy.</p>
        </div>
        <button className="button secondary phrase-flyout-close" onClick={onClose}>
          Close
        </button>
      </div>

      <p className="phrase-source-note">
        Source: {PHRASEBOOK_SOURCE}
      </p>

      <label className="phrase-search-field">
        <span>Filter</span>
        <input
          type="search"
          value={searchTerm}
          placeholder="Japanese, romaji, or English"
          onChange={(event) => setSearchTerm(event.currentTarget.value)}
        />
      </label>

      <p className="phrase-count">
        {filteredPhrases.length} / {JAPANESE_PHRASEBOOK.length} phrases
      </p>

      <div className="phrase-list" role="list" aria-label="Japanese phrase list">
        {filteredPhrases.length > 0 ? (
          filteredPhrases.map((phrase) => {
            const isPlaying = playingPhraseId === phrase.id;

            return (
              <div
                key={phrase.id}
                className={`phrase-item ${isPlaying ? "playing" : ""}`}
                role="listitem"
              >
                <button
                  className={`phrase-play-button ${isPlaying ? "playing" : ""}`}
                  onClick={() => void onPlayPhrase(phrase)}
                  aria-label={`${isPlaying ? "Stop" : "Play"} ${phrase.english}`}
                >
                  {isPlaying ? "Stop" : "Play"}
                </button>
                <div className="phrase-text">
                  <p className="phrase-japanese" lang="ja">
                    {phrase.japanese}
                  </p>
                  <p className="phrase-romaji">{phrase.romaji}</p>
                  <p className="phrase-english">{phrase.english}</p>
                </div>
              </div>
            );
          })
        ) : (
          <p className="phrase-empty">No phrase matches this filter.</p>
        )}
      </div>

      {playbackError ? <p className="phrase-error">{playbackError}</p> : null}

      <audio ref={audioRef} preload="none" onEnded={onAudioEnded} onError={onAudioError} />
    </aside>
  );
}
