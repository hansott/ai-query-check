use serde::Deserialize;

#[derive(Deserialize)]
pub struct HookInput {
    pub tool_input: ToolInput,
}

#[derive(Deserialize)]
pub struct ToolInput {
    pub command: String,
}
