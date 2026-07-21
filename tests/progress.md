

[ COMPLETE ]
  === src/cli/mod.rs ===
  [x] test_parse_list
  [x] test_parse_config_current
  [x] test_parse_config_use
  [x] test_parse_config_path
  [x] test_parse_flags
  [x] test_parse_no_args
  [x] test_parse_config_no_subcommand
  [x] test_parse_config_use_no_profile
  [x] test_parse_unknown_command
  [x] test_parse_help_command
  [x] test_parse_help_flag
  [x] test_parse_command_help_flag
  [x] test_parse_send
  [x] test_parse_send_await_does_not_consume_message
  [x] test_parse_send_with_id
  [x] test_parse_delete
  [x] test_unknown_flag_is_error
  [x] test_unknown_flag_does_not_consume_message
  [x] test_parse_server_action_minimal
  [x] test_parse_server_action_with_profile
  [x] test_parse_server_action_with_body_params
  [x] test_parse_server_action_missing_action
  [x] test_parse_server_action_with_eq_flag
  
[ COMPLETE ]
  === src/cli/output.rs ===
  [x] test_print_json
  [x] test_print_error
  
[ COMPLETE ]
  === src/client/anthropic/mod.rs ===
  [x] test_to_wire_message_plain
  [x] test_to_wire_message_tool_call
  [x] test_to_wire_message_tool_result
  [x] test_to_wire_message_tool_result_json_object
  [x] test_anthropic_response_deserialization

[ COMPLETE ]
  === src/client/anthropic/sse.rs ===
  [x] test_whitespace_only_text_becomes_none
  
[ COMPLETE ]
  === src/client/base.rs ===
  [x] test_is_retryable
  [x] test_base_client_creation
  
[ COMPLETE ]
  === src/client/mod.rs ===
  [x] test_create_provider_defaults_to_anthropic
  
[ COMPLETE ]
  === src/client/openai/api.rs ===
  [x] test_build_request_body_with_system
  [x] test_build_request_body_streaming
  
[ COMPLETE ]
  === src/client/openai/mod.rs ===
  [x] test_to_openai_message_plain
  [x] test_to_openai_message_tool_call
  [x] test_to_openai_message_tool_result
  [x] test_to_openai_message_tool_result_json_object
  [x] test_openai_tool_schema_mapping
  [x] test_openai_response_deserialization
  [x] test_openai_response_with_tool_calls
  
[ COMPLETE ]
  === src/client/openai/sse.rs ===
  [x] test_whitespace_only_text_becomes_none
  [x] test_missing_content_delta_no_panic
  [x] test_missing_choices_no_panic
  [x] test_missing_delta_no_panic
  [x] test_missing_tool_calls_no_panic
  [x] test_missing_finish_reason_no_panic
  [x] test_empty_choices_no_panic
  [x] test_tool_calls_missing_id_no_panic
  [x] test_tool_calls_missing_function_no_panic
  [x] test_reasoning_content_parsed
  [x] test_lm_studio_like_partial_chunks_no_panic
  [x] test_tool_calls_id_arrives_later_no_panic
  
[ COMPLETE ]
  === src/client/resolve.rs ===
  [x] test_resolve_env_inline_whole_value
  [x] test_resolve_env_inline_with_prefix
  [x] test_resolve_env_inline_unset_var
  
[ COMPLETE ]
  === src/cmd/config.rs ===
  [x] test_config_path_ok
  [x] test_config_current_missing
  
[ COMPLETE ]
  === src/cmd/delete.rs ===
  [x] test_delete_not_found
  [x] test_delete_creates_archive
  
[ COMPLETE ]
  === src/cmd/internal_run.rs ===
  [x] test_internal_run_unknown_profile
  
[ COMPLETE ]
  === src/cmd/list.rs ===
  [x] test_list_empty
  [x] test_list_is_json_array
  
[ COMPLETE ]
  === src/cmd/send.rs ===
  [x] test_fork_uses_active_profile_not_hardcoded_default
  [x] test_fork_errors_when_no_profile_available
  [x] test_send_writes_user_message_to_jsonl
  
[ COMPLETE ]
  === src/cmd/server_action.rs ===
  [x] test_build_body_empty
  [x] test_build_body_params
  [x] test_method_from_str
  [x] test_default_base_url
  
[ COMPLETE ]
  === src/cmd/set.rs ===
  [x] test_set_label
  [x] test_set_updates_metadata
  
