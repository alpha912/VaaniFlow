// src/BubbleApp.tsx — VaaniFlow floating bubble with canvas waveform
import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { IconMicrophone, IconCheck, IconX, IconSettings, IconPower } from "@tabler/icons-react";

type State = "idle" | "ready" | "recording" | "processing" | "done" | "error";

// ─── Canvas waveform ──────────────────────────────────────────────────────────

function Waveform({ active }: { active: boolean }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const animRef = useRef<number>(0);
  const bufferRef = useRef<number[]>(Array(40).fill(0));
  const phaseRef = useRef(0);

  const draw = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const W = canvas.width;
    const H = canvas.height;
    ctx.clearRect(0, 0, W, H);

    phaseRef.current += 0.08;
    const phase = phaseRef.current;

    // Push a new amplitude value built from layered sine waves + noise
    const buf = bufferRef.current;
    buf.shift();
    const amp =
      0.30 * Math.sin(phase * 1.3) +
      0.25 * Math.sin(phase * 2.7 + 1.1) +
      0.20 * Math.sin(phase * 4.1 + 2.3) +
      0.15 * Math.sin(phase * 0.7 + 0.5) +
      0.10 * (Math.random() - 0.5);
    buf.push(Math.max(-1, Math.min(1, amp)));

    const midY = H / 2;
    const barW = W / buf.length;

    for (let i = 0; i < buf.length; i++) {
      const a = Math.abs(buf[i]);
      const barH = Math.max(2, a * (H / 2) * 0.88);
      const x = i * barW;

      // Brightness peaks at centre of the waveform
      const brightness = 0.4 + 0.6 * Math.sin((i / buf.length) * Math.PI);
      const alpha = 0.55 + 0.45 * brightness;

      ctx.fillStyle = `rgba(255,255,255,${alpha.toFixed(2)})`;

      // Top half
      ctx.beginPath();
      ctx.roundRect(x + 1, midY - barH, Math.max(1, barW - 2), barH, 1.5);
      ctx.fill();

      // Mirrored bottom half (lower opacity)
      ctx.fillStyle = `rgba(255,255,255,${(alpha * 0.5).toFixed(2)})`;
      ctx.beginPath();
      ctx.roundRect(x + 1, midY, Math.max(1, barW - 2), barH, 1.5);
      ctx.fill();
    }

    animRef.current = requestAnimationFrame(draw);
  }, []);

  useEffect(() => {
    if (active) {
      animRef.current = requestAnimationFrame(draw);
    } else {
      cancelAnimationFrame(animRef.current);
      const canvas = canvasRef.current;
      if (canvas) {
        canvas.getContext("2d")?.clearRect(0, 0, canvas.width, canvas.height);
      }
      bufferRef.current = Array(40).fill(0);
      phaseRef.current = 0;
    }
    return () => cancelAnimationFrame(animRef.current);
  }, [active, draw]);

  return (
    <canvas
      ref={canvasRef}
      width={112}
      height={40}
      style={{ display: "block" }}
    />
  );
}

// ─── Spinner for processing state ─────────────────────────────────────────────

function Spinner() {
  return (
    <svg width={22} height={22} viewBox="0 0 24 24">
      <circle cx={12} cy={12} r={9} fill="none" stroke="rgba(255,255,255,0.2)" strokeWidth={2} />
      <path d="M12 3 A9 9 0 0 1 21 12" fill="none" stroke="white" strokeWidth={2} strokeLinecap="round">
        <animateTransform attributeName="transform" type="rotate" from="0 12 12" to="360 12 12" dur="0.75s" repeatCount="indefinite" />
      </path>
    </svg>
  );
}

// ─── Main bubble app ──────────────────────────────────────────────────────────

