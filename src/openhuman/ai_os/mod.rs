//! `ai_os` — user-managed AI provider management, conversation storage,
//! and token-usage tracking.
//!
//! Exposes JSON-RPC methods under the `ai_os` namespace:
//! - `ai_os.provider_add` / `_update` / `_delete` / `_list` / `_test`
//! - `ai_os.conversation_create` / `_list` / `_get` / `_delete`
//! - `ai_os.chat_send`
//! - `ai_os.usage_get`

pub mod chat;
mod schemas;
pub mod store;
pub mod types;

mod rpc;

pub use schemas::{
    all_controller_schemas as all_ai_os_controller_schemas,
    all_registered_controllers as all_ai_os_registered_controllers,
};
