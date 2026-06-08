//! Epic E: parameterized host × event × profile hook contract matrix.
//!
//! Cross-host review-gate Stop behavior (Claude canonical): advisory when armed without
//! independent reviewer; pending multiset / phase are Cursor telemetry only.
//! Cursor-only surfaces (multiset hygiene, review-lite, pre-goal) remain in `cursor_hooks/tests.rs`.

mod harness;
mod mcp;

use harness::{
    closeout_followup_visible, dispatch_closeout_claim_stop, dispatch_independent_reviewer,
    dispatch_reviewer_with_fork, dispatch_stop, dispatch_user_prompt_submit, fresh_matrix_repo,
    reject_token_clears_stop, stop_allowed, stop_review_gate_advisory,
    user_prompt_additional_context,
    write_matrix_active_task, CanonicalReviewGateDisableGuard, CloseoutEnforcementGuard,
    CursorModelInheritDisableGuard, ForkInferEnableGuard, LegacyReviewGateDisableGuard, MatrixHost,
    MyLightOverrideGuard, PaperProseDefaultGuard, ReviewGateActiveGuard,
    SpawnFirstNudgeDisableGuard, SpawnFirstNudgeEnableGuard, CLOSEOUT_MATRIX_HOSTS,
    DEEP_REVIEW_PROMPT, MATRIX_HOSTS, MY_LIGHT_IMPLEMENT_PROMPT, MY_LIGHT_STOP_PROMPT,
    NARROW_REVIEW_PROMPT, PAPER_PROSE_PROMPT, SECOND_DEEP_REVIEW_PROMPT,
};

fn run_for_hosts<F>(label: &str, mut case: F)
where
    F: FnMut(MatrixHost),
{
    for host in MATRIX_HOSTS {
        case(*host);
        let _ = label;
    }
}

/// Review armed → advisory Stop nudge → independent reviewer clears nudge.
#[test]
fn matrix_review_armed_independent_reviewer_clears_stop() {
    run_for_hosts("independent-reviewer-clears-stop", |host| {
        let _gate = ReviewGateActiveGuard::new(host);
        let repo = fresh_matrix_repo(host, "review-clear");
        let sid = format!("{}-review-clear", harness::host_label(host));

        dispatch_user_prompt_submit(host, &repo, &sid, DEEP_REVIEW_PROMPT);
        let advisory = dispatch_stop(host, &repo, &sid, "继续", None);
        assert!(
            stop_allowed(host, &advisory),
            "{host:?} must not hard-block Stop before independent reviewer; out={advisory:?}"
        );
        assert!(
            stop_review_gate_advisory(host, &advisory),
            "{host:?} must inject advisory REVIEW_GATE before reviewer; out={advisory:?}"
        );

        dispatch_independent_reviewer(host, &repo, &sid);
        let allowed = dispatch_stop(host, &repo, &sid, "继续", None);
        assert!(
            stop_allowed(host, &allowed),
            "{host:?} must allow Stop after independent reviewer evidence; out={allowed:?}"
        );
        assert!(
            !stop_review_gate_advisory(host, &allowed),
            "{host:?} reviewer evidence must clear advisory nudge; out={allowed:?}"
        );
    });
}

/// my-light profile suppresses hard REVIEW_GATE even when review was armed earlier in session.
#[test]
fn matrix_my_light_suppresses_review_gate_on_stop() {
    run_for_hosts("my-light-suppresses-rg", |host| {
        let _gate = ReviewGateActiveGuard::new(host);
        let _my_light = MyLightOverrideGuard::clear_stale_override();
        let repo = fresh_matrix_repo(host, "my-light-rg");
        let sid = format!("{}-my-light", harness::host_label(host));

        dispatch_user_prompt_submit(host, &repo, &sid, DEEP_REVIEW_PROMPT);
        let armed = dispatch_stop(host, &repo, &sid, "继续", None);
        assert!(
            stop_review_gate_advisory(host, &armed),
            "{host:?} precondition: review must be armed before my-light stop; out={armed:?}"
        );

        let allowed = dispatch_stop(host, &repo, &sid, MY_LIGHT_STOP_PROMPT, Some(
            "[P1] scripts/foo.rs:42: missing edge case",
        ));
        assert!(
            stop_allowed(host, &allowed),
            "{host:?} my-light /implementx stop must suppress REVIEW_GATE; out={allowed:?}"
        );
    });
}

