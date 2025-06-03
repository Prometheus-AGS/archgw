use crate::providers::openai::types::{ChatCompletionsRequest, ChatCompletionsResponse};
pub use crate::providers::openai::types::{Choice, Message, Usage};

use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroqRequest {
    #[serde(flatten)]
    pub base: ChatCompletionsRequest,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroqResponse {
    #[serde(flatten)]
    pub base: ChatCompletionsResponse,
}
