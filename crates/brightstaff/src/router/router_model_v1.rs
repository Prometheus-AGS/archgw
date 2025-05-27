use common::{
    api::open_ai::{ChatCompletionsRequest, ContentType, Message},
    consts::{SYSTEM_ROLE, USER_ROLE},
    tokenizer::Tokenizer,
};
use serde::{Deserialize, Serialize};
use tracing::debug;

use super::router_model::{RouterModel, RoutingModelError};

pub const MAX_TOKEN_LEN: usize = 2048; // Default max token length for the routing model
pub const ARCH_ROUTER_V1_SYSTEM_PROMPT: &str = r#"
You are a helpful assistant designed to find the best suited route.
You are provided with route description within <routes></routes> XML tags:
<routes>
{routes}
</routes>

Your task is to decide which route is best suit with user intent on the conversation in <conversation></conversation> XML tags.  Follow the instruction:
1. If the latest intent from user is irrelevant, response with empty route {"route": ""}.
2. If the user request is full fill and user thank or ending the conversation , response with empty route {"route": ""}.
3. Understand user latest intent and find the best match route in <routes></routes> xml tags.

Based on your analysis, provide your response in the following JSON formats if you decide to match any route:
{"route": "route_name"}


<conversation>
{conversation}
</conversation>
"#;