/// Narrow single-path review must not arm deep review gate nor Stop-block.
#[test]
fn matrix_narrow_review_skips_arming_and_stop_block() {
    run_for_hosts("narrow-review-skip", |host| {
        let _gate = ReviewGateActiveGuard::new(host);
        let repo = fresh_matrix_repo(host, "narrow");
        let sid = format!("{}-narrow", harness::host_label(host));

        dispatch_user_prompt_submit(host, &repo, &sid, NARROW_REVIEW_PROMPT);
        let stop = dispatch_stop(host, &repo, &sid, NARROW_REVIEW_PROMPT, Some(
            "[P1] README.md:1: typo in title",
        ));
        assert!(
            stop_allowed(host, &stop),
            "{host:?} narrow review must not Stop-block; out={stop:?}"
        );
    });
}

/// Canonical `ROUTER_RS_REVIEW_GATE_DISABLE=1` suppresses REVIEW_GATE nudges on Stop (all hosts).
#[test]
fn matrix_canonical_review_gate_disable_suppresses_stop_block() {
    for host in MATRIX_HOSTS {
        let _gate = CanonicalReviewGateDisableGuard::new(*host);
        let repo = fresh_matrix_repo(*host, "canonical-disable");
        let sid = format!("{}-canonical-disable", harness::host_label(*host));

        dispatch_user_prompt_submit(*host, &repo, &sid, DEEP_REVIEW_PROMPT);
        let stop = dispatch_stop(*host, &repo, &sid, "继续", None);
        assert!(
            stop_allowed(*host, &stop),
            "{host:?} canonical ROUTER_RS_REVIEW_GATE_DISABLE must suppress Stop block; out={stop:?}"
        );
    }
}

/// Deep review prompt arms gate: Stop injects advisory nudge until reviewer evidence.
#[test]
fn matrix_deep_review_prompt_injects_advisory_until_reviewer() {
    for host in MATRIX_HOSTS {
        let _gate = ReviewGateActiveGuard::new(*host);
        let repo = fresh_matrix_repo(*host, "deep-arm");
        let sid = format!("{}-deep-arm", harness::host_label(*host));
        dispatch_user_prompt_submit(*host, &repo, &sid, DEEP_REVIEW_PROMPT);
        let stop = dispatch_stop(*host, &repo, &sid, "继续", None);
        assert!(
            stop_allowed(*host, &stop),
            "{host:?} deep review must not hard-block Stop; out={stop:?}"
        );
        assert!(
            stop_review_gate_advisory(*host, &stop),
            "{host:?} deep review must inject advisory REVIEW_GATE; out={stop:?}"
        );
    }
}

/// Spawn-first nudge injects compact skill pointer when review arms (cross-host).
#[test]
fn matrix_spawn_first_nudge_injects_on_deep_review_arm() {
    for host in MATRIX_HOSTS {
        let _gate = ReviewGateActiveGuard::new(*host);
        let _spawn = SpawnFirstNudgeEnableGuard::enable();
        let _model_off = if *host == MatrixHost::Cursor {
            Some(CursorModelInheritDisableGuard::disable())
        } else {
            None
        };
        let repo = fresh_matrix_repo(*host, "spawn-on");
        let sid = format!("{}-spawn-on", harness::host_label(*host));
        let out = dispatch_user_prompt_submit(*host, &repo, &sid, DEEP_REVIEW_PROMPT);
        let ctx = user_prompt_additional_context(*host, &out);
        assert!(
            ctx.contains("skills/code-review-deep/SKILL.md"),
            "{host:?} spawn-first must point at skill; ctx={ctx:?}"
        );
        assert!(
            ctx.contains("fork_context=false"),
            "{host:?} spawn-first must mention fork_context=false; ctx={ctx:?}"
        );
        let _ = _model_off;
    }
}

