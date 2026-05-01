use hermes_desktop::backend::Database;
use hermes_desktop::commands::terminal_backends::{
    TerminalBackendSaveProfileRequest, terminal_backend_list_profiles_for_db,
    terminal_backend_list_status_for_db_with_checker, terminal_backend_save_profile_for_db,
    terminal_backend_test_profile_for_db_with_checker,
};

fn command_checker(available: &'static [&'static str]) -> impl Fn(&str) -> bool {
    move |command| available.contains(&command)
}

#[test]
fn terminal_backend_registry_seeds_default_profiles() {
    let db = Database::in_memory().expect("database should initialize");

    let profiles = terminal_backend_list_profiles_for_db(&db).expect("profiles should list");

    let kinds: Vec<_> = profiles
        .iter()
        .map(|profile| profile.kind.as_str())
        .collect();
    assert_eq!(
        kinds,
        vec!["local", "docker", "ssh", "modal", "daytona", "singularity"]
    );
    assert!(profiles.iter().all(|profile| profile.enabled));

    let persisted_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM terminal_backend_profiles",
            &[],
            |row| row.get(0),
        )
        .expect("seeded profiles should persist");
    assert_eq!(persisted_count, 6);
}

#[test]
fn terminal_backend_profiles_persist_config_and_report_status() {
    let db = Database::in_memory().expect("database should initialize");

    let saved = terminal_backend_save_profile_for_db(
        &db,
        TerminalBackendSaveProfileRequest {
            id: Some("ssh-staging".to_string()),
            kind: "ssh".to_string(),
            display_name: "SSH Staging".to_string(),
            enabled: true,
            config: serde_json::json!({
                "host": "staging.example.test",
                "user": "agent"
            }),
        },
    )
    .expect("ssh profile should save");
    assert_eq!(saved.id, "ssh-staging");
    assert_eq!(saved.kind, "ssh");

    let profiles = terminal_backend_list_profiles_for_db(&db).expect("profiles should reload");
    let ssh_profile = profiles
        .iter()
        .find(|profile| profile.id == "ssh-staging")
        .expect("saved profile should be present");
    assert_eq!(ssh_profile.config["host"], "staging.example.test");

    let statuses = terminal_backend_list_status_for_db_with_checker(&db, command_checker(&["ssh"]))
        .expect("statuses should list");
    let ssh_status = statuses
        .iter()
        .find(|status| status.id == "ssh-staging")
        .expect("saved ssh status should be present");
    assert_eq!(ssh_status.availability, "available");
    assert!(ssh_status.configured);
    assert!(ssh_status.testable);
}

#[test]
fn terminal_backend_test_uses_command_availability_for_docker() {
    let db = Database::in_memory().expect("database should initialize");

    let unavailable =
        terminal_backend_test_profile_for_db_with_checker(&db, "docker", command_checker(&[]))
            .expect("docker test should return a result");
    assert_eq!(unavailable.status, "failed");
    assert_eq!(unavailable.availability, "unavailable");
    assert!(unavailable.message.contains("docker"));

    let available = terminal_backend_test_profile_for_db_with_checker(
        &db,
        "docker",
        command_checker(&["docker"]),
    )
    .expect("docker test should return a result");
    assert_eq!(available.status, "passed");
    assert_eq!(available.availability, "available");
}

#[test]
fn terminal_backend_cloud_profiles_are_configured_or_unavailable_without_deps() {
    let db = Database::in_memory().expect("database should initialize");

    let modal_default_statuses =
        terminal_backend_list_status_for_db_with_checker(&db, command_checker(&[]))
            .expect("statuses should list");
    let modal_default = modal_default_statuses
        .iter()
        .find(|status| status.id == "modal")
        .expect("modal default should be present");
    assert_eq!(modal_default.availability, "unavailable");
    assert!(!modal_default.configured);
    assert!(!modal_default.testable);

    terminal_backend_save_profile_for_db(
        &db,
        TerminalBackendSaveProfileRequest {
            id: Some("modal-team".to_string()),
            kind: "modal".to_string(),
            display_name: "Modal Team".to_string(),
            enabled: true,
            config: serde_json::json!({
                "token_ref": "env:MODAL_TOKEN",
                "workspace": "team"
            }),
        },
    )
    .expect("modal profile should save");

    let statuses = terminal_backend_list_status_for_db_with_checker(&db, command_checker(&[]))
        .expect("statuses should list");
    let modal_status = statuses
        .iter()
        .find(|status| status.id == "modal-team")
        .expect("saved modal status should be present");
    assert_eq!(modal_status.availability, "configured");
    assert!(modal_status.configured);
    assert!(!modal_status.testable);

    let test_result =
        terminal_backend_test_profile_for_db_with_checker(&db, "modal-team", command_checker(&[]))
            .expect("modal test should return a result");
    assert_eq!(test_result.status, "skipped");
    assert_eq!(test_result.availability, "configured");
}