export default function BubbleApp() {
  const [state, setState] = useState<State>("idle");
  const [transcript, setTranscript] = useState("");
  const [errorMsg, setErrorMsg] = useState("");
  const [ctxMenu, setCtxMenu] = useState<{ x: number; y: number } | null>(null);
  const stateRef = useRef<State>("idle");
  const resetTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Update both state and ref synchronously so hotkey listener
  // always sees the latest value (useEffect would defer the ref update)
  const setStateSync = useCallback((newState: State | ((prev: State) => State)) => {
    setState((prev) => {
      const next = typeof newState === "function" ? newState(prev) : newState;
      stateRef.current = next;
      return next;
    });
  }, []);

  useEffect(() => {
    const uns = [
      listen("text-field-focused",  () => setStateSync((s) => s === "idle" ? "ready" : s)),
      listen("text-field-blurred",  () => setStateSync((s) => s === "ready" ? "idle" : s)),
      listen("recording-started",   () => setStateSync("recording")),
      listen("recording-stopped",   () => setStateSync("processing")),
      listen("hotkey-triggered",    () => {
        const s = stateRef.current;
        if (s === "idle" || s === "ready") {
          invoke("start_recording").catch(console.error);
          setStateSync("recording");
        } else if (s === "recording") {
          invoke("stop_recording").catch(console.error);
          setStateSync("processing");
        }
      }),
      listen<string>("transcription-done",  (e) => {
        if (resetTimerRef.current) clearTimeout(resetTimerRef.current);
        setTranscript(e.payload);
        setStateSync("done");
        resetTimerRef.current = setTimeout(() => {
          setStateSync("idle");
          setTranscript("");
          resetTimerRef.current = null;
        }, 2200);
      }),
      listen<string>("transcription-error", (e) => {
        if (resetTimerRef.current) clearTimeout(resetTimerRef.current);
        setErrorMsg(e.payload);
        setStateSync("error");
        resetTimerRef.current = setTimeout(() => {
          setStateSync("idle");
          setErrorMsg("");
          resetTimerRef.current = null;
        }, 3200);
      }),
    ];
    return () => {
      uns.forEach((p) => p.then((f) => f()));
      if (resetTimerRef.current) clearTimeout(resetTimerRef.current);
    };
  }, [setStateSync]);

  function handleClick() {
    if (state === "idle" || state === "ready") {
      invoke("start_recording").catch(console.error);
      setStateSync("recording");
    } else if (state === "recording") {
      invoke("stop_recording").catch(console.error);
      setStateSync("processing");
    }
  }

  function handleRightClick(e: React.MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    setCtxMenu({ x: e.clientX, y: e.clientY });
  }

  function handleCtxAction(action: "settings" | "quit") {
    setCtxMenu(null);
    if (action === "settings") {
      invoke("open_settings").catch(console.error);
    } else {
      invoke("quit_app").catch(console.error);
    }
  }

  // Dismiss context menu on click-away
  useEffect(() => {
    if (!ctxMenu) return;
    const dismiss = () => setCtxMenu(null);
    window.addEventListener("click", dismiss);
    return () => window.removeEventListener("click", dismiss);
  }, [ctxMenu]);

  // Bubble expands into a pill when recording to show the waveform
  const isRecording = state === "recording";
  const W = isRecording ? 152 : 56;
  const H = 56;
  const R = H / 2;

  const theme = {
    idle:       { bg: "rgba(18,24,36,0.92)",   border: "rgba(255,255,255,0.15)", shadow: "0 4px 20px rgba(0,0,0,0.6)",        opacity: 0.90 },
    ready:      { bg: "rgba(15,76,92,0.93)",   border: "rgba(26,122,138,0.70)", shadow: "0 4px 28px rgba(26,122,138,0.45)",  opacity: 1    },
    recording:  { bg: "rgba(196,68,12,0.95)",  border: "rgba(232,103,26,0.85)", shadow: "0 0 36px rgba(232,103,26,0.70)",    opacity: 1    },
    processing: { bg: "rgba(26,122,138,0.93)", border: "rgba(26,122,138,0.60)", shadow: "0 4px 24px rgba(26,122,138,0.4)",   opacity: 1    },
    done:       { bg: "rgba(21,128,61,0.93)",  border: "rgba(34,197,94,0.60)",  shadow: "0 4px 24px rgba(34,197,94,0.45)",  opacity: 1    },
    error:      { bg: "rgba(185,28,28,0.93)",  border: "rgba(239,68,68,0.60)",  shadow: "0 4px 24px rgba(239,68,68,0.45)",  opacity: 1    },
  }[state];

  const tooltip =
    state === "idle"       ? "Click a text field to start" :
    state === "ready"      ? "Click to record" :
    state === "recording"  ? "Click to stop" :
    state === "processing" ? "Transcribing…" :
    state === "done"       ? (transcript ? `"${transcript.slice(0, 32)}${transcript.length > 32 ? "…" : ""}"` : "Done!") :
    state === "error"      ? errorMsg.slice(0, 48) :
    null;

  return (
    <div style={{
      width: "100%", height: "100%",
      display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "flex-end",
      background: "transparent", position: "relative",
      paddingBottom: 30,
    }}>

      {/* Double pulse rings when recording */}
      {isRecording && [0, 0.6].map((delay, i) => (
        <div key={i} style={{
          position: "absolute",
          width: W, height: H, borderRadius: R,
          border: "2px solid rgba(232,103,26,0.45)",
          animation: `pulse-ring 1.6s ease-out ${delay}s infinite`,
          pointerEvents: "none",
        }} />
      ))}

      {/* Main bubble */}
      <div
        onClick={handleClick}
        onContextMenu={handleRightClick}
        data-tauri-drag-region
        style={{
          width: W, height: H, borderRadius: R,
          background: theme.bg,
          border: `1.5px solid ${theme.border}`,
          boxShadow: theme.shadow,
          opacity: theme.opacity,
          cursor: state === "processing" ? "wait" : state === "done" || state === "error" ? "default" : "pointer",
          display: "flex", alignItems: "center", justifyContent: "center",
          overflow: "hidden",
          backdropFilter: "blur(16px)",
          WebkitBackdropFilter: "blur(16px)",
          userSelect: "none",
          transition: "width 0.28s cubic-bezier(0.4,0,0.2,1), background 0.18s, box-shadow 0.18s, opacity 0.18s",
          animation: isRecording ? "recording-glow 1.8s ease-in-out infinite" : "none",
        }}
      >
        {/* Recording: stop-square icon + waveform canvas */}
        {isRecording && (
          <div style={{ display: "flex", alignItems: "center", gap: 10, padding: "0 12px" }}>
            <div style={{ width: 14, height: 14, borderRadius: 3, background: "white", flexShrink: 0 }} />
            <Waveform active={true} />
          </div>
        )}

        {state === "processing" && <Spinner />}
        {state === "done"       && <IconCheck size={26} stroke={2} color="white" />}
        {state === "error"      && <IconX size={22} stroke={2} color="white" />}
        {(state === "idle" || state === "ready") && <IconMicrophone size={26} stroke={1.5} color="white" />}
      </div>

      {/* Tooltip below bubble */}
      {tooltip && !ctxMenu && (
        <div style={{
          position: "absolute", bottom: 0,
          left: "50%", transform: "translateX(-50%)",
          background: "rgba(0,0,0,0.80)",
          color: "rgba(255,255,255,0.92)",
          fontSize: 11, fontFamily: "-apple-system, BlinkMacSystemFont, sans-serif",
          padding: "3px 10px", borderRadius: 20,
          whiteSpace: "nowrap", maxWidth: 240,
          overflow: "hidden", textOverflow: "ellipsis",
          backdropFilter: "blur(8px)", pointerEvents: "none",
        }}>
          {tooltip}
        </div>
      )}

      {/* Custom right-click context menu */}
      {ctxMenu && (
        <div
          onClick={(e) => e.stopPropagation()}
          style={{
            position: "absolute",
            bottom: 96,
            left: "50%",
            transform: "translateX(-50%)",
            background: "rgba(18, 24, 36, 0.96)",
            border: "1px solid rgba(255,255,255,0.12)",
            borderRadius: 10,
            padding: "4px 0",
            minWidth: 164,
            boxShadow: "0 8px 32px rgba(0,0,0,0.6), 0 0 0 1px rgba(255,255,255,0.05)",
            backdropFilter: "blur(20px)",
            WebkitBackdropFilter: "blur(20px)",
            zIndex: 9999,
            animation: "pop-in 0.15s ease-out",
          }}
        >
          <button
            onClick={() => handleCtxAction("settings")}
            style={{
              display: "flex", alignItems: "center", gap: 10,
              padding: "8px 14px", margin: "0 4px",
              background: "none", border: "none",
              color: "rgba(255,255,255,0.88)",
              fontSize: 13, fontFamily: "inherit", cursor: "pointer",
              borderRadius: 6, width: "calc(100% - 8px)",
            }}
            onMouseEnter={(e) => (e.currentTarget.style.background = "rgba(255,255,255,0.08)")}
            onMouseLeave={(e) => (e.currentTarget.style.background = "none")}
          >
            <IconSettings size={15} stroke={1.5} /> Settings
          </button>
          <div style={{ height: 1, background: "rgba(255,255,255,0.08)", margin: "4px 12px" }} />
          <button
            onClick={() => handleCtxAction("quit")}
            style={{
              display: "flex", alignItems: "center", gap: 10,
              padding: "8px 14px", margin: "0 4px",
              background: "none", border: "none",
              color: "rgba(239,68,68,0.9)",
              fontSize: 13, fontFamily: "inherit", cursor: "pointer",
              borderRadius: 6, width: "calc(100% - 8px)",
            }}
            onMouseEnter={(e) => (e.currentTarget.style.background = "rgba(239,68,68,0.08)")}
            onMouseLeave={(e) => (e.currentTarget.style.background = "none")}
          >
            <IconPower size={15} stroke={1.5} /> Quit VaaniFlow
          </button>
        </div>
      )}
    </div>
  );
}