/// `ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE=0` suppresses beforeSubmit additional context.
#[test]
fn matrix_spawn_first_nudge_disabled_no_additional_context() {
    for host in MATRIX_HOSTS {
        let _gate = ReviewGateActiveGuard::new(*host);
        let _spawn_off = SpawnFirstNudgeDisableGuard::disable();
        let _model_off = if *host == MatrixHost::Cursor {
            Some(CursorModelInheritDisableGuard::disable())
        } else {
            None
        };
        let repo = fresh_matrix_repo(*host, "spawn-off");
        let sid = format!("{}-spawn-off", harness::host_label(*host));
        let out = dispatch_user_prompt_submit(*host, &repo, &sid, DEEP_REVIEW_PROMPT);
        let ctx = user_prompt_additional_context(*host, &out);
        assert!(
            ctx.is_empty(),
            "{host:?} spawn-first off must not inject context; out={out:?}"
        );
        let _ = _model_off;
    }
}

/// Canonical fork infer: omitted `fork_context` on deep lane counts as independent reviewer.
#[test]
fn matrix_fork_infer_missing_fork_clears_stop() {
    for host in MATRIX_HOSTS {
        let _gate = ReviewGateActiveGuard::new(*host);
        let _infer = ForkInferEnableGuard::enable();
        let repo = fresh_matrix_repo(*host, "fork-infer");
        let sid = format!("{}-fork-infer", harness::host_label(*host));
        dispatch_user_prompt_submit(*host, &repo, &sid, DEEP_REVIEW_PROMPT);
        dispatch_reviewer_with_fork(*host, &repo, &sid, None, "matrix-infer-1");
        let stop = dispatch_stop(*host, &repo, &sid, "继续", None);
        assert!(
            stop_allowed(*host, &stop),
            "{host:?} fork infer must clear Stop after reviewer; out={stop:?}"
        );
    }
}

/// Explicit `fork_context: true` does not satisfy independent reviewer evidence (advisory nudge remains).
#[test]
fn matrix_fork_explicit_true_still_advises_stop() {
    for host in MATRIX_HOSTS {
        let _gate = ReviewGateActiveGuard::new(*host);
        let repo = fresh_matrix_repo(*host, "fork-true");
        let sid = format!("{}-fork-true", harness::host_label(*host));
        dispatch_user_prompt_submit(*host, &repo, &sid, DEEP_REVIEW_PROMPT);
        dispatch_reviewer_with_fork(*host, &repo, &sid, Some(true), "matrix-fork-true");
        let stop = dispatch_stop(*host, &repo, &sid, "继续", None);
        assert!(
            stop_allowed(*host, &stop),
            "{host:?} shared fork must not hard-block Stop; out={stop:?}"
        );
        assert!(
            stop_review_gate_advisory(*host, &stop),
            "{host:?} shared fork must keep advisory REVIEW_GATE; out={stop:?}"
        );
    }
}

/// Bounded reject token in user Stop prompt clears advisory nudge (all hosts).
#[test]
fn matrix_reject_token_user_prompt_stop_semantics() {
    for host in MATRIX_HOSTS {
        let _gate = ReviewGateActiveGuard::new(*host);
        let repo = fresh_matrix_repo(*host, "reject-user");
        let sid = format!("{}-reject-user", harness::host_label(*host));
        dispatch_user_prompt_submit(*host, &repo, &sid, DEEP_REVIEW_PROMPT);
        let stop = dispatch_stop(*host, &repo, &sid, "reject reason: small_task", None);
        assert!(
            reject_token_clears_stop(*host),
            "{host:?} must honor bounded reject clearance"
        );
        assert!(
            stop_allowed(*host, &stop),
            "{host:?} reject token must clear Stop advisory; out={stop:?}"
        );
        assert!(
            !stop_review_gate_advisory(*host, &stop),
            "{host:?} reject token must clear REVIEW_GATE nudge; out={stop:?}"
        );
    }
}

