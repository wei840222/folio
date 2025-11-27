use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use temporalio_common::protos::coresdk::FromJsonPayloadExt;
use temporalio_sdk::{ActContext, ActivityError, WfContext, WorkflowResult};

#[derive(Debug, Serialize, Deserialize)]
pub struct FileExpirationInput {
    pub path: PathBuf,
    pub ttl: Duration,
}

// Activity: Delete the file
pub async fn delete_file_activity(
    _ctx: ActContext,
    input: FileExpirationInput,
) -> Result<(), ActivityError> {
    log::info!("Executing delete_file_activity for path: {:?}", input.path);

    if input.path.exists() {
        std::fs::remove_file(&input.path).map_err(|e| {
            log::error!("Failed to delete file {:?}: {}", input.path, e);
            ActivityError::from(anyhow::anyhow!("Failed to delete file: {}", e))
        })?;
        log::info!("Successfully deleted file: {:?}", input.path);
    } else {
        log::warn!("File not found during deletion: {:?}", input.path);
    }

    Ok(())
}

// Workflow: Wait (handled by start delay) and then delete
pub async fn file_expiration_workflow(ctx: WfContext) -> WorkflowResult<()> {
    let payload = ctx.get_args().first().expect("No input provided");
    let input = FileExpirationInput::from_json_payload(payload)?;

    if !ctx.is_replaying() {
        log::info!(
            "Workflow started for file: {:?}. Waiting {} seconds before deleting...",
            input.path,
            input.ttl.as_secs()
        );
    }

    // Wait for the TTL
    ctx.timer(input.ttl).await;

    if !ctx.is_replaying() {
        log::info!("Timer fired. Executing activity...");
    }

    let activity_opts = temporalio_sdk::ActivityOptions {
        activity_type: "delete_file_activity".to_string(),
        input: temporalio_common::protos::coresdk::AsJsonPayloadExt::as_json_payload(&input)?,
        start_to_close_timeout: Some(Duration::from_secs(10)),
        ..Default::default()
    };

    let result = ctx.activity(activity_opts).await;

    if !ctx.is_replaying() {
        log::info!(
            "Workflow completed for file: {:?} with result: {:#?}",
            input.path,
            result
        );
    }

    Ok(().into())
}
