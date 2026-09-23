//! Exercises the goal extension's empty-response breaker through the public API.

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::ThreadGoalGetResponse;
use codex_app_server_protocol::ThreadGoalSetResponse;
use codex_app_server_protocol::ThreadGoalStatus;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStatus;
use codex_config::types::AuthCredentialsStoreMode;
use codex_features::Feature;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn goal_continuations_use_configured_cyber_access_program() -> Result<()> {
    let server = responses::start_mock_server().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/v1/responses"))
        .respond_with(wiremock::ResponseTemplate::new(/*s*/ 426))
        .mount(&server)
        .await;
    // The existing empty-response breaker stops this goal after three turns.
    let scripts = (1..=3)
        .map(|turn| {
            let mut final_item = responses::ev_assistant_message(&format!("final-{turn}"), "");
            final_item["item"]["phase"] = json!("final_answer");
            responses::sse(vec![
                final_item,
                responses::ev_completed(&format!("response-{turn}")),
            ])
        })
        .collect();
    let requests = responses::mount_sse_sequence(&server, scripts).await;
    let home = TempDir::new()?;
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            "model = \"gpt-5.4\"\napproval_policy = \"never\"\nopenai_base_url = \"{}/v1\"\ncyber_access_program = \"daybreak_blue\"\ncli_auth_credentials_store = \"file\"\n[features]\ngoals = true\n",
            server.uri()
        ),
    )?;
    write_chatgpt_auth(
        home.path(),
        ChatGptAuthFixture::new("test-token").account_id("test-account"),
        AuthCredentialsStoreMode::File,
    )?;
    let mut app = TestAppServer::builder()
        .with_codex_home(home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .without_managed_config()
        .build_initialized()
        .await?;
    let request = app
        .send_thread_start_request_with_auto_env(ThreadStartParams::default())
        .await?;
    let ThreadStartResponse { thread, .. } = app.read_response(request).await?;
    let request = app
        .send_raw_request(
            "thread/goal/set",
            Some(json!({"threadId": thread.id, "objective": "Finish the fixture task"})),
        )
        .await?;
    let _: ThreadGoalSetResponse = app.read_response(request).await?;
    for _ in 0..3 {
        let completed: TurnCompletedNotification = timeout(
            Duration::from_secs(/*secs*/ 30),
            app.read_notification("turn/completed"),
        )
        .await??;
        assert_eq!(completed.turn.status, TurnStatus::Completed);
        assert_eq!(completed.turn.error, None);
    }
    assert_eq!(
        requests
            .requests()
            .iter()
            .map(|request| request.body_json()["access_programs"].clone())
            .collect::<Vec<_>>(),
        vec![json!({"cyber": "daybreak_blue"}); 3]
    );
    Ok(())
}

#[test_case::test_case(None; "empty")]
#[test_case::test_case(Some("final_answer"); "draft")]
#[test_case::test_case(Some("commentary"); "commentary")]
#[test_case::test_case(Some("tool"); "tool")]
#[tokio::test]
async fn empty_goal_continuations_block_after_three_without_activity(
    recovery: Option<&str>,
) -> Result<()> {
    let turns = if recovery.is_some() { 6 } else { 3 };
    let mut scripts = Vec::new();
    for turn in 1..=turns {
        let mut id = format!("response-{turn}");
        let mut events = vec![responses::ev_response_created(&id)];
        if turn == 3
            && let Some(recovery) = recovery
        {
            if recovery == "tool" {
                events.push(responses::ev_function_call("get-goal", "get_goal", "{}"));
                events.push(responses::ev_completed(&id));
                scripts.push(responses::sse(events));
                id.push_str("-after-tool");
                events = vec![responses::ev_response_created(&id)];
            } else {
                let mut added = responses::ev_assistant_message("progress", "");
                added["type"] = json!("response.output_item.added");
                events.push(added);
                events.push(responses::ev_output_text_delta("Useful progress"));
                let mut event = responses::ev_assistant_message("progress", "Useful progress");
                event["item"]["phase"] = json!(recovery);
                events.push(event);
            }
        }
        let mut empty_final = responses::ev_assistant_message(&format!("empty-final-{turn}"), "");
        empty_final["item"]["phase"] = json!("final_answer");
        events.push(empty_final);
        events.push(responses::ev_completed(&id));
        scripts.push(responses::sse(events));
    }
    let server = create_mock_responses_server_sequence(scripts).await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_model("gpt-5.4")
        .enable_feature(Feature::Goals)
        .write(codex_home.path())?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized()
        .await?;
    let request = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams::default())
        .await?;
    let ThreadStartResponse { thread, .. } = mcp.read_response(request).await?;
    let request = mcp
        .send_raw_request(
            "thread/goal/set",
            Some(json!({"threadId": thread.id, "objective": "Finish the work"})),
        )
        .await?;
    let _: ThreadGoalSetResponse = mcp.read_response(request).await?;

    for turn in 1..=turns {
        if turn == 3 && matches!(recovery, Some("final_answer" | "commentary")) {
            let delta: serde_json::Value = timeout(
                Duration::from_secs(30),
                mcp.read_notification("item/agentMessage/delta"),
            )
            .await??;
            assert_eq!(json!("Useful progress"), delta["delta"]);
        }
        let completed: TurnCompletedNotification = timeout(
            Duration::from_secs(30),
            mcp.read_notification("turn/completed"),
        )
        .await??;
        assert_eq!(TurnStatus::Completed, completed.turn.status);
        assert_eq!(None, completed.turn.error);
    }
    let request = mcp
        .send_raw_request("thread/goal/get", Some(json!({"threadId": thread.id})))
        .await?;
    let result: ThreadGoalGetResponse = mcp.read_response(request).await?;
    assert_eq!(
        ThreadGoalStatus::Blocked,
        result.goal.expect("goal exists").status
    );
    server.verify().await;
    Ok(())
}
