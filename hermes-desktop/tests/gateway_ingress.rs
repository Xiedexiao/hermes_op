use hermes_desktop::backend::{
    Database, GatewayDirection, GatewaySource, SessionService, SessionServiceImpl, SessionSource,
};
use hermes_desktop::commands::gateway::{
    GatewayIngestMessageRequest, GatewayListRecentConversationsRequest,
    GatewayListRecentMessagesRequest, gateway_ingest_message_for_db,
    gateway_list_recent_conversations_for_db, gateway_list_recent_messages_for_db,
};

#[test]
fn gateway_ingest_message_creates_conversation_message_and_session() {
    let db = Database::in_memory().expect("database should initialize");

    let ingested = gateway_ingest_message_for_db(
        &db,
        GatewayIngestMessageRequest {
            source: "telegram".to_string(),
            external_conversation_id: "chat-123".to_string(),
            external_thread_id: None,
            external_message_id: "msg-001".to_string(),
            channel_name: Some("ops-alerts".to_string()),
            participant_display: Some("Alice".to_string()),
            direction: None,
            sender_id: Some("user-42".to_string()),
            sender_display: Some("Alice".to_string()),
            subject: None,
            body: "Agent, please summarize the incident.".to_string(),
            payload_json: Some(serde_json::json!({ "priority": "high" })),
            received_at: Some("2026-04-23T10:00:00Z".to_string()),
        },
    )
    .expect("gateway message should ingest");

    assert_eq!(ingested.conversation.source, GatewaySource::Telegram);
    assert_eq!(ingested.conversation.external_conversation_id, "chat-123");
    assert_eq!(ingested.conversation.external_thread_id, "");
    assert_eq!(ingested.message.source, GatewaySource::Telegram);
    assert_eq!(ingested.message.external_message_id, "msg-001");
    assert_eq!(ingested.message.direction, GatewayDirection::Inbound);
    assert_eq!(
        ingested.message.payload_json.as_ref(),
        Some(&serde_json::json!({ "priority": "high" }))
    );

    let sessions = SessionServiceImpl::new(db.clone())
        .list_recent(10)
        .expect("sessions should list");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, ingested.conversation.session_id);
    assert_eq!(sessions[0].source, SessionSource::Telegram);

    let conversations = gateway_list_recent_conversations_for_db(
        &db,
        Some(GatewayListRecentConversationsRequest { limit: Some(10) }),
    )
    .expect("conversations should list");
    assert_eq!(conversations.len(), 1);
    assert_eq!(conversations[0].id, ingested.conversation.id);
    assert_eq!(conversations[0].session_id, sessions[0].id);
    assert_eq!(conversations[0].last_message_at, "2026-04-23T10:00:00Z");

    let messages = gateway_list_recent_messages_for_db(
        &db,
        GatewayListRecentMessagesRequest {
            conversation_id: Some(ingested.conversation.id.clone()),
            session_id: None,
            limit: Some(10),
        },
    )
    .expect("messages should list");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].id, ingested.message.id);
    assert_eq!(messages[0].conversation_id, ingested.conversation.id);
}

#[test]
fn gateway_ingest_message_reuses_session_for_same_source_conversation_and_validates_input() {
    let db = Database::in_memory().expect("database should initialize");

    let first = gateway_ingest_message_for_db(
        &db,
        GatewayIngestMessageRequest {
            source: "slack".to_string(),
            external_conversation_id: "C12345".to_string(),
            external_thread_id: Some("T67890".to_string()),
            external_message_id: "slack-001".to_string(),
            channel_name: Some("#deployments".to_string()),
            participant_display: Some("Release Bot".to_string()),
            direction: Some("inbound".to_string()),
            sender_id: Some("U123".to_string()),
            sender_display: Some("Release Bot".to_string()),
            subject: Some("deploy".to_string()),
            body: "Deployment started".to_string(),
            payload_json: None,
            received_at: Some("2026-04-23T10:00:00Z".to_string()),
        },
    )
    .expect("first slack message should ingest");

    let second = gateway_ingest_message_for_db(
        &db,
        GatewayIngestMessageRequest {
            source: "slack".to_string(),
            external_conversation_id: "C12345".to_string(),
            external_thread_id: Some("T67890".to_string()),
            external_message_id: "slack-002".to_string(),
            channel_name: Some("#deployments".to_string()),
            participant_display: Some("Release Bot".to_string()),
            direction: Some("inbound".to_string()),
            sender_id: Some("U123".to_string()),
            sender_display: Some("Release Bot".to_string()),
            subject: Some("deploy".to_string()),
            body: "Deployment finished".to_string(),
            payload_json: Some(serde_json::json!({ "status": "ok" })),
            received_at: Some("2026-04-23T10:05:00Z".to_string()),
        },
    )
    .expect("second slack message should ingest");

    assert_eq!(first.conversation.id, second.conversation.id);
    assert_eq!(
        first.conversation.session_id,
        second.conversation.session_id
    );
    assert_eq!(second.conversation.external_thread_id, "T67890");

    let sessions = SessionServiceImpl::new(db.clone())
        .list_recent(10)
        .expect("sessions should list");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].source, SessionSource::Slack);

    let messages = gateway_list_recent_messages_for_db(
        &db,
        GatewayListRecentMessagesRequest {
            conversation_id: Some(second.conversation.id.clone()),
            session_id: None,
            limit: Some(10),
        },
    )
    .expect("messages should list");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].external_message_id, "slack-002");
    assert_eq!(messages[1].external_message_id, "slack-001");

    let invalid = gateway_ingest_message_for_db(
        &db,
        GatewayIngestMessageRequest {
            source: "sms".to_string(),
            external_conversation_id: "unsupported".to_string(),
            external_thread_id: None,
            external_message_id: "sms-001".to_string(),
            channel_name: None,
            participant_display: None,
            direction: None,
            sender_id: None,
            sender_display: None,
            subject: None,
            body: "hello".to_string(),
            payload_json: None,
            received_at: None,
        },
    )
    .expect_err("unsupported source should fail");
    assert_eq!(invalid.code, "validation_error");

    let blank_body = gateway_ingest_message_for_db(
        &db,
        GatewayIngestMessageRequest {
            source: "email".to_string(),
            external_conversation_id: "thread-42".to_string(),
            external_thread_id: None,
            external_message_id: "email-001".to_string(),
            channel_name: None,
            participant_display: Some("Inbox".to_string()),
            direction: None,
            sender_id: Some("ops@example.test".to_string()),
            sender_display: Some("Ops".to_string()),
            subject: Some("blank".to_string()),
            body: "   ".to_string(),
            payload_json: None,
            received_at: None,
        },
    )
    .expect_err("blank body should fail");
    assert_eq!(blank_body.code, "validation_error");
}
