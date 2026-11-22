//! Tool-calling agent loop.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::ProviderConfig;
use crate::error::AiError;
use crate::types::{Message, ToolCall, ToolSpec};

/// Progress reported by [`run_agent`].
#[derive(Clone, Debug, PartialEq)]
pub enum AgentEvent {
    /// The model produced text.
    AssistantText(String),
    /// The model asked for a tool call (emitted before it runs).
    ToolCall(ToolCall),
    /// A tool call finished.
    ToolResult {
        /// The call that ran.
        call: ToolCall,
        /// Output (or error message) sent back to the model.
        result: String,
        /// Whether the tool failed.
        is_error: bool,
    },
}

/// How a [`run_agent`] run ended (cancellation and failures are errors).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentOutcome {
    /// The model answered without asking for more tools.
    Finished,
    /// `max_steps` model calls were made and the model still wanted to call
    /// tools: the task is probably unfinished. The last tool results are in
    /// the history, so the user can ask the model to continue.
    StepLimit,
}

/// Text sent back for calls skipped because the run was cancelled, so the
/// history stays valid (every tool call needs a result).
const CANCELLED_RESULT: &str = "Cancelled by the user.";

/// Run the model with tools until it answers without calling any.
///
/// Each step sends `history` to the model, appends the assistant message to
/// `history`, then runs every requested tool through `exec` and appends the
/// results. `exec` returns `Ok(output)` or `Err(error message)`; both are
/// passed to the model. The loop stops when the model makes no tool calls or
/// after `max_steps` model calls. It returns [`AgentOutcome::Finished`] when the
/// model answered, and [`AgentOutcome::StepLimit`] when the step budget ran out
/// while the model was still calling tools (the last tool results are then in
/// `history`).
///
/// `cancel` is checked before each model call, after it returns (the reply is
/// then discarded) and before each tool runs. On cancellation the function
/// returns [`AiError::Cancelled`]; `history` stays consistent: pending tool
/// calls get a "cancelled" result.
///
/// Blocking: run it off the UI thread.
#[allow(clippy::too_many_arguments)]
pub fn run_agent(
    cfg: &ProviderConfig,
    system: &str,
    history: &mut Vec<Message>,
    tools: &[ToolSpec],
    exec: &mut dyn FnMut(&ToolCall) -> Result<String, String>,
    on_event: &mut dyn FnMut(AgentEvent),
    max_steps: usize,
    cancel: &AtomicBool,
) -> Result<AgentOutcome, AiError> {
    let cancelled = || cancel.load(Ordering::Relaxed);
    for _ in 0..max_steps {
        if cancelled() {
            return Err(AiError::Cancelled);
        }
        let response = crate::chat(cfg, system, history, tools)?;
        if cancelled() {
            return Err(AiError::Cancelled);
        }
        let message = response.message;
        let calls = message.tool_calls.clone();
        if !message.content.trim().is_empty() {
            on_event(AgentEvent::AssistantText(message.content.clone()));
        }
        history.push(message);
        if calls.is_empty() {
            return Ok(AgentOutcome::Finished);
        }
        let mut stopped = false;
        for call in calls {
            if stopped || cancelled() {
                stopped = true;
                history.push(Message::tool_result(&call, CANCELLED_RESULT, true));
                continue;
            }
            on_event(AgentEvent::ToolCall(call.clone()));
            let (result, is_error) = match exec(&call) {
                Ok(out) => (out, false),
                Err(err) => (err, true),
            };
            history.push(Message::tool_result(&call, result.clone(), is_error));
            on_event(AgentEvent::ToolResult {
                call,
                result,
                is_error,
            });
        }
        if stopped {
            return Err(AiError::Cancelled);
        }
    }
    Ok(AgentOutcome::StepLimit)
}