[ COMPLETE ]
  === src/config/mod.rs ===
  [x] test_resolve_env
  [x] test_get_orchid_dir_orchid_dir_override
  [x] test_get_orchid_dir_xdg_config_home
  [x] test_get_orchid_dir_home_fallback
  [x] test_config_with_scope_exceptions
  [x] test_config_without_scope_exceptions_defaults_empty
  
[ COMPLETE ]
  === src/convo/id.rs ===
  [x] test_generate_id_format
  [x] test_generate_id_unique
  
[ COMPLETE ]
  === src/convo/mod.rs ===
  [x] test_create_conversation
  [x] test_get_conversation
  [x] test_list_conversations
  [x] test_update_conversation
  [x] test_atomic_write
  [x] test_create_with_scope_exceptions
  [x] test_update_scope_exceptions
  [x] test_deserialize_metadata_without_scope_exceptions
  
[ COMPLETE ]
  === src/convo/resolve.rs ===
  [x] test_is_id_format
  [x] test_resolve_rejects_non_id
  
[ COMPLETE ]
  === src/jsonerr/mod.rs ===
  [x] test_serialize
  [x] test_config_not_found
  
[ INCOMPLETE ]
  === src/lib.rs ===
  
[ COMPLETE ]
  === src/log/mod.rs ===
  [x] test_append_and_read
  
[ COMPLETE ]
  === src/loop/budget.rs ===
  [x] test_ok
  [x] test_warning
  [x] test_exceeded
  
[ COMPLETE ]
  === src/loop/events.rs ===
  [x] test_get_convo_jsonl_path
  
[ COMPLETE ]
  === src/loop/history.rs ===
  [x] test_build_empty_history
  [x] test_build_message_history
  [x] test_stale_read_replacement
  
[ COMPLETE ]
  === src/loop/lifecycle.rs ===
  [x] test_on_run_start
  [x] test_on_run_end
  
[ COMPLETE ]
  === src/loop/mod.rs ===
  [x] test_tool_error_returned_to_model_not_propagated
  [x] test_provider_error_leaves_convo_idle
  [x] test_persona_budget_override
  [x] test_persona_budget_warn_exceeds_hard_clamped
  [x] test_empty_response_continues_loop_instead_of_breaking
  [x] test_whitespace_only_message_triggers_retry
  [x] test_pre_send_budget_exceeded_does_not_call_provider
  
[ COMPLETE ]
  === src/provider/mod.rs ===
  [x] test_response_serialize
  [x] test_provider_error_display
  
[ COMPLETE ]
  === src/tools/bash.rs ===
  [x] test_execute_simple
  [x] test_execute_with_stderr
  [x] test_tokenize_simple
  [x] test_tokenize_quoted
  [x] test_is_builtin
  
[ COMPLETE ]
  === src/tools/fs_edit.rs ===
  [x] test_create_new_file
  [x] test_replace_single_legacy
  [x] test_batch_edits
  [x] test_batch_fail_fast
  [x] test_replace_multiple_error
  [x] test_replace_all
  [x] test_edit_out_of_scope
  
[ COMPLETE ]
  === src/tools/fs_read.rs ===
  [x] test_read_single_returns_json_object
  [x] test_read_batch_returns_json_object
  [x] test_read_batch_partial_error_is_structured
  [x] test_read_nonexistent_single_propagates_error
  [x] test_read_out_of_scope
  [x] test_extract_paths_paths_key
  [x] test_extract_paths_path_key
  
[ COMPLETE ]
  === src/tools/mod.rs ===
  [x] test_tool_definitions_count
  [x] test_execute_tool_bash
  [x] test_execute_tool_unknown
  [x] test_execute_tool_fs_read
  
[ COMPLETE ]
  === src/tools/scope.rs ===
  [x] test_expand_path_absolute
  [x] test_expand_path_relative
  [x] test_is_in_scope_tmp_when_working_dir_is_tmp
  [x] test_is_in_scope_tmp_outside_working_dir
  [x] test_is_in_scope_var_folders_outside_working_dir
  [x] test_expand_vars_simple
  [x] test_compile_exceptions_empty
  [x] test_compile_exceptions_matches
  [x] test_is_allowed_in_scope
  [x] test_is_allowed_global_exception
  [x] test_is_allowed_convo_exception
  [x] test_is_allowed_no_match_denied
  [x] test_compile_exceptions_tmp_pattern