pub type Result<T> = std::result::Result<T, RoutingModelError>;
pub struct RouterModelV1 {
    llm_providers_with_usage_yaml: String,
    routing_model: String,
    max_token_length: usize,
    tokenizer: Box<dyn Tokenizer + Send + Sync>,
}
impl RouterModelV1 {
    pub fn new(
        llm_providers_with_usage_yaml: String,
        routing_model: String,
        max_token_length: usize,
        tokenizer: Box<dyn Tokenizer + Send + Sync>,
    ) -> Self {
        RouterModelV1 {
            llm_providers_with_usage_yaml,
            routing_model,
            max_token_length,
            tokenizer,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmRouterResponse {
    pub route: Option<String>,
}

impl RouterModel for RouterModelV1 {
    fn generate_request(&self, messages: &[Message]) -> ChatCompletionsRequest {
        let mut messages_vec = messages
            .iter()
            .filter(|m| m.role != SYSTEM_ROLE)
            .map(|m| {
                let content_json_str = serde_json::to_string(&m.content).unwrap_or_default();
                format!("{}: {}", m.role, content_json_str)
            })
            .collect::<Vec<String>>();

        let mut messages_content: String;

        loop {
            messages_content = ARCH_ROUTER_V1_SYSTEM_PROMPT
                .replace("{routes}", &self.llm_providers_with_usage_yaml)
                .replace("{conversation}", messages_vec.join("\n").as_str());

            let token_count = self
                .tokenizer
                .token_count(&messages_content, &self.routing_model)
                .unwrap_or(0);
            if token_count <= self.max_token_length || messages_vec.len() <= 2 {
                if messages_vec.len() <= 2 {
                    debug!("RouterModelV1: conversation is too short, using remaining messages",)
                }
                break;
            }
            debug!(
                "RouterModelV1: token count {} exceeds max token length {}, truncating conversation",
                token_count,
                self.max_token_length
            );
            // trim top two elements from the conversation
            messages_vec = messages_vec.into_iter().skip(2).collect::<Vec<String>>();
        }

        ChatCompletionsRequest {
            model: self.routing_model.clone(),
            messages: vec![Message {
                content: Some(ContentType::Text(messages_content)),
                role: USER_ROLE.to_string(),
                model: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            tools: None,
            stream: false,
            stream_options: None,
            metadata: None,
        }
    }

    fn parse_response(&self, content: &str) -> Result<Option<String>> {
        if content.is_empty() {
            return Ok(None);
        }
        let router_resp_fixed = fix_json_response(content);
        let router_response: LlmRouterResponse = serde_json::from_str(router_resp_fixed.as_str())?;

        let selected_llm = router_response.route.unwrap_or_default().to_string();

        if selected_llm.is_empty() {
            return Ok(None);
        }

        Ok(Some(selected_llm))
    }

    fn get_model_name(&self) -> String {
        self.routing_model.clone()
    }
}

fn fix_json_response(body: &str) -> String {
    let mut updated_body = body.to_string();

    updated_body = updated_body.replace("'", "\"");

    if updated_body.contains("\\n") {
        updated_body = updated_body.replace("\\n", "");
    }

    if updated_body.starts_with("```json") {
        updated_body = updated_body
            .strip_prefix("```json")
            .unwrap_or(&updated_body)
            .to_string();
    }

    if updated_body.ends_with("```") {
        updated_body = updated_body
            .strip_suffix("```")
            .unwrap_or(&updated_body)
            .to_string();
    }

    updated_body
}

impl std::fmt::Debug for dyn RouterModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RouterModel")
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::tracing::init_tracer;

    use super::*;
    use common::tokenizer::TiktokenTokenizer;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_system_prompt_format() {
        let expected_prompt = r#"
You are a helpful assistant designed to find the best suited route.
You are provided with route description within <routes></routes> XML tags:
<routes>
route1: description1
route2: description2
</routes>

Your task is to decide which route is best suit with user intent on the conversation in <conversation></conversation> XML tags.  Follow the instruction:
1. If the latest intent from user is irrelevant, response with empty route {"route": ""}.
2. If the user request is full fill and user thank or ending the conversation , response with empty route {"route": ""}.
3. Understand user latest intent and find the best match route in <routes></routes> xml tags.

Based on your analysis, provide your response in the following JSON formats if you decide to match any route:
{"route": "route_name"}


<conversation>
user: "Hello, I want to book a flight."
assistant: "Sure, where would you like to go?"
user: "seattle"
</conversation>
"#;

        let routes_yaml = "route1: description1\nroute2: description2";
        let routing_model = "test-model".to_string();
        let router = RouterModelV1::new(
            routes_yaml.to_string(),
            routing_model.clone(),
            usize::MAX,
            Box::new(TiktokenTokenizer {}),
        );

        let messages = vec![
            Message {
                role: "system".to_string(),
                content: Some(ContentType::Text(
                    "You are a helpful assistant.".to_string(),
                )),
                ..Default::default()
            },
            Message {
                role: "user".to_string(),
                content: Some(ContentType::Text(
                    "Hello, I want to book a flight.".to_string(),
                )),
                ..Default::default()
            },
            Message {
                role: "assistant".to_string(),
                content: Some(ContentType::Text(
                    "Sure, where would you like to go?".to_string(),
                )),
                ..Default::default()
            },
            Message {
                role: "user".to_string(),
                content: Some(ContentType::Text("seattle".to_string())),
                ..Default::default()
            },
        ];

        let req = router.generate_request(&messages);

        let prompt = req.messages[0].content.as_ref().unwrap();

        assert_eq!(expected_prompt, prompt.to_string());
    }

    #[test]
    fn test_conversation_exceed_token_count() {
        let _tracer = init_tracer();
        let expected_prompt = r#"
You are a helpful assistant designed to find the best suited route.
You are provided with route description within <routes></routes> XML tags:
<routes>
route1: description1
route2: description2
</routes>

Your task is to decide which route is best suit with user intent on the conversation in <conversation></conversation> XML tags.  Follow the instruction:
1. If the latest intent from user is irrelevant, response with empty route {"route": ""}.
2. If the user request is full fill and user thank or ending the conversation , response with empty route {"route": ""}.
3. Understand user latest intent and find the best match route in <routes></routes> xml tags.

Based on your analysis, provide your response in the following JSON formats if you decide to match any route:
{"route": "route_name"}


<conversation>
user: "I want to book a flight."
assistant: "Sure, where would you like to go?"
user: "seattle"
</conversation>
"#;

        let routes_yaml = "route1: description1\nroute2: description2";
        let routing_model = "test-model".to_string();
        let router = RouterModelV1::new(
            routes_yaml.to_string(),
            routing_model.clone(),
            210,
            Box::new(TiktokenTokenizer {}),
        );

        let messages = vec![
            Message {
                role: "system".to_string(),
                content: Some(ContentType::Text(
                    "You are a helpful assistant.".to_string(),
                )),
                ..Default::default()
            },
            Message {
                role: "user".to_string(),
                content: Some(ContentType::Text("Hi".to_string())),
                ..Default::default()
            },
            Message {
                role: "assistant".to_string(),
                content: Some(ContentType::Text("Hello! How can I assist you".to_string())),
                ..Default::default()
            },
            Message {
                role: "user".to_string(),
                content: Some(ContentType::Text("I want to book a flight.".to_string())),
                ..Default::default()
            },
            Message {
                role: "assistant".to_string(),
                content: Some(ContentType::Text(
                    "Sure, where would you like to go?".to_string(),
                )),
                ..Default::default()
            },
            Message {
                role: "user".to_string(),
                content: Some(ContentType::Text("seattle".to_string())),
                ..Default::default()
            },
        ];

        let req = router.generate_request(&messages);

        let prompt = req.messages[0].content.as_ref().unwrap();

        assert_eq!(expected_prompt, prompt.to_string());
    }

    #[test]
    fn test_conversation_exceed_token_count_large_single_message() {
        let _tracer = init_tracer();
        let expected_prompt = r#"
You are a helpful assistant designed to find the best suited route.
You are provided with route description within <routes></routes> XML tags:
<routes>
route1: description1
route2: description2
</routes>

Your task is to decide which route is best suit with user intent on the conversation in <conversation></conversation> XML tags.  Follow the instruction:
1. If the latest intent from user is irrelevant, response with empty route {"route": ""}.
2. If the user request is full fill and user thank or ending the conversation , response with empty route {"route": ""}.
3. Understand user latest intent and find the best match route in <routes></routes> xml tags.

Based on your analysis, provide your response in the following JSON formats if you decide to match any route:
{"route": "route_name"}


<conversation>
user: "Seatte, WA. But I also need to know about the weather there, and if there are any good restaurants nearby, and what the best time to visit is, and also if there are any events happening in the city."
</conversation>
"#;

        let routes_yaml = "route1: description1\nroute2: description2";
        let routing_model = "test-model".to_string();
        let router = RouterModelV1::new(
            routes_yaml.to_string(),
            routing_model.clone(),
            210,
            Box::new(TiktokenTokenizer {}),
        );

        let messages = vec![
            Message {
                role: "system".to_string(),
                content: Some(ContentType::Text(
                    "You are a helpful assistant.".to_string(),
                )),
                ..Default::default()
            },
            Message {
                role: "user".to_string(),
                content: Some(ContentType::Text("Hi".to_string())),
                ..Default::default()
            },
            Message {
                role: "assistant".to_string(),
                content: Some(ContentType::Text("Hello! How can I assist you".to_string())),
                ..Default::default()
            },
            Message {
                role: "user".to_string(),
                content: Some(ContentType::Text("I want to book a flight.".to_string())),
                ..Default::default()
            },
            Message {
                role: "assistant".to_string(),
                content: Some(ContentType::Text(
                    "Sure, where would you like to go?".to_string(),
                )),
                ..Default::default()
            },
            Message {
                role: "user".to_string(),
                content: Some(ContentType::Text("Seatte, WA. But I also need to know about the weather there, \
                                                 and if there are any good restaurants nearby, and what the \
                                                 best time to visit is, and also if there are any events \
                                                 happening in the city.".to_string())),
                ..Default::default()
            },
        ];

        let req = router.generate_request(&messages);

        let prompt = req.messages[0].content.as_ref().unwrap();

        assert_eq!(expected_prompt, prompt.to_string());
    }

    #[test]
    fn test_parse_response() {
        let router = RouterModelV1::new(
            "route1: description1\nroute2: description2".to_string(),
            "test-model".to_string(),
            2000,
            Box::new(TiktokenTokenizer {}),
        );

        // Case 1: Valid JSON with non-empty route
        let input = r#"{"route": "route1"}"#;
        let result = router.parse_response(input).unwrap();
        assert_eq!(result, Some("route1".to_string()));

        // Case 2: Valid JSON with empty route
        let input = r#"{"route": ""}"#;
        let result = router.parse_response(input).unwrap();
        assert_eq!(result, None);

        // Case 3: Valid JSON with null route
        let input = r#"{"route": null}"#;
        let result = router.parse_response(input).unwrap();
        assert_eq!(result, None);

        // Case 4: JSON missing route field
        let input = r#"{}"#;
        let result = router.parse_response(input).unwrap();
        assert_eq!(result, None);

        // Case 4.1: empty string
        let input = r#""#;
        let result = router.parse_response(input).unwrap();
        assert_eq!(result, None);

        // Case 5: Malformed JSON
        let input = r#"{"route": "route1""#; // missing closing }
        let result = router.parse_response(input);
        assert!(result.is_err());

        // Case 6: Single quotes and \n in JSON
        let input = "{'route': 'route2'}\\n";
        let result = router.parse_response(input).unwrap();
        assert_eq!(result, Some("route2".to_string()));

        // Case 7: Code block marker
        let input = "```json\n{\"route\": \"route1\"}\n```";
        let result = router.parse_response(input).unwrap();
        assert_eq!(result, Some("route1".to_string()));
    }
}
