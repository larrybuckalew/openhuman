use serde_json::{Map, Value};

use crate::core::all::{ControllerFuture, RegisteredController};
use crate::core::{ControllerSchema, FieldSchema, TypeSchema};

use super::rpc;

// ─── public exports ───────────────────────────────────────────────────────────

pub fn all_controller_schemas() -> Vec<ControllerSchema> {
    vec![
        schema("provider_add"),
        schema("provider_update"),
        schema("provider_delete"),
        schema("provider_list"),
        schema("provider_test"),
        schema("conversation_create"),
        schema("conversation_list"),
        schema("conversation_get"),
        schema("conversation_delete"),
        schema("chat_send"),
        schema("usage_get"),
    ]
}

pub fn all_registered_controllers() -> Vec<RegisteredController> {
    vec![
        RegisteredController {
            schema: schema("provider_add"),
            handler: handle_provider_add,
        },
        RegisteredController {
            schema: schema("provider_update"),
            handler: handle_provider_update,
        },
        RegisteredController {
            schema: schema("provider_delete"),
            handler: handle_provider_delete,
        },
        RegisteredController {
            schema: schema("provider_list"),
            handler: handle_provider_list,
        },
        RegisteredController {
            schema: schema("provider_test"),
            handler: handle_provider_test,
        },
        RegisteredController {
            schema: schema("conversation_create"),
            handler: handle_conversation_create,
        },
        RegisteredController {
            schema: schema("conversation_list"),
            handler: handle_conversation_list,
        },
        RegisteredController {
            schema: schema("conversation_get"),
            handler: handle_conversation_get,
        },
        RegisteredController {
            schema: schema("conversation_delete"),
            handler: handle_conversation_delete,
        },
        RegisteredController {
            schema: schema("chat_send"),
            handler: handle_chat_send,
        },
        RegisteredController {
            schema: schema("usage_get"),
            handler: handle_usage_get,
        },
    ]
}

// ─── schema definitions ───────────────────────────────────────────────────────