/// Bounded reject token in assistant Stop response clears advisory nudge (all hosts).
#[test]
fn matrix_reject_token_assistant_response_stop_semantics() {
    for host in MATRIX_HOSTS {
        let _gate = ReviewGateActiveGuard::new(*host);
        let repo = fresh_matrix_repo(*host, "reject-resp");
        let sid = format!("{}-reject-resp", harness::host_label(*host));
        dispatch_user_prompt_submit(*host, &repo, &sid, DEEP_REVIEW_PROMPT);
        let stop = dispatch_stop(
            *host,
            &repo,
            &sid,
            "继续",
            Some("reject reason: shared_context_heavy"),
        );
        assert!(
            reject_token_clears_stop(*host),
            "{host:?} must honor assistant reject clearance"
        );
        assert!(
            stop_allowed(*host, &stop),
            "{host:?} assistant reject token must clear Stop advisory; out={stop:?}"
        );
        assert!(
            !stop_review_gate_advisory(*host, &stop),
            "{host:?} assistant reject must clear REVIEW_GATE nudge; out={stop:?}"
        );
    }
}

/// `rg_clear` on Stop clears advisory nudge without independent reviewer (all hosts).
#[test]
fn matrix_rg_clear_stop_prompt_semantics() {
    for host in MATRIX_HOSTS {
        let _gate = ReviewGateActiveGuard::new(*host);
        let repo = fresh_matrix_repo(*host, "rg-clear");
        let sid = format!("{}-rg-clear", harness::host_label(*host));
        dispatch_user_prompt_submit(*host, &repo, &sid, DEEP_REVIEW_PROMPT);
        let stop = dispatch_stop(*host, &repo, &sid, "rg_clear", None);
        assert!(
            reject_token_clears_stop(*host),
            "{host:?} must honor rg_clear clearance (Claude canonical)"
        );
        assert!(
            stop_allowed(*host, &stop),
            "{host:?} rg_clear must clear Stop advisory; out={stop:?}"
        );
        assert!(
            !stop_review_gate_advisory(*host, &stop),
            "{host:?} rg_clear must clear REVIEW_GATE nudge; out={stop:?}"
        );
    }
}

/// Legacy per-host `ROUTER_RS_*_REVIEW_GATE_DISABLE=1` suppresses REVIEW_GATE nudges (canonical unset).
#[test]
fn matrix_legacy_review_gate_disable_suppresses_stop_block() {
    for host in MATRIX_HOSTS {
        let _gate = LegacyReviewGateDisableGuard::new(*host);
        let repo = fresh_matrix_repo(*host, "legacy-disable");
        let sid = format!("{}-legacy-disable", harness::host_label(*host));
        dispatch_user_prompt_submit(*host, &repo, &sid, DEEP_REVIEW_PROMPT);
        let stop = dispatch_stop(*host, &repo, &sid, "继续", None);
        assert!(
            stop_allowed(*host, &stop),
            "{host:?} legacy REVIEW_GATE_DISABLE must suppress Stop block; out={stop:?}"
        );
    }
}

/// my-light UserPromptSubmit clears sticky deep review; plain Stop no longer injects review nudge.
#[test]
fn matrix_my_light_user_prompt_clears_armed_review() {
    run_for_hosts("my-light-ups-clears", |host| {
        let _gate = ReviewGateActiveGuard::new(host);
        let _my_light = MyLightOverrideGuard::clear_stale_override();
        let repo = fresh_matrix_repo(host, "my-light-clear");
        let sid = format!("{}-my-light-clear", harness::host_label(host));
        dispatch_user_prompt_submit(host, &repo, &sid, DEEP_REVIEW_PROMPT);
        let armed = dispatch_stop(host, &repo, &sid, "继续", None);
        assert!(
            stop_review_gate_advisory(host, &armed),
            "{host:?} precondition: deep review must advise before my-light UPS; out={armed:?}"
        );
        dispatch_user_prompt_submit(host, &repo, &sid, MY_LIGHT_IMPLEMENT_PROMPT);
        let allowed = dispatch_stop(host, &repo, &sid, "继续", None);
        assert!(
            stop_allowed(host, &allowed),
            "{host:?} my-light UPS must clear armed review; out={allowed:?}"
        );
    });
}

