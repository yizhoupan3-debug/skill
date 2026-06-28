//! Property-based tests for core-state invariants.
//!
//! These tests verify that pure functions in the state machine have the expected
//! mathematical properties across a wide range of inputs, and that state
//! transitions obey documented rules.

#[cfg(test)]
pub(crate) mod proptests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use proptest::prelude::*;
    use serde_json::{Value, json};

    use crate::state_manager::goal_ops::{
        count_nonempty_string_items, validate_drive_contract, value_string_list,
    };
    use crate::state_manager::{
        evidence_index_entry_implies_success, goal_state_requests_continuation,
    };

    // ── Strategy helpers ────────────────────────────────────────────────

    fn arb_json_value() -> impl Strategy<Value = Value> {
        // Non-recursive: flat JSON values for robustness testing.
        // Recursion is avoided because impl Trait can't be used recursively in stable Rust.
        prop_oneof![
            any::<bool>().prop_map(Value::Bool),
            any::<i64>().prop_map(|n| json!(n)),
            ".*".prop_map(Value::String),
            prop::collection::vec("[a-z]{0,3}", 0..3)
                .prop_map(|v| { Value::Array(v.into_iter().map(Value::String).collect()) }),
            prop::collection::hash_map(
                "[a-z]{0,3}",
                prop_oneof![
                    any::<bool>().prop_map(Value::Bool),
                    any::<i64>().prop_map(|n| json!(n)),
                    ".*".prop_map(Value::String),
                    Just(Value::Null),
                ],
                0..3,
            )
            .prop_map(|m| { Value::Object(m.into_iter().collect()) }),
            Just(Value::Null),
        ]
    }

    fn arb_nonempty_string() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9_]{1,10}"
    }

    fn arb_value_list(min: usize, max: usize) -> impl Strategy<Value = Vec<Value>> {
        prop::collection::vec(
            prop_oneof![
                arb_nonempty_string().prop_map(Value::String),
                Just(Value::String(String::new())),
                Just(Value::String("   ".to_string())),
                Just(Value::Null),
                any::<i64>().prop_map(|n| json!(n)),
            ],
            min..=max,
        )
    }

    fn arb_nonempty_value_list(min: usize, max: usize) -> impl Strategy<Value = Vec<Value>> {
        prop::collection::vec(arb_nonempty_string().prop_map(Value::String), min..=max)
    }

    fn arb_goal_state() -> impl Strategy<Value = Value> {
        (
            any::<bool>(),
            any::<bool>(),
            prop_oneof![
                Just("running"),
                Just("paused"),
                Just("completed"),
                Just("blocked"),
                Just("superseded")
            ],
            prop_oneof![Just(Value::Null), any::<bool>().prop_map(Value::Bool),],
        )
            .prop_map(|(stale, drive, status, session_id)| {
                json!({
                    "stale": stale,
                    "drive_until_done": drive,
                    "status": status,
                    "session_id": session_id,
                })
            })
    }

    // ── Group 1: Pure function invariants ───────────────────────────────

    proptest! {
        /// I1: validate_drive_contract truth table.
        /// When drive_until_done=false → always Ok.
        #[test]
        fn prop_validate_drive_contract_disabled(
            non_goals in arb_value_list(0, 5),
            done_when in arb_value_list(0, 5),
            validation_commands in arb_value_list(0, 5),
        ) {
            let result = validate_drive_contract(
                false, &non_goals, &done_when, &validation_commands, "test"
            );
            prop_assert!(result.is_ok(), "drive_until_done=false should always be Ok");
        }

        /// I1b: When drive_until_done=true and contract is insufficient → Err.
        #[test]
        fn prop_validate_drive_contract_insufficient(
            non_goals in arb_value_list(0, 0),
            done_when in arb_value_list(0, 1),
            validation_commands in arb_value_list(0, 0),
        ) {
            let result = validate_drive_contract(
                true, &non_goals, &done_when, &validation_commands, "test"
            );
            prop_assert!(result.is_err(), "insufficient contract should Err");
        }

        /// I1c: When drive_until_done=true and all requirements met → Ok.
        #[test]
        fn prop_validate_drive_contract_sufficient(
            non_goals in arb_nonempty_value_list(1, 5),
            done_when in arb_nonempty_value_list(2, 5),
            validation_commands in arb_nonempty_value_list(1, 5),
        ) {
            let result = validate_drive_contract(
                true, &non_goals, &done_when, &validation_commands, "test"
            );
            prop_assert!(result.is_ok(), "sufficient contract should Ok");
        }

        /// I2: goal_state_requests_continuation — stale goals never request continuation.
        #[test]
        fn prop_goal_stale_never_requests_continuation(
            drive in any::<bool>(),
            status in prop_oneof![Just("running"), Just("paused"), Just("completed")],
        ) {
            let state = json!({
                "stale": true,
                "drive_until_done": drive,
                "status": status,
            });
            prop_assert!(!goal_state_requests_continuation(&state));
        }

        /// I2b: goal_state_requests_continuation — only status=running (loop semantics, GoalType::Linear removed).
        #[test]
        fn prop_goal_continuation_truth_table(
            drive in any::<bool>(),
            status in prop_oneof![
                Just("running"), Just("paused"), Just("completed"),
                Just("blocked"), Just("superseded"),
            ],
        ) {
            let state = json!({
                "stale": false,
                "drive_until_done": drive,
                "status": status,
            });
            let expected = status == "running";
            prop_assert_eq!(goal_state_requests_continuation(&state), expected);
        }


        /// I4: evidence_index_entry_implies_success — only success=true or exit_code=0.
        #[test]
        fn prop_evidence_success_no_false_positives(
            success in proptest::option::of(any::<bool>()),
            exit_code in proptest::option::of(any::<i64>()),
        ) {
            let mut map = serde_json::Map::new();
            if let Some(s) = success {
                map.insert("success".into(), Value::Bool(s));
            }
            if let Some(ec) = exit_code {
                // Only include exit_code 0 or 1 to keep signal clear
                map.insert("exit_code".into(), json!(if ec == 0 { 0 } else { 1 }));
            }
            let entry = Value::Object(map);
            let result = evidence_index_entry_implies_success(&entry);
            let expected = success == Some(true) || exit_code == Some(0);
            prop_assert_eq!(result, expected);
        }

        /// I5: count_nonempty_string_items is monotonic (appending never decreases count).
        #[test]
        fn prop_count_nonempty_monotonic(
            prefix in arb_value_list(0, 5),
            suffix in arb_value_list(0, 3),
        ) {
            let mut combined = prefix.clone();
            combined.extend(suffix);
            let c1 = count_nonempty_string_items(&prefix);
            let c2 = count_nonempty_string_items(&combined);
            prop_assert!(c2 >= c1, "appending items never decreases count");
        }
    }

    // ── Group 2: JSON robustness ────────────────────────────────────────

    proptest! {
        /// Verify goal_state_requests_continuation never panics on arbitrary JSON.
        #[test]
        fn prop_goal_requests_no_panic(state in arb_json_value()) {
            // Must not panic for any JSON input
            let _ = goal_state_requests_continuation(&state);
        }

        /// Verify evidence_index_entry_implies_success never panics on arbitrary JSON.
        #[test]
        fn prop_evidence_success_no_panic(entry in arb_json_value()) {
            let _ = evidence_index_entry_implies_success(&entry);
        }

        /// Verify value_string_list never panics on arbitrary JSON.
        #[test]
        fn prop_value_string_list_no_panic(
            payload in arb_json_value(),
            key in "[a-z]{0,10}",
        ) {
            let _ = value_string_list(&payload, &key);
        }
    }

    // ── Group 3: State transition invariants ────────────────────────────

    /// Simulated GOAL_STATE machine state for in-memory property testing.
    #[derive(Debug, Clone, PartialEq)]
    enum GoalStatus {
        Running,
        Paused,
        Blocked,
        Completed,
        Superseded,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct SimGoal {
        status: GoalStatus,
        drive_until_done: bool,
        checkpoint_count: usize,
        archived: bool,
    }

    #[derive(Debug, Clone)]
    enum Op {
        Start,
        Pause,
        Resume,
        Checkpoint,
        Block,
        Unblock,
        Amend,
        Complete,
        Clear,
    }

    fn apply_op(state: Option<SimGoal>, op: &Op) -> Option<SimGoal> {
        match op {
            Op::Start => {
                // Start always creates or resets a goal
                Some(SimGoal {
                    status: GoalStatus::Running,
                    drive_until_done: true,
                    checkpoint_count: 0,
                    archived: false,
                })
            }
            Op::Pause => match state {
                Some(s) if s.status == GoalStatus::Running => Some(SimGoal {
                    status: GoalStatus::Paused,
                    drive_until_done: false,
                    ..s
                }),
                _ => state,
            },
            Op::Resume => match state {
                Some(s) if !s.archived && s.status != GoalStatus::Completed => Some(SimGoal {
                    status: GoalStatus::Running,
                    drive_until_done: true,
                    ..s
                }),
                _ => state,
            },
            Op::Checkpoint => match state {
                Some(s) if s.status == GoalStatus::Running => Some(SimGoal {
                    checkpoint_count: s.checkpoint_count + 1,
                    ..s
                }),
                _ => state,
            },
            Op::Block => match state {
                Some(s) if s.status == GoalStatus::Running => Some(SimGoal {
                    status: GoalStatus::Blocked,
                    ..s
                }),
                _ => state,
            },
            Op::Unblock => match state {
                Some(s) if s.status == GoalStatus::Blocked => Some(SimGoal {
                    status: GoalStatus::Running,
                    drive_until_done: true,
                    ..s
                }),
                _ => state,
            },
            Op::Amend => match state {
                Some(ref s)
                    if s.archived
                        || s.status == GoalStatus::Completed
                        || s.status == GoalStatus::Superseded =>
                {
                    state
                }
                Some(s) => Some(s), // amend preserves state in simulation (actual changes immaterial here)
                None => state,
            },
            Op::Complete => match state {
                Some(s) if !s.archived => Some(SimGoal {
                    status: GoalStatus::Completed,
                    archived: true,
                    ..s
                }),
                _ => state,
            },
            Op::Clear => None,
        }
    }

    fn op_sequence() -> impl Strategy<Value = Vec<Op>> {
        prop::collection::vec(
            prop_oneof![
                Just(Op::Start),
                Just(Op::Pause),
                Just(Op::Resume),
                Just(Op::Checkpoint),
                Just(Op::Block),
                Just(Op::Unblock),
                Just(Op::Amend),
                Just(Op::Complete),
                Just(Op::Clear),
            ],
            0..=30,
        )
    }

    proptest! {
        #[test]
        fn prop_state_machine_transition_invariants(ops in op_sequence()) {
            let mut state: Option<SimGoal> = None;
            for op in &ops {
                let before = state.clone();
                state = apply_op(state, op);

                // Invariant 1: Completed goals can't be checkpointed or resumed
                if let Some(ref s) = before {
                    if s.archived {
                        match op {
                            Op::Checkpoint | Op::Resume => {
                                prop_assert_eq!(&state, &before, "archived/completed goal rejected {}", stringify!(op));
                            }
                            _ => {}
                        }
                    }
                }

                // Invariant 2: Pause sets drive_until_done=false when goal was Running.
                if let Op::Pause = op {
                    if before.as_ref().is_some_and(|s| s.status == GoalStatus::Running) {
                        if let Some(ref s) = state {
                            prop_assert!(matches!(s.status, GoalStatus::Paused));
                            prop_assert!(!s.drive_until_done, "pause sets drive_until_done=false");
                        }
                    }
                }

                // Invariant 3: Resume restores running+drive when goal was Paused.
                if let Op::Resume = op {
                    if before.as_ref().is_some_and(|s| s.status == GoalStatus::Paused) {
                        if let Some(ref s) = state {
                            prop_assert!(matches!(s.status, GoalStatus::Running));
                            prop_assert!(s.drive_until_done, "resume sets drive_until_done=true");
                        }
                    }
                }

                // Invariant 4: Checkpoint increments count when goal was Running.
                if let Op::Checkpoint = op {
                    if before.as_ref().is_some_and(|s| s.status == GoalStatus::Running) {
                        if let (Some(ref s), Some(ref b)) = (state.as_ref(), before.as_ref()) {
                            prop_assert_eq!(s.checkpoint_count, b.checkpoint_count + 1,
                                "checkpoint increments count");
                        }
                    }
                }

                // Invariant 5: Clear removes state entirely.
                if let Op::Clear = op {
                    prop_assert!(state.is_none(), "clear removes state");
                }

                // Invariant 6: Unblock restores running+drive when goal was Blocked.
                if let Op::Unblock = op {
                    if before.as_ref().is_some_and(|s| s.status == GoalStatus::Blocked) {
                        if let Some(ref s) = state {
                            prop_assert!(matches!(s.status, GoalStatus::Running));
                            prop_assert!(s.drive_until_done, "unblocked goal has drive_until_done=true");
                        }
                    }
                }
            }
        }
    }

    // ── Group 4: GOAL_STATE schema invariants ───────────────────────────

    proptest! {
        #[test]
        fn prop_goal_state_has_required_fields(state in arb_goal_state()) {
            // Verify the goal state object always has the schema fields we expect
            if let Value::Object(ref m) = state {
                prop_assert!(m.contains_key("stale"));
                prop_assert!(m.contains_key("drive_until_done"));
                prop_assert!(m.contains_key("status"));
            }
        }
    }
}