pub fn schema(function: &str) -> ControllerSchema {
    match function {
        "provider_add" => ControllerSchema {
            namespace: "ai_os",
            function: "provider_add",
            description: "Register a new AI provider (OpenAI-compatible, Anthropic, or Google).",
            inputs: vec![
                FieldSchema {
                    name: "name",
                    ty: TypeSchema::String,
                    comment: "Human-readable name for this provider.",
                    required: true,
                },
                FieldSchema {
                    name: "kind",
                    ty: TypeSchema::Enum {
                        variants: vec!["open_ai_compatible", "anthropic", "google"],
                    },
                    comment: "Provider protocol kind.",
                    required: true,
                },
                FieldSchema {
                    name: "base_url",
                    ty: TypeSchema::String,
                    comment: "Base URL for the provider API (e.g. https://api.openai.com/v1).",
                    required: true,
                },
                FieldSchema {
                    name: "api_key",
                    ty: TypeSchema::String,
                    comment: "Optional API key / credential.",
                    required: false,
                },
                FieldSchema {
                    name: "default_model",
                    ty: TypeSchema::String,
                    comment: "Default model to use when none is specified.",
                    required: true,
                },
                FieldSchema {
                    name: "enabled",
                    ty: TypeSchema::Bool,
                    comment: "Whether this provider is active. Defaults to true.",
                    required: false,
                },
            ],
            outputs: vec![FieldSchema {
                name: "provider",
                ty: TypeSchema::Ref("UserProvider"),
                comment: "The newly created provider record.",
                required: true,
            }],
        },

        "provider_update" => ControllerSchema {
            namespace: "ai_os",
            function: "provider_update",
            description: "Update one or more fields on an existing provider.",
            inputs: vec![
                FieldSchema {
                    name: "id",
                    ty: TypeSchema::String,
                    comment: "Provider identifier.",
                    required: true,
                },
                FieldSchema {
                    name: "name",
                    ty: TypeSchema::String,
                    comment: "New display name.",
                    required: false,
                },
                FieldSchema {
                    name: "kind",
                    ty: TypeSchema::String,
                    comment: "New provider kind.",
                    required: false,
                },
                FieldSchema {
                    name: "base_url",
                    ty: TypeSchema::String,
                    comment: "New base URL.",
                    required: false,
                },
                FieldSchema {
                    name: "api_key",
                    ty: TypeSchema::String,
                    comment: "New API key.",
                    required: false,
                },
                FieldSchema {
                    name: "default_model",
                    ty: TypeSchema::String,
                    comment: "New default model.",
                    required: false,
                },
                FieldSchema {
                    name: "enabled",
                    ty: TypeSchema::Bool,
                    comment: "Enable or disable the provider.",
                    required: false,
                },
            ],
            outputs: vec![FieldSchema {
                name: "provider",
                ty: TypeSchema::Ref("UserProvider"),
                comment: "Updated provider record.",
                required: true,
            }],
        },

        "provider_delete" => ControllerSchema {
            namespace: "ai_os",
            function: "provider_delete",
            description: "Delete an AI provider by id.",
            inputs: vec![FieldSchema {
                name: "id",
                ty: TypeSchema::String,
                comment: "Provider identifier to delete.",
                required: true,
            }],
            outputs: vec![FieldSchema {
                name: "ok",
                ty: TypeSchema::Bool,
                comment: "True when the provider was deleted.",
                required: true,
            }],
        },

        "provider_list" => ControllerSchema {
            namespace: "ai_os",
            function: "provider_list",
            description: "List all registered AI providers.",
            inputs: vec![],
            outputs: vec![FieldSchema {
                name: "providers",
                ty: TypeSchema::Array(Box::new(TypeSchema::Ref("UserProvider"))),
                comment: "All providers ordered by creation time.",
                required: true,
            }],
        },

        "provider_test" => ControllerSchema {
            namespace: "ai_os",
            function: "provider_test",
            description: "Send a test message to a provider to verify connectivity and credentials.",
            inputs: vec![FieldSchema {
                name: "id",
                ty: TypeSchema::String,
                comment: "Provider identifier to test.",
                required: true,
            }],
            outputs: vec![
                FieldSchema {
                    name: "ok",
                    ty: TypeSchema::Bool,
                    comment: "True when the provider responded successfully.",
                    required: true,
                },
                FieldSchema {
                    name: "latency_ms",
                    ty: TypeSchema::I64,
                    comment: "Round-trip latency in milliseconds (only present on success).",
                    required: false,
                },
                FieldSchema {
                    name: "error",
                    ty: TypeSchema::String,
                    comment: "Error message (only present on failure).",
                    required: false,
                },
            ],
        },

        "conversation_create" => ControllerSchema {
            namespace: "ai_os",
            function: "conversation_create",
            description: "Create a new conversation with a provider.",
            inputs: vec![
                FieldSchema {
                    name: "provider_id",
                    ty: TypeSchema::String,
                    comment: "Provider to use for this conversation.",
                    required: true,
                },
                FieldSchema {
                    name: "model",
                    ty: TypeSchema::String,
                    comment: "Model to use. Defaults to provider default_model.",
                    required: false,
                },
                FieldSchema {
                    name: "title",
                    ty: TypeSchema::String,
                    comment: "Optional conversation title. Defaults to 'New conversation'.",
                    required: false,
                },
            ],
            outputs: vec![FieldSchema {
                name: "conversation",
                ty: TypeSchema::Ref("AiConversation"),
                comment: "The newly created conversation.",
                required: true,
            }],
        },

        "conversation_list" => ControllerSchema {
            namespace: "ai_os",
            function: "conversation_list",
            description: "List conversations, optionally filtered by provider.",
            inputs: vec![FieldSchema {
                name: "provider_id",
                ty: TypeSchema::String,
                comment: "Optional provider filter.",
                required: false,
            }],
            outputs: vec![FieldSchema {
                name: "conversations",
                ty: TypeSchema::Array(Box::new(TypeSchema::Ref("AiConversation"))),
                comment: "Conversations ordered by most recently updated first.",
                required: true,
            }],
        },

        "conversation_get" => ControllerSchema {
            namespace: "ai_os",
            function: "conversation_get",
            description: "Get a conversation and all its messages.",
            inputs: vec![FieldSchema {
                name: "id",
                ty: TypeSchema::String,
                comment: "Conversation identifier.",
                required: true,
            }],
            outputs: vec![
                FieldSchema {
                    name: "conversation",
                    ty: TypeSchema::Ref("AiConversation"),
                    comment: "The conversation metadata.",
                    required: true,
                },
                FieldSchema {
                    name: "messages",
                    ty: TypeSchema::Array(Box::new(TypeSchema::Ref("AiMessage"))),
                    comment: "All messages in chronological order.",
                    required: true,
                },
            ],
        },

        "conversation_delete" => ControllerSchema {
            namespace: "ai_os",
            function: "conversation_delete",
            description: "Delete a conversation and all its messages.",
            inputs: vec![FieldSchema {
                name: "id",
                ty: TypeSchema::String,
                comment: "Conversation identifier to delete.",
                required: true,
            }],
            outputs: vec![FieldSchema {
                name: "ok",
                ty: TypeSchema::Bool,
                comment: "True when the conversation was deleted.",
                required: true,
            }],
        },

        "chat_send" => ControllerSchema {
            namespace: "ai_os",
            function: "chat_send",
            description: "Send a user message to a conversation and get the assistant reply.",
            inputs: vec![
                FieldSchema {
                    name: "conversation_id",
                    ty: TypeSchema::String,
                    comment: "Conversation to send the message to.",
                    required: true,
                },
                FieldSchema {
                    name: "content",
                    ty: TypeSchema::String,
                    comment: "User message text.",
                    required: true,
                },
            ],
            outputs: vec![
                FieldSchema {
                    name: "message",
                    ty: TypeSchema::Ref("AiMessage"),
                    comment: "The assistant reply message.",
                    required: true,
                },
                FieldSchema {
                    name: "usage",
                    ty: TypeSchema::Object {
                        fields: vec![
                            FieldSchema {
                                name: "input_tokens",
                                ty: TypeSchema::U64,
                                comment: "Input token count for this turn.",
                                required: true,
                            },
                            FieldSchema {
                                name: "output_tokens",
                                ty: TypeSchema::U64,
                                comment: "Output token count for this turn.",
                                required: true,
                            },
                            FieldSchema {
                                name: "charged_amount_usd",
                                ty: TypeSchema::F64,
                                comment: "Amount charged in USD (0 when unavailable).",
                                required: true,
                            },
                        ],
                    },
                    comment: "Token usage for this request.",
                    required: true,
                },
            ],
        },

        "usage_get" => ControllerSchema {
            namespace: "ai_os",
            function: "usage_get",
            description: "Get aggregated token usage and cost broken down by provider.",
            inputs: vec![FieldSchema {
                name: "days",
                ty: TypeSchema::I64,
                comment: "Look-back window in days. Defaults to 30.",
                required: false,
            }],
            outputs: vec![FieldSchema {
                name: "summary",
                ty: TypeSchema::Ref("UsageSummary"),
                comment: "Aggregated usage data for the requested period.",
                required: true,
            }],
        },

        _ => ControllerSchema {
            namespace: "ai_os",
            function: "unknown",
            description: "Unknown ai_os controller.",
            inputs: vec![],
            outputs: vec![FieldSchema {
                name: "error",
                ty: TypeSchema::String,
                comment: "Lookup error details.",
                required: true,
            }],
        },
    }
}

// ─── handler shims ────────────────────────────────────────────────────────────

fn handle_provider_add(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move { rpc::handle_provider_add(params).await })
}

fn handle_provider_update(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move { rpc::handle_provider_update(params).await })
}

fn handle_provider_delete(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move { rpc::handle_provider_delete(params).await })
}

fn handle_provider_list(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move { rpc::handle_provider_list(params).await })
}

fn handle_provider_test(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move { rpc::handle_provider_test(params).await })
}

fn handle_conversation_create(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move { rpc::handle_conversation_create(params).await })
}

fn handle_conversation_list(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move { rpc::handle_conversation_list(params).await })
}

fn handle_conversation_get(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move { rpc::handle_conversation_get(params).await })
}

fn handle_conversation_delete(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move { rpc::handle_conversation_delete(params).await })
}

fn handle_chat_send(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move { rpc::handle_chat_send(params).await })
}

fn handle_usage_get(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move { rpc::handle_usage_get(params).await })
}
