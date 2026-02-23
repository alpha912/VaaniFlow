// src/App.tsx — Settings window
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

const LANGUAGES = [
  { code: "auto", label: "Auto-detect" },
  { code: "hi-IN", label: "Hindi — हिन्दी" },
  { code: "ta-IN", label: "Tamil — தமிழ்" },
  { code: "te-IN", label: "Telugu — తెలుగు" },
  { code: "kn-IN", label: "Kannada — ಕನ್ನಡ" },
  { code: "ml-IN", label: "Malayalam — മലയാളം" },
  { code: "bn-IN", label: "Bengali — বাংলা" },
  { code: "gu-IN", label: "Gujarati — ગુજરાતી" },
  { code: "mr-IN", label: "Marathi — मराठी" },
  { code: "pa-IN", label: "Punjabi — ਪੰਜਾਬੀ" },
  { code: "or-IN", label: "Odia — ଓଡ଼ିଆ" },
  { code: "en-IN", label: "English (Indian accent)" },
];

const TARGET_LANGUAGES = [
  { code: "same", label: "Same as source (no translation)" },
  ...LANGUAGES.filter((l) => l.code !== "auto"),
];

interface Config {
  api_key: string;
  source_language: string;
  target_language: string;
  hotkey: string;
}

export default function App() {
  const [config, setConfig] = useState<Config>({
    api_key: "",
    source_language: "auto",
    target_language: "same",
    hotkey: "ctrl+shift+space",
  });
  const [saved, setSaved] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<string | null>(null);
  const [showKey, setShowKey] = useState(false);

  useEffect(() => {
    invoke<Config>("load_config").then((cfg) => {
      // In dev mode, fall back to the VITE_SARVAM_API_KEY env variable
      if (!cfg.api_key && import.meta.env.VITE_SARVAM_API_KEY) {
        cfg.api_key = import.meta.env.VITE_SARVAM_API_KEY;
      }
      setConfig(cfg);
    }).catch(console.error);
  }, []);

  async function handleSave() {
    try {
      await invoke("save_config", { config });
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      console.error(e);
    }
  }

  async function handleTestKey() {
    if (!config.api_key) return;
    setTesting(true);
    setTestResult(null);
    try {
      const result = await invoke<string>("test_api_key", { apiKey: config.api_key });
      setTestResult(result);
    } catch (e: any) {
      setTestResult("Error: " + String(e));
    } finally {
      setTesting(false);
    }
  }

  return (
    <div style={{ padding: "28px 32px", maxWidth: 560, margin: "0 auto" }}>
      {/* Header */}
      <div style={{ marginBottom: 32, display: "flex", alignItems: "center", gap: 12 }}>
        <div style={{ fontSize: 28 }}>🎙️</div>
        <div>
          <h1 style={{ fontSize: 22, fontWeight: 700, color: "#E8671A" }}>वाणी VaaniFlow</h1>
          <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 2 }}>
            Speak any language. Flow anywhere.
          </p>
        </div>
      </div>

      {/* API Key */}
      <div className="field">
        <label>Sarvam API Key</label>
        <div style={{ display: "flex", gap: 8 }}>
          <div style={{ position: "relative", flex: 1 }}>
            <input
              type={showKey ? "text" : "password"}
              value={config.api_key}
              onChange={(e) => setConfig({ ...config, api_key: e.target.value })}
              placeholder="Enter your Sarvam API key..."
              style={{ paddingRight: 40 }}
            />
            <button
              onClick={() => setShowKey(!showKey)}
              style={{
                position: "absolute", right: 8, top: "50%", transform: "translateY(-50%)",
                background: "none", padding: 4, color: "var(--text-muted)", fontSize: 16,
              }}
            >
              {showKey ? "🙈" : "👁️"}
            </button>
          </div>
          <button
            className="btn-ghost"
            onClick={handleTestKey}
            disabled={testing || !config.api_key}
            style={{ whiteSpace: "nowrap" }}
          >
            {testing ? "Testing…" : "Test Key"}
          </button>
        </div>
        {testResult && (
          <p style={{
            marginTop: 6, fontSize: 12, padding: "6px 10px", borderRadius: 6,
            background: testResult.startsWith("Error") ? "rgba(239,68,68,0.1)" : "rgba(34,197,94,0.1)",
            color: testResult.startsWith("Error") ? "var(--error)" : "var(--success)",
          }}>
            {testResult}
          </p>
        )}
        <p style={{ marginTop: 6, fontSize: 12, color: "var(--text-muted)" }}>
          Get a free key at{" "}
          <a href="https://dashboard.sarvam.ai" target="_blank" rel="noopener" style={{ color: "var(--orange)" }}>
            dashboard.sarvam.ai
          </a>{" "}
          — ₹1,000 free credits on signup (~33 hours of speech).
        </p>
      </div>

      {/* Source Language */}
      <div className="field">
        <label>Source Language</label>
        <select
          value={config.source_language}
          onChange={(e) => setConfig({ ...config, source_language: e.target.value })}
        >
          {LANGUAGES.map((l) => (
            <option key={l.code} value={l.code}>{l.label}</option>
          ))}
        </select>
        <p style={{ marginTop: 6, fontSize: 12, color: "var(--text-muted)" }}>
          "Auto-detect" works for all 22 Indian languages including code-mixed speech (Hinglish, Tanglish, etc.)
        </p>
      </div>

      {/* Target Language */}
      <div className="field">
        <label>Destination Language (Translation)</label>
        <select
          value={config.target_language}
          onChange={(e) => setConfig({ ...config, target_language: e.target.value })}
        >
          {TARGET_LANGUAGES.map((l) => (
            <option key={l.code} value={l.code}>{l.label}</option>
          ))}
        </select>
        <p style={{ marginTop: 6, fontSize: 12, color: "var(--text-muted)" }}>
          When set, your speech is transcribed first then translated. Powered by Sarvam Mayura v1.
        </p>
      </div>

      {/* Hotkey */}
      <div className="field">
        <label>Global Hotkey</label>
        <input
          type="text"
          value={config.hotkey}
          onChange={(e) => setConfig({ ...config, hotkey: e.target.value })}
          placeholder="e.g. ctrl+shift+space"
        />
        <p style={{ marginTop: 6, fontSize: 12, color: "var(--text-muted)" }}>
          Press anywhere to toggle recording. Format: ctrl+shift+space, cmd+shift+v, etc.
        </p>
      </div>

      {/* How it works */}
      <div style={{
        background: "rgba(15, 76, 92, 0.2)",
        border: "1px solid rgba(26, 122, 138, 0.3)",
        borderRadius: 10, padding: "14px 16px", marginBottom: 24,
      }}>
        <p style={{ fontSize: 12, color: "var(--text-muted)", lineHeight: 1.7 }}>
          <strong style={{ color: "var(--teal-mid)" }}>How it works:</strong>{" "}
          Click any text field in any app. The bubble floats near it. Click the bubble or press the hotkey.
          Speak. Stop. Your transcribed (and optionally translated) text is typed into the field automatically.
        </p>
      </div>

      {/* Save */}
      <button
        className="btn-primary"
        onClick={handleSave}
        style={{ width: "100%", padding: "12px" }}
      >
        {saved ? "✓ Saved!" : "Save Settings"}
      </button>

      <p style={{ marginTop: 16, fontSize: 11, color: "var(--text-muted)", textAlign: "center" }}>
        MIT Licensed · Your API key never leaves your machine · VaaniFlow × Sarvam AI
      </p>
    </div>
  );
}
