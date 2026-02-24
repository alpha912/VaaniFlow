// src/App.tsx — Settings window
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  IconMicrophone, IconEye, IconEyeOff, IconLanguage,
  IconWorld, IconKeyboard, IconMicrophone2, IconKey, IconCheck,
  IconShieldLock,
} from "@tabler/icons-react";

const LANGUAGES = [
  { code: "auto", label: "Auto-detect" },
  { code: "hi-IN", label: "Hindi — हिन्दी" },
  { code: "ta-IN", label: "Tamil — தமிழ்" },
  { code: "te-IN", label: "Telugu — తెలుగు" },
  { code: "kn-IN", label: "Kannada — ಕನ್ನಡ" },
  { code: "ml-IN", label: "Malayalam — മലయാളം" },
  { code: "bn-IN", label: "Bengali — বাংলা" },
  { code: "gu-IN", label: "Gujarati — ગુજરાતી" },
  { code: "mr-IN", label: "Marathi — मराठी" },
  { code: "pa-IN", label: "Punjabi — ਪੰਜਾਬੀ" },
  { code: "or-IN", label: "Odia — ଓଡ଼ିଆ" },
  { code: "en-IN", label: "English (Indian accent)" },
];

const TARGET_LANGUAGES = [
  { code: "same", label: "Same as source (no translation)" },
  ...LANGUAGES.filter((l) => l.code !== "auto").map((l) =>
    l.code === "en-IN" ? { code: l.code, label: "English" } : l
  ),
];

interface Config {
  api_key: string;
  source_language: string;
  target_language: string;
  hotkey: string;
  microphone: string;
}

export default function App() {
  const [config, setConfig] = useState<Config>({
    api_key: "",
    source_language: "auto",
    target_language: "same",
    hotkey: "Ctrl+Shift+Space",
    microphone: "default",
  });
  const [saved, setSaved] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<string | null>(null);
  const [showKey, setShowKey] = useState(false);
  const [microphones, setMicrophones] = useState<string[]>([]);

  useEffect(() => {
    invoke<Config>("load_config").then((cfg) => {
      // In dev mode, fall back to the VITE_SARVAM_API_KEY env variable
      if (!cfg.api_key && import.meta.env.VITE_SARVAM_API_KEY) {
        cfg.api_key = import.meta.env.VITE_SARVAM_API_KEY;
      }
      setConfig(cfg);
    }).catch(console.error);

    invoke<string[]>("list_microphones").then(setMicrophones).catch(console.error);
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
    <div style={{ padding: "24px 28px", maxWidth: 560, margin: "0 auto" }}>

      {/* Header */}
      <div style={{ marginBottom: 24, display: "flex", alignItems: "center", gap: 14 }}>
        <div style={{
          width: 44, height: 44, borderRadius: 12,
          background: "rgba(232, 103, 26, 0.12)",
          display: "flex", alignItems: "center", justifyContent: "center",
        }}>
          <IconMicrophone size={24} stroke={1.5} color="#E8671A" />
        </div>
        <div>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <h1 style={{ fontSize: 20, fontWeight: 700, color: "var(--text)" }}>VaaniFlow</h1>
            <span style={{
              fontSize: 11, fontWeight: 500, color: "var(--text-muted)",
              background: "var(--bg2)", border: "1px solid var(--border)",
              borderRadius: 6, padding: "1px 8px",
            }}>v0.2</span>
          </div>
          <p style={{ fontSize: 13, color: "var(--text-muted)", marginTop: 1 }}>
            Speak any language. Flow anywhere.
          </p>
        </div>
      </div>

      {/* API Connection Card */}
      <div className="card">
        <div className="card-title">API Connection</div>
        <div className="field" style={{ marginBottom: 0 }}>
          <label>
            <IconKey size={14} stroke={1.5} />
            Sarvam API Key
          </label>
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
                  background: "none", padding: 4, color: "var(--text-muted)",
                }}
              >
                {showKey ? <IconEyeOff size={16} stroke={1.5} /> : <IconEye size={16} stroke={1.5} />}
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
              marginTop: 8, fontSize: 13, padding: "8px 12px", borderRadius: 8,
              background: testResult.startsWith("Error") ? "rgba(239,68,68,0.1)" : "rgba(34,197,94,0.1)",
              color: testResult.startsWith("Error") ? "var(--error)" : "var(--success)",
            }}>
              {testResult}
            </p>
          )}
          <p style={{ marginTop: 8, fontSize: 12, color: "var(--text-muted)" }}>
            Free key at{" "}
            <a href="https://dashboard.sarvam.ai" target="_blank" rel="noopener" style={{ color: "var(--orange)" }}>
              dashboard.sarvam.ai
            </a>
            {" "}— 1,000 free credits on signup.
          </p>
        </div>
      </div>

      {/* Preferences Card */}
      <div className="card">
        <div className="card-title">Preferences</div>

        {/* Languages row */}
        <div className="row">
          <div className="field">
            <label>
              <IconLanguage size={14} stroke={1.5} />
              Source Language
            </label>
            <select
              value={config.source_language}
              onChange={(e) => setConfig({ ...config, source_language: e.target.value })}
              title="Source Language"
            >
              {LANGUAGES.map((l) => (
                <option key={l.code} value={l.code}>{l.label}</option>
              ))}
            </select>
          </div>
          <div className="field">
            <label>
              <IconWorld size={14} stroke={1.5} />
              Target Language
            </label>
            <select
              value={config.target_language}
              onChange={(e) => setConfig({ ...config, target_language: e.target.value })}
              title="Target Language"
            >
              {TARGET_LANGUAGES.map((l) => (
                <option key={l.code} value={l.code}>{l.label}</option>
              ))}
            </select>
          </div>
        </div>

        {/* Hotkey + Mic row */}
        <div className="row">
          <div className="field" style={{ marginBottom: 0 }}>
            <label>
              <IconKeyboard size={14} stroke={1.5} />
              Global Hotkey
            </label>
            <input
              type="text"
              value={config.hotkey}
              onChange={(e) => setConfig({ ...config, hotkey: e.target.value })}
              placeholder="Ctrl+Shift+Space"
            />
          </div>
          <div className="field" style={{ marginBottom: 0 }}>
            <label>
              <IconMicrophone2 size={14} stroke={1.5} />
              Microphone
            </label>
            <select
              value={config.microphone}
              onChange={(e) => setConfig({ ...config, microphone: e.target.value })}
              title="Microphone"
            >
              <option value="default">System Default</option>
              {microphones.map((name) => (
                <option key={name} value={name}>{name}</option>
              ))}
            </select>
          </div>
        </div>
      </div>

      {/* Save */}
      <button
        className="btn-primary"
        onClick={handleSave}
        style={{ width: "100%", padding: "12px", fontSize: 15, fontWeight: 600 }}
      >
        {saved
          ? <><IconCheck size={18} stroke={2.5} style={{ verticalAlign: "middle", marginRight: 6 }} />Saved!</>
          : "Save Settings"}
      </button>

      {/* Footer */}
      <div style={{
        marginTop: 16, display: "flex", alignItems: "center", justifyContent: "center",
        gap: 6, fontSize: 11, color: "var(--text-muted)",
      }}>
        <IconShieldLock size={13} stroke={1.5} />
        Your API key never leaves this device.
      </div>
    </div>
  );
}
