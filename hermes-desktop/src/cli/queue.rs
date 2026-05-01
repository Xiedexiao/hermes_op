use super::{CliError, open_app_database, render_queue_usage};
use chrono::Utc;
use hermes_desktop::backend::Database;
use serde::{Deserialize, Serialize};

const CLI_PROMPT_QUEUE_KEY: &str = "cli_prompt_queue";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PersistedPromptQueue {
    #[serde(default)]
    prompts: Vec<String>,
}

pub(super) fn handle_command(command: &str) -> Result<String, CliError> {
    let trimmed = command.trim();
    if trimmed == "/queue" {
        return Err(CliError::InvalidUsage(render_queue_usage()));
    }

    let db = open_app_database()?;
    if trimmed == "/queue status" {
        return Ok(render_status(&load_prompt_queue_for_db(&db)?));
    }

    let prompt = trimmed
        .strip_prefix("/queue")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CliError::InvalidUsage(render_queue_usage()))?;

    let mut prompts = load_prompt_queue_for_db(&db)?;
    prompts.push(prompt.to_string());
    save_prompt_queue_for_db(&db, &prompts)?;

    Ok(format!(
        "queue\tqueued\tcount={}\tprompt={}\n",
        prompts.len(),
        prompt,
    ))
}

pub(super) fn load_queued_prompts() -> Result<Vec<String>, CliError> {
    let db = open_app_database()?;
    load_prompt_queue_for_db(&db)
}

pub(super) fn clear_queued_prompts() -> Result<(), CliError> {
    let db = open_app_database()?;
    save_prompt_queue_for_db(&db, &[])
}

fn render_status(prompts: &[String]) -> String {
    let mut output = format!("queue\tcount={}\n", prompts.len());
    for (index, prompt) in prompts.iter().enumerate() {
        output.push_str(&format!(
            "queue\titem\tindex={}\tprompt={}\n",
            index + 1,
            prompt
        ));
    }
    output
}

fn load_prompt_queue_for_db(db: &Database) -> Result<Vec<String>, CliError> {
    let stored = db
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = ?",
            &[&CLI_PROMPT_QUEUE_KEY as &dyn rusqlite::ToSql],
            |row| row.get::<_, String>(0),
        )
        .ok();

    let prompts = stored
        .and_then(|json| serde_json::from_str::<PersistedPromptQueue>(&json).ok())
        .unwrap_or_default()
        .prompts
        .into_iter()
        .map(|prompt| prompt.trim().to_string())
        .filter(|prompt| !prompt.is_empty())
        .collect();

    Ok(prompts)
}

fn save_prompt_queue_for_db(db: &Database, prompts: &[String]) -> Result<(), CliError> {
    if prompts.is_empty() {
        db.execute(
            "DELETE FROM app_settings WHERE key = ?",
            &[&CLI_PROMPT_QUEUE_KEY as &dyn rusqlite::ToSql],
        )
        .map_err(|err| CliError::Runtime(err.to_string()))?;
        return Ok(());
    }

    let value_json = serde_json::to_string(&PersistedPromptQueue {
        prompts: prompts.to_vec(),
    })
    .map_err(|err| CliError::Runtime(err.to_string()))?;
    let now = Utc::now().to_rfc3339();
    let params: Vec<&dyn rusqlite::ToSql> = vec![&CLI_PROMPT_QUEUE_KEY, &value_json, &now];
    db.execute(
        "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES (?, ?, ?)",
        &params,
    )
    .map_err(|err| CliError::Runtime(err.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::render_status;

    #[test]
    fn render_status_lists_each_prompt_in_order() {
        assert_eq!(
            render_status(&["first".to_string(), "second".to_string()]),
            concat!(
                "queue\tcount=2\n",
                "queue\titem\tindex=1\tprompt=first\n",
                "queue\titem\tindex=2\tprompt=second\n",
            )
        );
    }
}
