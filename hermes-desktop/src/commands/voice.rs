//! Voice workflow commands and persistent local voice state.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::backend::{AppError, Database};

const VOICE_SETTINGS_KEY: &str = "voice_settings";
const LEGACY_STUB_PROVIDER: &str = "stub-local";
const LOCAL_STT_PROVIDER: &str = "local-text-capture";
const LOCAL_TTS_PROVIDER: &str = "local-speak-queue";
const DEFAULT_TRANSCRIPTION_LANGUAGE: &str = "en-US";
const DEFAULT_HISTORY_LIMIT: u32 = 20;
const MAX_HISTORY_LIMIT: u32 = 200;
const VOICE_STATUS_COMPLETED: &str = "completed";
const VOICE_STATUS_QUEUED: &str = "queued";
const VOICE_STATUS_SPOKEN: &str = "spoken";
const DEFAULT_TRANSCRIPTION_SOURCE: &str = "manual";
const DEFAULT_SPEAK_ORIGIN: &str = "assistant";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VoiceProviderKind {
    Stt,
    Tts,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VoiceProvider {
    pub id: String,
    pub label: String,
    pub kind: VoiceProviderKind,
    pub local_only: bool,
    pub transport: String,
    pub interaction_model: String,
    pub supports_audio_input: bool,
    pub supports_audio_output: bool,
    pub capabilities: Vec<String>,
    pub compatibility_aliases: Vec<String>,
    pub runtime_boundary: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VoiceSummary {
    pub enabled: bool,
    pub stt_provider: String,
    pub tts_provider: String,
    pub updated_at: String,
    pub transcription_count: u32,
    pub queued_speak_count: u32,
    pub pending_speak_count: u32,
    pub history_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_transcript: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_spoken_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_event_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_event_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct VoiceSettings {
    pub enabled: bool,
    pub stt_provider: String,
    pub tts_provider: String,
    pub updated_at: String,
    pub transcription_language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_voice: Option<String>,
    pub auto_speak_transcripts: bool,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        default_voice_settings()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VoiceSetEnabledRequest {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VoiceUpdateSettingsRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stt_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tts_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcription_language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_voice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_speak_transcripts: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VoiceTranscribeStubRequest {
    pub transcript: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VoiceTranscribeRequest {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_queue_for_speech: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VoiceTranscriptionResult {
    pub transcript: String,
    pub provider: String,
    pub normalized_transcript: String,
    pub source: String,
    pub language: String,
    pub word_count: u32,
    pub queued_for_speech: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VoiceSpeakStubRequest {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VoiceSpeakRequest {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VoiceSpeakResult {
    pub queued: bool,
    pub provider: String,
    pub text: String,
    pub id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
    pub origin: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VoiceHistoryListRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default)]
    pub include_payload: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VoiceHistoryItem {
    pub id: String,
    pub kind: String,
    pub provider: String,
    pub status: String,
    pub text: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VoiceHistoryListResult {
    pub total: u32,
    pub items: Vec<VoiceHistoryItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VoiceProcessSpeakQueueRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mark_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VoiceProcessSpeakQueueResult {
    pub processed: bool,
    pub item: VoiceHistoryItem,
    pub remaining: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
struct VoiceEventPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    normalized_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    voice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    origin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    word_count: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RawVoiceEvent {
    id: String,
    kind: String,
    content: String,
    provider: String,
    created_at: String,
    status: String,
    updated_at: String,
    payload_json: String,
}

pub fn voice_list_providers_for_db() -> Vec<VoiceProvider> {
    builtin_voice_providers()
}

#[tauri::command]
pub fn voice_list_providers() -> Result<Vec<VoiceProvider>, AppError> {
    Ok(voice_list_providers_for_db())
}

pub fn voice_status_for_db(db: &Database) -> Result<VoiceSettings, AppError> {
    ensure_voice_schema(db)?;
    load_voice_settings(db)
}

#[tauri::command]
pub fn voice_status(db: State<'_, Database>) -> Result<VoiceSettings, AppError> {
    voice_status_for_db(db.inner())
}

pub fn voice_update_settings_for_db(
    db: &Database,
    request: VoiceUpdateSettingsRequest,
) -> Result<VoiceSettings, AppError> {
    ensure_voice_schema(db)?;
    let mut settings = load_voice_settings(db)?;

    if let Some(enabled) = request.enabled {
        settings.enabled = enabled;
    }
    if let Some(stt_provider) = request.stt_provider {
        settings.stt_provider = normalize_provider_id(VoiceProviderKind::Stt, &stt_provider)?;
    }
    if let Some(tts_provider) = request.tts_provider {
        settings.tts_provider = normalize_provider_id(VoiceProviderKind::Tts, &tts_provider)?;
    }
    if let Some(transcription_language) = request.transcription_language {
        settings.transcription_language = normalize_required_value(
            &transcription_language,
            "voice transcription language cannot be empty",
        )?;
    }
    if let Some(preferred_voice) = request.preferred_voice {
        settings.preferred_voice = Some(normalize_required_value(
            &preferred_voice,
            "preferred voice cannot be empty",
        )?);
    }
    if let Some(auto_speak_transcripts) = request.auto_speak_transcripts {
        settings.auto_speak_transcripts = auto_speak_transcripts;
    }

    settings.updated_at = Utc::now().to_rfc3339();
    save_voice_settings(db, &settings)?;
    Ok(settings)
}

#[tauri::command]
pub fn voice_update_settings(
    db: State<'_, Database>,
    request: VoiceUpdateSettingsRequest,
) -> Result<VoiceSettings, AppError> {
    voice_update_settings_for_db(db.inner(), request)
}

pub fn voice_set_enabled_for_db(
    db: &Database,
    request: VoiceSetEnabledRequest,
) -> Result<VoiceSettings, AppError> {
    voice_update_settings_for_db(
        db,
        VoiceUpdateSettingsRequest {
            enabled: Some(request.enabled),
            stt_provider: None,
            tts_provider: None,
            transcription_language: None,
            preferred_voice: None,
            auto_speak_transcripts: None,
        },
    )
}

#[tauri::command]
pub fn voice_set_enabled(
    db: State<'_, Database>,
    request: VoiceSetEnabledRequest,
) -> Result<VoiceSettings, AppError> {
    voice_set_enabled_for_db(db.inner(), request)
}

pub fn voice_transcribe_for_db(
    db: &Database,
    request: VoiceTranscribeRequest,
) -> Result<VoiceTranscriptionResult, AppError> {
    ensure_voice_schema(db)?;
    let settings = require_voice_enabled(db)?;
    let normalized_transcript =
        normalize_required_value(&request.text, "voice transcript cannot be empty")?;
    let source = normalize_optional_value(request.source.as_deref())
        .unwrap_or_else(|| DEFAULT_TRANSCRIPTION_SOURCE.to_string());
    let language = normalize_optional_value(request.language.as_deref())
        .unwrap_or_else(|| settings.transcription_language.clone());
    let word_count = count_words(&normalized_transcript);

    insert_voice_event(
        db,
        "transcription",
        &normalized_transcript,
        &settings.stt_provider,
        VOICE_STATUS_COMPLETED,
        &VoiceEventPayload {
            normalized_text: Some(normalized_transcript.clone()),
            source: Some(source.clone()),
            language: Some(language.clone()),
            voice: None,
            origin: None,
            word_count: Some(word_count),
        },
    )?;

    let queued_for_speech = request
        .auto_queue_for_speech
        .unwrap_or(settings.auto_speak_transcripts);
    if queued_for_speech {
        queue_speech_event(
            db,
            &settings,
            VoiceSpeakRequest {
                text: normalized_transcript.clone(),
                voice: settings.preferred_voice.clone(),
                origin: Some("transcription".to_string()),
            },
        )?;
    }

    Ok(VoiceTranscriptionResult {
        transcript: normalized_transcript.clone(),
        provider: settings.stt_provider,
        normalized_transcript,
        source,
        language,
        word_count,
        queued_for_speech,
    })
}

#[tauri::command]
pub fn voice_transcribe(
    db: State<'_, Database>,
    request: VoiceTranscribeRequest,
) -> Result<VoiceTranscriptionResult, AppError> {
    voice_transcribe_for_db(db.inner(), request)
}

pub fn voice_transcribe_stub_for_db(
    db: &Database,
    request: VoiceTranscribeStubRequest,
) -> Result<VoiceTranscriptionResult, AppError> {
    voice_transcribe_for_db(
        db,
        VoiceTranscribeRequest {
            text: request.transcript,
            source: Some("compatibility-wrapper".to_string()),
            language: None,
            auto_queue_for_speech: None,
        },
    )
}

#[tauri::command]
pub fn voice_transcribe_stub(
    db: State<'_, Database>,
    request: VoiceTranscribeStubRequest,
) -> Result<VoiceTranscriptionResult, AppError> {
    voice_transcribe_stub_for_db(db.inner(), request)
}

pub fn voice_speak_for_db(
    db: &Database,
    request: VoiceSpeakRequest,
) -> Result<VoiceSpeakResult, AppError> {
    ensure_voice_schema(db)?;
    let settings = require_voice_enabled(db)?;
    queue_speech_event(db, &settings, request)
}

#[tauri::command]
pub fn voice_speak(
    db: State<'_, Database>,
    request: VoiceSpeakRequest,
) -> Result<VoiceSpeakResult, AppError> {
    voice_speak_for_db(db.inner(), request)
}

pub fn voice_speak_stub_for_db(
    db: &Database,
    request: VoiceSpeakStubRequest,
) -> Result<VoiceSpeakResult, AppError> {
    voice_speak_for_db(
        db,
        VoiceSpeakRequest {
            text: request.text,
            voice: None,
            origin: Some("compatibility-wrapper".to_string()),
        },
    )
}

#[tauri::command]
pub fn voice_speak_stub(
    db: State<'_, Database>,
    request: VoiceSpeakStubRequest,
) -> Result<VoiceSpeakResult, AppError> {
    voice_speak_stub_for_db(db.inner(), request)
}

fn default_voice_settings() -> VoiceSettings {
    VoiceSettings {
        enabled: false,
        stt_provider: LOCAL_STT_PROVIDER.to_string(),
        tts_provider: LOCAL_TTS_PROVIDER.to_string(),
        updated_at: Utc::now().to_rfc3339(),
        transcription_language: DEFAULT_TRANSCRIPTION_LANGUAGE.to_string(),
        preferred_voice: None,
        auto_speak_transcripts: false,
    }
}

pub fn voice_summary_for_db(db: &Database) -> Result<VoiceSummary, AppError> {
    ensure_voice_schema(db)?;
    let settings = load_voice_settings(db)?;
    let transcription_count = query_voice_event_count(db, "transcription")?;
    let queued_speak_count = query_voice_event_count(db, "speech")?;
    let pending_speak_count =
        query_voice_event_count_with_status(db, "speech", VOICE_STATUS_QUEUED)?;
    let history_count = query_total_voice_event_count(db, None)?;
    let last_transcript =
        query_latest_voice_event(db, Some("transcription"), None)?.map(|event| event.content);
    let last_spoken_text = query_latest_voice_event(db, Some("speech"), Some(VOICE_STATUS_SPOKEN))?
        .map(|event| event.content);
    let last_event = query_latest_voice_event(db, None, None)?;

    Ok(VoiceSummary {
        enabled: settings.enabled,
        stt_provider: settings.stt_provider,
        tts_provider: settings.tts_provider,
        updated_at: settings.updated_at,
        transcription_count,
        queued_speak_count,
        pending_speak_count,
        history_count,
        last_transcript,
        last_spoken_text,
        last_event_kind: last_event.as_ref().map(|event| event.kind.clone()),
        last_event_at: last_event.map(|event| event.updated_at),
    })
}

#[tauri::command]
pub fn voice_summary(db: State<'_, Database>) -> Result<VoiceSummary, AppError> {
    voice_summary_for_db(db.inner())
}

pub fn voice_list_history_for_db(
    db: &Database,
    request: VoiceHistoryListRequest,
) -> Result<VoiceHistoryListResult, AppError> {
    ensure_voice_schema(db)?;
    let kind = normalize_optional_value(request.kind.as_deref());
    let limit = request
        .limit
        .unwrap_or(DEFAULT_HISTORY_LIMIT)
        .clamp(1, MAX_HISTORY_LIMIT);
    let total = query_total_voice_event_count(db, kind.as_deref())?;
    let include_payload = request.include_payload;

    let items = db.with_connection(|conn| {
        let mut events = Vec::new();
        if let Some(kind) = kind.as_deref() {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, kind, content, provider, created_at, status, updated_at, payload_json
                FROM voice_events
                WHERE kind = ?1
                ORDER BY sequence DESC, created_at DESC
                LIMIT ?2
                "#,
            )?;
            let rows = stmt.query_map(rusqlite::params![kind, limit], map_raw_voice_event)?;
            for row in rows {
                events.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, kind, content, provider, created_at, status, updated_at, payload_json
                FROM voice_events
                ORDER BY sequence DESC, created_at DESC
                LIMIT ?1
                "#,
            )?;
            let rows = stmt.query_map(rusqlite::params![limit], map_raw_voice_event)?;
            for row in rows {
                events.push(row?);
            }
        }
        Ok(events)
    })?;

    Ok(VoiceHistoryListResult {
        total,
        items: items
            .into_iter()
            .map(|event| history_item_from_raw(event, include_payload))
            .collect(),
    })
}

#[tauri::command]
pub fn voice_list_history(
    db: State<'_, Database>,
    request: VoiceHistoryListRequest,
) -> Result<VoiceHistoryListResult, AppError> {
    voice_list_history_for_db(db.inner(), request)
}

pub fn voice_process_speak_queue_for_db(
    db: &Database,
    request: VoiceProcessSpeakQueueRequest,
) -> Result<VoiceProcessSpeakQueueResult, AppError> {
    ensure_voice_schema(db)?;
    let mark_status = normalize_queue_completion_status(request.mark_status.as_deref())?;
    let next_item = query_oldest_queued_speech_event(db)?;

    let Some(item) = next_item else {
        return Ok(VoiceProcessSpeakQueueResult {
            processed: false,
            item: empty_history_item(),
            remaining: 0,
        });
    };

    let updated_at = Utc::now().to_rfc3339();
    db.execute(
        "UPDATE voice_events SET status = ?1, updated_at = ?2 WHERE id = ?3",
        &[&mark_status as &dyn rusqlite::ToSql, &updated_at, &item.id],
    )?;

    let updated = query_voice_event_by_id(db, &item.id)?.ok_or_else(|| {
        AppError::storage("queued voice item disappeared before it could be returned")
    })?;
    let remaining = query_voice_event_count_with_status(db, "speech", VOICE_STATUS_QUEUED)?;

    Ok(VoiceProcessSpeakQueueResult {
        processed: true,
        item: history_item_from_raw(updated, true),
        remaining,
    })
}

#[tauri::command]
pub fn voice_process_speak_queue(
    db: State<'_, Database>,
    request: VoiceProcessSpeakQueueRequest,
) -> Result<VoiceProcessSpeakQueueResult, AppError> {
    voice_process_speak_queue_for_db(db.inner(), request)
}

fn ensure_voice_schema(db: &Database) -> Result<(), AppError> {
    db.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS voice_events (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            content TEXT NOT NULL,
            provider TEXT NOT NULL,
            created_at TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'completed',
            updated_at TEXT NOT NULL DEFAULT '',
            payload_json TEXT NOT NULL DEFAULT '{}',
            sequence INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_voice_events_kind_created_at
            ON voice_events(kind, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_voice_events_kind_status_sequence
            ON voice_events(kind, status, sequence ASC);
        "#,
    )?;
    ensure_voice_event_column(db, "status", "TEXT NOT NULL DEFAULT 'completed'")?;
    ensure_voice_event_column(db, "updated_at", "TEXT NOT NULL DEFAULT ''")?;
    ensure_voice_event_column(db, "payload_json", "TEXT NOT NULL DEFAULT '{}'")?;
    ensure_voice_event_column(db, "sequence", "INTEGER NOT NULL DEFAULT 0")?;
    Ok(())
}

fn load_voice_settings(db: &Database) -> Result<VoiceSettings, AppError> {
    match db.query_row(
        "SELECT value_json FROM app_settings WHERE key = ?1",
        &[&VOICE_SETTINGS_KEY],
        |row| row.get::<_, String>(0),
    ) {
        Ok(json) => {
            let settings: VoiceSettings =
                serde_json::from_str(&json).map_err(AppError::from_json_error)?;
            Ok(normalize_voice_settings(settings))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(default_voice_settings()),
        Err(err) => Err(AppError::storage(format!(
            "Failed to load voice settings: {}",
            err
        ))),
    }
}

fn save_voice_settings(db: &Database, settings: &VoiceSettings) -> Result<(), AppError> {
    let normalized = normalize_voice_settings(settings.clone());
    let json = serde_json::to_string(&normalized).map_err(AppError::from_json_error)?;
    db.execute(
        "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES (?, ?, ?)",
        &[
            &VOICE_SETTINGS_KEY as &dyn rusqlite::ToSql,
            &json,
            &normalized.updated_at,
        ],
    )?;
    Ok(())
}

fn insert_voice_event(
    db: &Database,
    kind: &str,
    content: &str,
    provider: &str,
    status: &str,
    payload: &VoiceEventPayload,
) -> Result<(), AppError> {
    let id = format!("voice-{}", Uuid::new_v4());
    let created_at = Utc::now().to_rfc3339();
    let payload_json = serde_json::to_string(payload).map_err(AppError::from_json_error)?;
    let sequence = next_voice_event_sequence(db)?;

    db.execute(
        r#"
        INSERT INTO voice_events (
            id,
            kind,
            content,
            provider,
            created_at,
            status,
            updated_at,
            payload_json,
            sequence
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        &[
            &id as &dyn rusqlite::ToSql,
            &kind,
            &content,
            &provider,
            &created_at,
            &status,
            &created_at,
            &payload_json,
            &sequence,
        ],
    )?;
    Ok(())
}

fn query_voice_event_count(db: &Database, kind: &str) -> Result<u32, AppError> {
    let count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM voice_events WHERE kind = ?1",
            &[&kind],
            |row| row.get(0),
        )
        .map_err(|err| AppError::storage(format!("Failed to count voice events: {}", err)))?;
    Ok(count.max(0) as u32)
}

fn query_voice_event_count_with_status(
    db: &Database,
    kind: &str,
    status: &str,
) -> Result<u32, AppError> {
    let count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM voice_events WHERE kind = ?1 AND status = ?2",
            &[&kind, &status],
            |row| row.get(0),
        )
        .map_err(|err| AppError::storage(format!("Failed to count voice events: {}", err)))?;
    Ok(count.max(0) as u32)
}

fn query_total_voice_event_count(db: &Database, kind: Option<&str>) -> Result<u32, AppError> {
    let count: i64 = if let Some(kind) = kind {
        db.query_row(
            "SELECT COUNT(*) FROM voice_events WHERE kind = ?1",
            &[&kind],
            |row| row.get(0),
        )
    } else {
        db.query_row("SELECT COUNT(*) FROM voice_events", &[], |row| row.get(0))
    }
    .map_err(|err| AppError::storage(format!("Failed to count voice events: {}", err)))?;
    Ok(count.max(0) as u32)
}

fn ensure_voice_event_column(
    db: &Database,
    column_name: &str,
    column_definition: &str,
) -> Result<(), AppError> {
    let exists = db.with_connection(|conn| {
        let mut stmt = conn.prepare("PRAGMA table_info(voice_events)")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        let mut found = false;
        for row in rows {
            if row? == column_name {
                found = true;
                break;
            }
        }
        Ok(found)
    })?;

    if !exists {
        db.execute_batch(&format!(
            "ALTER TABLE voice_events ADD COLUMN {} {};",
            column_name, column_definition
        ))?;
    }

    Ok(())
}

fn builtin_voice_providers() -> Vec<VoiceProvider> {
    let local_boundary = "Local text-only workflow: does not capture microphone audio and does not synthesize audio.".to_string();
    vec![
        VoiceProvider {
            id: LOCAL_STT_PROVIDER.to_string(),
            label: "Local Text Capture".to_string(),
            kind: VoiceProviderKind::Stt,
            local_only: true,
            transport: "text-entry".to_string(),
            interaction_model: "manual_text_input".to_string(),
            supports_audio_input: false,
            supports_audio_output: false,
            capabilities: vec![
                "manual_transcription".to_string(),
                "history_persistence".to_string(),
                "language_tagging".to_string(),
            ],
            compatibility_aliases: vec![LEGACY_STUB_PROVIDER.to_string()],
            runtime_boundary: local_boundary.clone(),
            notes: "Local-only text intake that normalizes manually entered transcript text and persists transcript history; it is not an audio STT provider.".to_string(),
        },
        VoiceProvider {
            id: LOCAL_TTS_PROVIDER.to_string(),
            label: "Local Speak Queue".to_string(),
            kind: VoiceProviderKind::Tts,
            local_only: true,
            transport: "queue".to_string(),
            interaction_model: "queued_text_output".to_string(),
            supports_audio_input: false,
            supports_audio_output: false,
            capabilities: vec![
                "queue_enqueue".to_string(),
                "queue_process_next".to_string(),
                "history_persistence".to_string(),
            ],
            compatibility_aliases: vec![LEGACY_STUB_PROVIDER.to_string()],
            runtime_boundary: local_boundary,
            notes: "Local-only text output queue that persists requested utterances until the app marks them processed; it is not an audio TTS provider.".to_string(),
        },
    ]
}

fn normalize_voice_settings(mut settings: VoiceSettings) -> VoiceSettings {
    settings.stt_provider =
        normalize_provider_alias(VoiceProviderKind::Stt, settings.stt_provider.as_str());
    settings.tts_provider =
        normalize_provider_alias(VoiceProviderKind::Tts, settings.tts_provider.as_str());
    if settings.updated_at.trim().is_empty() {
        settings.updated_at = Utc::now().to_rfc3339();
    }
    settings.transcription_language =
        normalize_optional_value(Some(settings.transcription_language.as_str()))
            .unwrap_or_else(|| DEFAULT_TRANSCRIPTION_LANGUAGE.to_string());
    settings.preferred_voice = normalize_optional_value(settings.preferred_voice.as_deref());
    settings
}

fn normalize_provider_id(kind: VoiceProviderKind, provider_id: &str) -> Result<String, AppError> {
    let normalized = normalize_provider_alias(kind.clone(), provider_id);
    let exists = builtin_voice_providers()
        .into_iter()
        .any(|provider| provider.kind == kind && provider.id == normalized);
    if exists {
        Ok(normalized)
    } else {
        Err(AppError::validation(format!(
            "unsupported {} provider: {}",
            match kind {
                VoiceProviderKind::Stt => "stt",
                VoiceProviderKind::Tts => "tts",
            },
            provider_id.trim()
        )))
    }
}

fn normalize_provider_alias(kind: VoiceProviderKind, provider_id: &str) -> String {
    let trimmed = provider_id.trim();
    if trimmed.is_empty() || trimmed == LEGACY_STUB_PROVIDER {
        match kind {
            VoiceProviderKind::Stt => LOCAL_STT_PROVIDER.to_string(),
            VoiceProviderKind::Tts => LOCAL_TTS_PROVIDER.to_string(),
        }
    } else {
        trimmed.to_string()
    }
}

fn normalize_required_value(value: &str, message: &str) -> Result<String, AppError> {
    let normalized = normalize_text(value);
    if normalized.is_empty() {
        Err(AppError::validation(message))
    } else {
        Ok(normalized)
    }
}

fn normalize_optional_value(value: Option<&str>) -> Option<String> {
    value.map(normalize_text).filter(|value| !value.is_empty())
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn count_words(value: &str) -> u32 {
    value.split_whitespace().count() as u32
}

fn require_voice_enabled(db: &Database) -> Result<VoiceSettings, AppError> {
    let settings = load_voice_settings(db)?;
    if settings.enabled {
        Ok(settings)
    } else {
        Err(AppError::validation(
            "voice workflow is disabled; enable voice before transcribing or speaking",
        ))
    }
}

fn queue_speech_event(
    db: &Database,
    settings: &VoiceSettings,
    request: VoiceSpeakRequest,
) -> Result<VoiceSpeakResult, AppError> {
    let text = normalize_required_value(&request.text, "voice speak text cannot be empty")?;
    let voice = normalize_optional_value(request.voice.as_deref())
        .or_else(|| settings.preferred_voice.clone());
    let origin = normalize_optional_value(request.origin.as_deref())
        .unwrap_or_else(|| DEFAULT_SPEAK_ORIGIN.to_string());
    let id = format!("voice-{}", Uuid::new_v4());
    let created_at = Utc::now().to_rfc3339();
    let payload_json = serde_json::to_string(&VoiceEventPayload {
        normalized_text: Some(text.clone()),
        source: None,
        language: None,
        voice: voice.clone(),
        origin: Some(origin.clone()),
        word_count: Some(count_words(&text)),
    })
    .map_err(AppError::from_json_error)?;
    let sequence = next_voice_event_sequence(db)?;

    db.execute(
        r#"
        INSERT INTO voice_events (
            id,
            kind,
            content,
            provider,
            created_at,
            status,
            updated_at,
            payload_json,
            sequence
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        &[
            &id as &dyn rusqlite::ToSql,
            &"speech",
            &text,
            &settings.tts_provider,
            &created_at,
            &VOICE_STATUS_QUEUED,
            &created_at,
            &payload_json,
            &sequence,
        ],
    )?;

    Ok(VoiceSpeakResult {
        queued: true,
        provider: settings.tts_provider.clone(),
        text,
        id,
        status: VOICE_STATUS_QUEUED.to_string(),
        voice,
        origin,
        created_at,
    })
}

fn next_voice_event_sequence(db: &Database) -> Result<i64, AppError> {
    let max_sequence: Option<i64> = db
        .query_row("SELECT MAX(sequence) FROM voice_events", &[], |row| {
            row.get(0)
        })
        .map_err(|err| AppError::storage(format!("Failed to read voice sequence: {}", err)))?;
    Ok(max_sequence.unwrap_or(0) + 1)
}

fn map_raw_voice_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawVoiceEvent> {
    Ok(RawVoiceEvent {
        id: row.get(0)?,
        kind: row.get(1)?,
        content: row.get(2)?,
        provider: row.get(3)?,
        created_at: row.get(4)?,
        status: row.get(5)?,
        updated_at: row.get(6)?,
        payload_json: row.get(7)?,
    })
}

fn query_latest_voice_event(
    db: &Database,
    kind: Option<&str>,
    status: Option<&str>,
) -> Result<Option<RawVoiceEvent>, AppError> {
    db.with_connection(|conn| {
        let mut events = Vec::new();
        match (kind, status) {
            (Some(kind), Some(status)) => {
                let mut stmt = conn.prepare(
                    r#"
                    SELECT id, kind, content, provider, created_at, status, updated_at, payload_json
                    FROM voice_events
                    WHERE kind = ?1 AND status = ?2
                    ORDER BY sequence DESC, created_at DESC
                    LIMIT 1
                    "#,
                )?;
                let rows = stmt.query_map(rusqlite::params![kind, status], map_raw_voice_event)?;
                for row in rows {
                    events.push(row?);
                }
            }
            (Some(kind), None) => {
                let mut stmt = conn.prepare(
                    r#"
                    SELECT id, kind, content, provider, created_at, status, updated_at, payload_json
                    FROM voice_events
                    WHERE kind = ?1
                    ORDER BY sequence DESC, created_at DESC
                    LIMIT 1
                    "#,
                )?;
                let rows = stmt.query_map(rusqlite::params![kind], map_raw_voice_event)?;
                for row in rows {
                    events.push(row?);
                }
            }
            (None, None) => {
                let mut stmt = conn.prepare(
                    r#"
                    SELECT id, kind, content, provider, created_at, status, updated_at, payload_json
                    FROM voice_events
                    ORDER BY sequence DESC, created_at DESC
                    LIMIT 1
                    "#,
                )?;
                let rows = stmt.query_map(rusqlite::params![], map_raw_voice_event)?;
                for row in rows {
                    events.push(row?);
                }
            }
            (None, Some(_)) => return Ok(None),
        }
        Ok(events.into_iter().next())
    })
}

fn query_oldest_queued_speech_event(db: &Database) -> Result<Option<RawVoiceEvent>, AppError> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, kind, content, provider, created_at, status, updated_at, payload_json
            FROM voice_events
            WHERE kind = 'speech' AND status = 'queued'
            ORDER BY sequence ASC, created_at ASC
            LIMIT 1
            "#,
        )?;
        let rows = stmt.query_map(rusqlite::params![], map_raw_voice_event)?;
        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }
        Ok(events.into_iter().next())
    })
}

fn query_voice_event_by_id(db: &Database, id: &str) -> Result<Option<RawVoiceEvent>, AppError> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, kind, content, provider, created_at, status, updated_at, payload_json
            FROM voice_events
            WHERE id = ?1
            LIMIT 1
            "#,
        )?;
        let rows = stmt.query_map(rusqlite::params![id], map_raw_voice_event)?;
        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }
        Ok(events.into_iter().next())
    })
}

fn history_item_from_raw(event: RawVoiceEvent, include_payload: bool) -> VoiceHistoryItem {
    let payload = deserialize_voice_event_payload(&event.payload_json);
    let normalized_text = payload
        .normalized_text
        .clone()
        .unwrap_or_else(|| event.content.clone());

    VoiceHistoryItem {
        id: event.id,
        kind: event.kind,
        provider: event.provider,
        status: event.status,
        text: normalized_text.clone(),
        created_at: event.created_at,
        updated_at: event.updated_at,
        payload_text: if include_payload {
            Some(normalized_text)
        } else {
            None
        },
        source: payload.source,
        language: payload.language,
        voice: payload.voice,
        origin: payload.origin,
        word_count: payload.word_count,
    }
}

fn deserialize_voice_event_payload(payload_json: &str) -> VoiceEventPayload {
    serde_json::from_str(payload_json).unwrap_or_default()
}

fn empty_history_item() -> VoiceHistoryItem {
    VoiceHistoryItem {
        id: String::new(),
        kind: "speech".to_string(),
        provider: String::new(),
        status: "idle".to_string(),
        text: String::new(),
        created_at: String::new(),
        updated_at: String::new(),
        payload_text: None,
        source: None,
        language: None,
        voice: None,
        origin: None,
        word_count: None,
    }
}

fn normalize_queue_completion_status(status: Option<&str>) -> Result<String, AppError> {
    let normalized =
        normalize_optional_value(status).unwrap_or_else(|| VOICE_STATUS_SPOKEN.to_string());
    match normalized.as_str() {
        VOICE_STATUS_SPOKEN | VOICE_STATUS_COMPLETED => Ok(normalized),
        _ => Err(AppError::validation(
            "voice queue completion status must be either 'spoken' or 'completed'",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        VoiceHistoryListRequest, VoiceProcessSpeakQueueRequest, VoiceSetEnabledRequest,
        VoiceSpeakRequest, VoiceSpeakStubRequest, VoiceTranscribeRequest,
        VoiceTranscribeStubRequest, VoiceUpdateSettingsRequest, voice_list_history_for_db,
        voice_list_providers_for_db, voice_process_speak_queue_for_db, voice_set_enabled_for_db,
        voice_speak_for_db, voice_speak_stub_for_db, voice_status_for_db, voice_summary_for_db,
        voice_transcribe_for_db, voice_transcribe_stub_for_db, voice_update_settings_for_db,
    };
    use crate::backend::Database;

    #[test]
    fn voice_provider_catalog_declares_local_non_audio_boundary() {
        let providers = voice_list_providers_for_db();

        assert_eq!(providers.len(), 2);
        assert!(providers.iter().all(|provider| provider.local_only));
        assert!(
            providers
                .iter()
                .all(|provider| !provider.supports_audio_input)
        );
        assert!(
            providers
                .iter()
                .all(|provider| !provider.supports_audio_output)
        );
        assert!(providers.iter().all(|provider| {
            provider
                .runtime_boundary
                .contains("does not capture microphone audio")
        }));
        assert!(providers.iter().all(|provider| {
            provider
                .runtime_boundary
                .contains("does not synthesize audio")
        }));
        assert!(
            providers
                .iter()
                .all(|provider| !provider.id.contains("stub"))
        );
    }

    #[test]
    fn voice_settings_persist_enabled_state_and_local_provider_preferences() {
        let db = Database::in_memory().expect("database should initialize");
        assert!(!voice_status_for_db(&db).expect("status").enabled);

        let enabled = voice_set_enabled_for_db(&db, VoiceSetEnabledRequest { enabled: true })
            .expect("enable voice");
        assert!(enabled.enabled);

        let updated = voice_update_settings_for_db(
            &db,
            VoiceUpdateSettingsRequest {
                enabled: None,
                stt_provider: Some("local-text-capture".to_string()),
                tts_provider: Some("local-speak-queue".to_string()),
                transcription_language: Some("en-US".to_string()),
                preferred_voice: Some("narrator".to_string()),
                auto_speak_transcripts: Some(true),
            },
        )
        .expect("update settings");

        assert_eq!(updated.stt_provider, "local-text-capture");
        assert_eq!(updated.tts_provider, "local-speak-queue");
        assert_eq!(updated.transcription_language, "en-US");
        assert_eq!(updated.preferred_voice.as_deref(), Some("narrator"));
        assert!(updated.auto_speak_transcripts);

        let persisted = voice_status_for_db(&db).expect("status");
        assert!(persisted.enabled);
        assert_eq!(persisted.stt_provider, "local-text-capture");
        assert_eq!(persisted.tts_provider, "local-speak-queue");
        assert_eq!(persisted.transcription_language, "en-US");
        assert_eq!(persisted.preferred_voice.as_deref(), Some("narrator"));
        assert!(persisted.auto_speak_transcripts);
    }

    #[test]
    fn voice_transcribe_records_history_and_richer_summary() {
        let db = Database::in_memory().expect("database should initialize");
        voice_set_enabled_for_db(&db, VoiceSetEnabledRequest { enabled: true }).expect("enable");
        voice_update_settings_for_db(
            &db,
            VoiceUpdateSettingsRequest {
                enabled: None,
                stt_provider: Some("local-text-capture".to_string()),
                tts_provider: None,
                transcription_language: Some("en-US".to_string()),
                preferred_voice: None,
                auto_speak_transcripts: Some(false),
            },
        )
        .expect("settings");

        let transcript = voice_transcribe_for_db(
            &db,
            VoiceTranscribeRequest {
                text: " hello world from local input ".to_string(),
                source: Some("manual".to_string()),
                language: None,
                auto_queue_for_speech: None,
            },
        )
        .expect("transcribe");

        assert_eq!(transcript.transcript, "hello world from local input");
        assert_eq!(
            transcript.normalized_transcript,
            "hello world from local input"
        );
        assert_eq!(transcript.provider, "local-text-capture");
        assert_eq!(transcript.source, "manual");
        assert_eq!(transcript.language, "en-US");
        assert_eq!(transcript.word_count, 5);

        let summary = voice_summary_for_db(&db).expect("summary");
        assert_eq!(summary.transcription_count, 1);
        assert_eq!(summary.history_count, 1);
        assert_eq!(summary.pending_speak_count, 0);
        assert_eq!(
            summary.last_transcript.as_deref(),
            Some("hello world from local input")
        );
        assert_eq!(summary.last_event_kind.as_deref(), Some("transcription"));

        let history = voice_list_history_for_db(
            &db,
            VoiceHistoryListRequest {
                kind: None,
                limit: Some(10),
                include_payload: true,
            },
        )
        .expect("history");
        assert_eq!(history.items.len(), 1);
        assert_eq!(history.items[0].kind, "transcription");
        assert_eq!(
            history.items[0].payload_text.as_deref(),
            Some("hello world from local input")
        );
        assert_eq!(history.items[0].status, "completed");
    }

    #[test]
    fn voice_speak_queue_can_be_listed_processed_and_wrapped_by_stub_commands() {
        let db = Database::in_memory().expect("database should initialize");
        voice_set_enabled_for_db(&db, VoiceSetEnabledRequest { enabled: true }).expect("enable");

        let first = voice_speak_for_db(
            &db,
            VoiceSpeakRequest {
                text: " first queued message ".to_string(),
                voice: Some("narrator".to_string()),
                origin: Some("assistant".to_string()),
            },
        )
        .expect("queue first");
        assert!(first.queued);
        assert_eq!(first.status, "queued");

        let second = voice_speak_stub_for_db(
            &db,
            VoiceSpeakStubRequest {
                text: " second queued message ".to_string(),
            },
        )
        .expect("queue second");
        assert_eq!(second.text, "second queued message");

        let before = voice_summary_for_db(&db).expect("summary");
        assert_eq!(before.queued_speak_count, 2);
        assert_eq!(before.pending_speak_count, 2);
        assert_eq!(before.last_event_kind.as_deref(), Some("speech"));

        let queued = voice_list_history_for_db(
            &db,
            VoiceHistoryListRequest {
                kind: Some("speech".to_string()),
                limit: Some(10),
                include_payload: true,
            },
        )
        .expect("queued history");
        assert_eq!(queued.total, 2);
        assert_eq!(queued.items[0].status, "queued");
        assert_eq!(queued.items[1].status, "queued");

        let processed = voice_process_speak_queue_for_db(
            &db,
            VoiceProcessSpeakQueueRequest {
                mark_status: Some("spoken".to_string()),
            },
        )
        .expect("process queue");
        assert!(processed.processed);
        assert_eq!(processed.item.text, "first queued message");
        assert_eq!(processed.item.status, "spoken");

        let after = voice_summary_for_db(&db).expect("summary");
        assert_eq!(after.queued_speak_count, 2);
        assert_eq!(after.pending_speak_count, 1);
        assert_eq!(
            after.last_spoken_text.as_deref(),
            Some("first queued message")
        );

        let final_item = voice_process_speak_queue_for_db(
            &db,
            VoiceProcessSpeakQueueRequest { mark_status: None },
        )
        .expect("process second");
        assert_eq!(final_item.item.text, "second queued message");
        assert_eq!(final_item.item.status, "spoken");

        let empty = voice_process_speak_queue_for_db(
            &db,
            VoiceProcessSpeakQueueRequest { mark_status: None },
        )
        .expect("queue empty");
        assert!(!empty.processed);
        assert!(empty.item.id.is_empty());
        assert_eq!(empty.remaining, 0);
    }

    #[test]
    fn voice_stub_transcribe_wrapper_uses_real_local_workflow() {
        let db = Database::in_memory().expect("database should initialize");
        voice_set_enabled_for_db(&db, VoiceSetEnabledRequest { enabled: true }).expect("enable");

        let transcript = voice_transcribe_stub_for_db(
            &db,
            VoiceTranscribeStubRequest {
                transcript: " hello ".to_string(),
            },
        )
        .expect("transcribe stub");
        assert_eq!(transcript.transcript, "hello");

        let history = voice_list_history_for_db(
            &db,
            VoiceHistoryListRequest {
                kind: Some("transcription".to_string()),
                limit: Some(10),
                include_payload: true,
            },
        )
        .expect("history");
        assert_eq!(history.total, 1);
        assert_eq!(history.items[0].provider, "local-text-capture");
    }
}