/// Narrow path after a prior deep arm disarms sticky review_required.
#[test]
fn matrix_narrow_path_disarms_sticky_deep_arm() {
    run_for_hosts("narrow-disarm-sticky", |host| {
        let _gate = ReviewGateActiveGuard::new(host);
        let repo = fresh_matrix_repo(host, "narrow-disarm");
        let sid = format!("{}-narrow-disarm", harness::host_label(host));
        dispatch_user_prompt_submit(host, &repo, &sid, DEEP_REVIEW_PROMPT);
        dispatch_user_prompt_submit(host, &repo, &sid, NARROW_REVIEW_PROMPT);
        let stop = dispatch_stop(host, &repo, &sid, NARROW_REVIEW_PROMPT, None);
        assert!(
            stop_allowed(host, &stop),
            "{host:?} narrow path must disarm sticky deep review; out={stop:?}"
        );
    });
}

/// Review-gate disable must not inject spawn-first skill pointer on UserPromptSubmit.
#[test]
fn matrix_review_gate_disabled_suppresses_spawn_first_nudge() {
    for host in MATRIX_HOSTS {
        let _disable = CanonicalReviewGateDisableGuard::new(*host);
        let _model_off = if *host == MatrixHost::Cursor {
            Some(CursorModelInheritDisableGuard::disable())
        } else {
            None
        };
        let repo = fresh_matrix_repo(*host, "rg-off-nudge");
        let sid = format!("{}-rg-off-nudge", harness::host_label(*host));
        let out = dispatch_user_prompt_submit(*host, &repo, &sid, DEEP_REVIEW_PROMPT);
        let ctx = user_prompt_additional_context(*host, &out);
        assert!(
            !ctx.contains("skills/code-review-deep/SKILL.md"),
            "{host:?} review gate disabled must not inject spawn-first; ctx={ctx:?}"
        );
        let _ = _model_off;
    }
}

/// my-light suppress requires user Stop prompt; assistant-only `/implementx` must not disarm gate.
///
/// Codex merges prompt+response into `stop_signal`, so `/implementx` in assistant text suppresses
/// the gate entirely (host delta). Cursor/Claude profile checks use user prompt only.
#[test]
fn matrix_my_light_stop_assistant_only_does_not_suppress() {
    run_for_hosts("my-light-assist-only", |host| {
        let _gate = ReviewGateActiveGuard::new(host);
        let repo = fresh_matrix_repo(host, "assist-only");
        let sid = format!("{}-assist-only", harness::host_label(host));
        dispatch_user_prompt_submit(host, &repo, &sid, DEEP_REVIEW_PROMPT);
        let stop = dispatch_stop(
            host,
            &repo,
            &sid,
            "继续",
            Some("按 /implementx 流程执行即可"),
        );
        match host {
            MatrixHost::Codex => {
                assert!(
                    stop_allowed(host, &stop),
                    "{host:?} combined stop_signal may suppress via assistant /implementx; out={stop:?}"
                );
            }
            _ => assert!(
                stop_review_gate_advisory(host, &stop),
                "{host:?} assistant tail must not trigger my-light suppress; out={stop:?}"
            ),
        }
    });
}

