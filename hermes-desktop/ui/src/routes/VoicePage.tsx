import { useEffect, useMemo, useState } from 'react';
import {
  voiceListHistory,
  voiceListProviders,
  voiceProcessSpeakQueue,
  voiceSetEnabled,
  voiceSpeak,
  voiceStatus,
  voiceTranscribe,
  voiceUpdateSettings,
  type VoiceHistoryItem,
  type VoiceProvider,
  type VoiceSettings,
} from '../lib/tauri';
import './VoicePage.css';

const emptySettings: VoiceSettings = {
  enabled: false,
  stt_provider: 'local-text-capture',
  tts_provider: 'local-speak-queue',
  updated_at: '',
  transcription_language: 'zh-CN',
  preferred_voice: null,
  auto_speak_transcripts: false,
};

export function VoicePage() {
  const [settings, setSettings] = useState<VoiceSettings>(emptySettings);
  const [providers, setProviders] = useState<VoiceProvider[]>([]);
  const [history, setHistory] = useState<VoiceHistoryItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [actionLoading, setActionLoading] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [transcriptDraft, setTranscriptDraft] = useState('');
  const [speakDraft, setSpeakDraft] = useState('');

  useEffect(() => {
    void loadVoice();
  }, []);

  const queue = useMemo(
    () => history.filter((item) => item.kind === 'speech' && item.status === 'queued'),
    [history],
  );
  const transcripts = useMemo(
    () => history.filter((item) => item.kind === 'transcription'),
    [history],
  );
  const sttProviders = useMemo(
    () => providers.filter((provider) => provider.kind === 'stt'),
    [providers],
  );
  const ttsProviders = useMemo(
    () => providers.filter((provider) => provider.kind === 'tts'),
    [providers],
  );

  async function loadVoice() {
    setLoading(true);
    setError(null);
    try {
      const [voiceSettings, voiceHistory] = await Promise.all([
        voiceStatus(),
        voiceListHistory({ limit: 20, include_payload: true }),
      ]);
      const voiceProviders = await voiceListProviders();
      setSettings(voiceSettings);
      setProviders(voiceProviders);
      setHistory(voiceHistory.items);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }

  async function handleToggleEnabled() {
    setSaving(true);
    setError(null);
    try {
      const updated = await voiceSetEnabled({ enabled: !settings.enabled });
      setSettings(updated);
      setNotice(updated.enabled ? 'Voice workflow 已启用。' : 'Voice workflow 已停用。');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  }

  async function handleSaveSettings() {
    setSaving(true);
    setError(null);
    try {
      const updated = await voiceUpdateSettings({
        stt_provider: settings.stt_provider,
        tts_provider: settings.tts_provider,
        transcription_language: settings.transcription_language,
        preferred_voice: settings.preferred_voice,
        auto_speak_transcripts: settings.auto_speak_transcripts,
      });
      setSettings(updated);
      setNotice('Voice settings 已保存。');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  }

  async function handleTranscribe() {
    setActionLoading('transcribe');
    setError(null);
    try {
      await voiceTranscribe({
        text: transcriptDraft,
        source: 'manual',
      });
      setTranscriptDraft('');
      await loadVoice();
      setNotice('已记录本地 transcript。');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setActionLoading(null);
    }
  }

  async function handleSpeak() {
    setActionLoading('speak');
    setError(null);
    try {
      await voiceSpeak({
        text: speakDraft,
        voice: settings.preferred_voice,
        origin: 'assistant',
      });
      setSpeakDraft('');
      await loadVoice();
      setNotice('已加入本地 speak queue。');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setActionLoading(null);
    }
  }

  async function handleProcessQueue() {
    setActionLoading('process');
    setError(null);
    try {
      const result = await voiceProcessSpeakQueue({ mark_status: 'spoken' });
      await loadVoice();
      setNotice(
        result.processed
          ? `已处理队列项：${result.item.payload_text ?? result.item.id}`
          : '当前没有待处理的 speak queue。',
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setActionLoading(null);
    }
  }

  return (
    <div className="voice-page">
      <h2>Voice</h2>
      <div className="voice-banner voice-banner-boundary">
        Voice 当前是本地 text-only 闭环：Local Text Capture 记录手动输入文本，Local Speak Queue
        持久化待播报文本；不会采集麦克风，也不会合成音频。
      </div>
      {error ? <div className="voice-banner voice-banner-error">{error}</div> : null}
      {notice ? <div className="voice-banner">{notice}</div> : null}
      {loading ? <div className="voice-card">加载中...</div> : null}
      {!loading ? (
        <div className="voice-grid">
          <section className="voice-card">
            <div className="voice-card-header">
              <div>
                <h3>Workflow</h3>
                <p>本地 transcript / speak queue 已持久化到桌面数据库。</p>
              </div>
              <button className="voice-button" type="button" onClick={handleToggleEnabled} disabled={saving}>
                {settings.enabled ? 'Disable' : 'Enable'}
              </button>
            </div>
            <div className="voice-field-grid">
              <label>
                <span>STT provider</span>
                <select
                  value={settings.stt_provider}
                  onChange={(event) => setSettings({ ...settings, stt_provider: event.target.value })}
                >
                  {sttProviders.map((provider) => (
                    <option key={provider.id} value={provider.id}>{provider.label}</option>
                  ))}
                </select>
              </label>
              <label>
                <span>TTS provider</span>
                <select
                  value={settings.tts_provider}
                  onChange={(event) => setSettings({ ...settings, tts_provider: event.target.value })}
                >
                  {ttsProviders.map((provider) => (
                    <option key={provider.id} value={provider.id}>{provider.label}</option>
                  ))}
                </select>
              </label>
              <label>
                <span>Language</span>
                <input
                  value={settings.transcription_language}
                  onChange={(event) =>
                    setSettings({ ...settings, transcription_language: event.target.value })
                  }
                />
              </label>
              <label>
                <span>Preferred voice</span>
                <input
                  value={settings.preferred_voice ?? ''}
                  onChange={(event) =>
                    setSettings({
                      ...settings,
                      preferred_voice: event.target.value.trim() || null,
                    })
                  }
                />
              </label>
            </div>
            <div className="voice-provider-list">
              {providers.map((provider) => (
                <div className="voice-provider-card" key={provider.id}>
                  <div className="voice-history-title-row">
                    <strong>{provider.label}</strong>
                    <span>{provider.kind.toUpperCase()}</span>
                  </div>
                  <span>{provider.runtime_boundary}</span>
                  <span>Transport: {provider.transport} · Interaction: {provider.interaction_model}</span>
                  <span>Audio input: {provider.supports_audio_input ? 'yes' : 'no'} · Audio output: {provider.supports_audio_output ? 'yes' : 'no'}</span>
                </div>
              ))}
            </div>
            <label className="voice-checkbox">
              <input
                type="checkbox"
                checked={settings.auto_speak_transcripts}
                onChange={(event) =>
                  setSettings({ ...settings, auto_speak_transcripts: event.target.checked })
                }
              />
              <span>Auto queue transcript for speech</span>
            </label>
            <div className="voice-actions">
              <button className="voice-button voice-button-primary" type="button" onClick={handleSaveSettings} disabled={saving}>
                {saving ? '保存中...' : '保存设置'}
              </button>
              <button className="voice-button" type="button" onClick={() => void loadVoice()}>
                刷新
              </button>
            </div>
          </section>

          <section className="voice-card">
            <h3>Local Transcript</h3>
            <textarea
              value={transcriptDraft}
              onChange={(event) => setTranscriptDraft(event.target.value)}
              placeholder="Paste or type transcript text here"
            />
            <div className="voice-actions">
              <button
                className="voice-button voice-button-primary"
                type="button"
                onClick={handleTranscribe}
                disabled={actionLoading !== null || transcriptDraft.trim().length === 0}
              >
                {actionLoading === 'transcribe' ? '记录中...' : '记录 transcript'}
              </button>
            </div>
            <div className="voice-stats">
              <div>
                <strong>{transcripts.length}</strong>
                <span>transcripts</span>
              </div>
              <div>
                <strong>{queue.length}</strong>
                <span>queued speech</span>
              </div>
            </div>
          </section>

          <section className="voice-card">
            <h3>Speak Queue</h3>
            <textarea
              value={speakDraft}
              onChange={(event) => setSpeakDraft(event.target.value)}
              placeholder="Text to enqueue for local speech output"
            />
            <div className="voice-actions">
              <button
                className="voice-button voice-button-primary"
                type="button"
                onClick={handleSpeak}
                disabled={actionLoading !== null || speakDraft.trim().length === 0}
              >
                {actionLoading === 'speak' ? '排队中...' : '加入 speak queue'}
              </button>
              <button
                className="voice-button"
                type="button"
                onClick={handleProcessQueue}
                disabled={actionLoading !== null}
              >
                {actionLoading === 'process' ? '处理中...' : '处理下一个'}
              </button>
            </div>
            <div className="voice-queue-list">
              {queue.length === 0 ? <div className="voice-empty">当前没有待播报项。</div> : null}
              {queue.map((item) => (
                <div className="voice-history-item" key={item.id}>
                  <strong>{item.payload_text ?? item.id}</strong>
                  <span>{item.provider} · {item.status}</span>
                </div>
              ))}
            </div>
          </section>

          <section className="voice-card voice-card-wide">
            <h3>Recent Activity</h3>
            <div className="voice-history-list">
              {history.length === 0 ? <div className="voice-empty">还没有 voice history。</div> : null}
              {history.map((item) => (
                <div className="voice-history-item" key={item.id}>
                  <div className="voice-history-title-row">
                    <strong>{item.kind}</strong>
                    <span>{item.status}</span>
                  </div>
                  <span>{item.provider}</span>
                  <span>{item.payload_text ?? '-'}</span>
                  <span>{item.created_at}</span>
                </div>
              ))}
            </div>
          </section>
        </div>
      ) : null}
    </div>
  );
}