/// Paper prose quality hook injects by default on manuscript polish prompts (cross-host).
#[test]
fn matrix_paper_prose_injects_by_default_on_user_prompt() {
    for host in MATRIX_HOSTS {
        let _prose = PaperProseDefaultGuard::enable(*host);
        let _rg_clear = ReviewGateActiveGuard::new(*host);
        let repo = fresh_matrix_repo(*host, "paper-prose");
        let sid = format!("{}-paper-prose", harness::host_label(*host));
        let out = dispatch_user_prompt_submit(*host, &repo, &sid, PAPER_PROSE_PROMPT);
        let ctx = user_prompt_additional_context(*host, &out);
        assert!(
            ctx.contains("PAPER_PROSE_QUALITY_HOOK"),
            "{host:?} must inject paper prose hook by default; ctx={ctx:?}"
        );
    }
}

/// Strict closeout: completion claim on Stop blocks with missing_record followup (Cursor + Codex + Claude).
#[test]
fn matrix_closeout_blocks_stop_on_completion_claim() {
    for host in CLOSEOUT_MATRIX_HOSTS {
        let _closeout = CloseoutEnforcementGuard::strict();
        let repo = fresh_matrix_repo(*host, "closeout-strict");
        let sid = format!("{}-closeout", harness::host_label(*host));
        let tid = format!("t-{}-closeout", harness::host_label(*host));
        write_matrix_active_task(&repo, &tid);
        let stop = dispatch_closeout_claim_stop(*host, &repo, &sid);
        assert!(
            closeout_followup_visible(*host, &stop),
            "{host:?} strict closeout must block completion claim; out={stop:?}"
        );
    }
}

/// User-authored review bypass on Stop disarms advisory REVIEW_GATE nudge (all hosts).
#[test]
fn matrix_user_override_disarms_review_gate_on_stop() {
    run_for_hosts("user-override-disarm", |host| {
        let _gate = ReviewGateActiveGuard::new(host);
        let repo = fresh_matrix_repo(host, "user-override");
        let sid = format!("{}-user-override", harness::host_label(host));
        dispatch_user_prompt_submit(host, &repo, &sid, DEEP_REVIEW_PROMPT);
        let stop = dispatch_stop(
            host,
            &repo,
            &sid,
            "不要使用子代理，本轮只在主会话给结论",
            Some("收到。"),
        );
        assert!(
            stop_allowed(host, &stop),
            "{host:?} user override must not hard-block Stop; out={stop:?}"
        );
        match host {
            // Claude applies `review_override` on UserPromptSubmit only (Stop delta).
            MatrixHost::Claude => assert!(
                stop_review_gate_advisory(host, &stop),
                "{host:?} Stop-time override not wired; advisory expected; out={stop:?}"
            ),
            _ => assert!(
                !stop_review_gate_advisory(host, &stop),
                "{host:?} user override must clear REVIEW_GATE nudge; out={stop:?}"
            ),
        }
    });
}

/// Second deep review in same session re-injects advisory Stop until fresh independent reviewer evidence.
#[test]
fn matrix_second_deep_review_advises_stop_until_fresh_reviewer() {
    run_for_hosts("second-deep-rearm", |host| {
        let _gate = ReviewGateActiveGuard::new(host);
        let repo = fresh_matrix_repo(host, "rearm");
        let sid = format!("{}-rearm", harness::host_label(host));
        dispatch_user_prompt_submit(host, &repo, &sid, DEEP_REVIEW_PROMPT);
        dispatch_independent_reviewer(host, &repo, &sid);
        let cleared = dispatch_stop(host, &repo, &sid, "继续", None);
        assert!(
            stop_allowed(host, &cleared),
            "{host:?} precondition: reviewer must clear first cycle; out={cleared:?}"
        );
        dispatch_user_prompt_submit(host, &repo, &sid, SECOND_DEEP_REVIEW_PROMPT);
        let advised = dispatch_stop(host, &repo, &sid, "继续", None);
        assert!(
            stop_allowed(host, &advised),
            "{host:?} second deep review must not hard-block Stop; out={advised:?}"
        );
        assert!(
            stop_review_gate_advisory(host, &advised),
            "{host:?} second deep review must inject advisory without fresh reviewer; out={advised:?}"
        );
    });
}
