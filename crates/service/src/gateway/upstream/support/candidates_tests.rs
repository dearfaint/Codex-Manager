use super::{
    allow_openai_fallback_for_account_with_snapshot, candidate_skip_reason_for_proxy,
    CandidateSkipReason,
};
use codexmanager_core::storage::{
    now_ts, Account, ModelSourceMapping, ModelSourceModel, Storage, Token,
};

fn insert_active_account_with_token(storage: &Storage, account_id: &str, sort: i64) {
    let now = now_ts();
    storage
        .insert_account(&Account {
            id: account_id.to_string(),
            label: account_id.to_string(),
            issuer: "issuer".to_string(),
            chatgpt_account_id: None,
            workspace_id: None,
            group_name: None,
            sort,
            status: "active".to_string(),
            created_at: now,
            updated_at: now,
        })
        .expect("insert account");
    storage
        .insert_token(&Token {
            account_id: account_id.to_string(),
            id_token: "header.payload.sig".to_string(),
            access_token: "header.payload.sig".to_string(),
            refresh_token: "refresh".to_string(),
            api_key_access_token: None,
            last_refresh: now,
        })
        .expect("insert token");
}

fn upsert_account_source_model(storage: &Storage, account_id: &str, upstream_model: &str) {
    let now = now_ts();
    storage
        .upsert_model_source_model(&ModelSourceModel {
            source_kind: "openai_account".to_string(),
            source_id: account_id.to_string(),
            upstream_model: upstream_model.to_string(),
            display_name: Some(upstream_model.to_string()),
            status: "available".to_string(),
            discovery_kind: "manual".to_string(),
            last_synced_at: Some(now),
            extra_json: "{}".to_string(),
            created_at: now,
            updated_at: now,
        })
        .expect("upsert account source model");
}

fn insert_active_grouped_account_with_token(
    storage: &Storage,
    account_id: &str,
    group_name: &str,
    sort: i64,
) {
    let now = now_ts();
    storage
        .insert_account(&Account {
            id: account_id.to_string(),
            label: account_id.to_string(),
            issuer: "issuer".to_string(),
            chatgpt_account_id: None,
            workspace_id: None,
            group_name: Some(group_name.to_string()),
            sort,
            status: "active".to_string(),
            created_at: now,
            updated_at: now,
        })
        .expect("insert account");
    storage
        .insert_token(&Token {
            account_id: account_id.to_string(),
            id_token: "header.payload.sig".to_string(),
            access_token: "header.payload.sig".to_string(),
            refresh_token: "refresh".to_string(),
            api_key_access_token: None,
            last_refresh: now,
        })
        .expect("insert token");
}

#[test]
fn prepare_gateway_candidates_accepts_direct_upstream_model_without_platform_mapping() {
    let _guard = crate::test_env_guard();
    let storage = Storage::open_in_memory().expect("open");
    storage.init().expect("init");
    let now = now_ts();
    storage
        .insert_account(&Account {
            id: "acc-direct-upstream".to_string(),
            label: "acc-direct-upstream".to_string(),
            issuer: "issuer".to_string(),
            chatgpt_account_id: None,
            workspace_id: None,
            group_name: None,
            sort: 0,
            status: "active".to_string(),
            created_at: now,
            updated_at: now,
        })
        .expect("insert account");
    storage
        .insert_token(&Token {
            account_id: "acc-direct-upstream".to_string(),
            id_token: "header.payload.sig".to_string(),
            access_token: "header.payload.sig".to_string(),
            refresh_token: "refresh".to_string(),
            api_key_access_token: None,
            last_refresh: now,
        })
        .expect("insert token");
    storage
        .upsert_model_source_model(&ModelSourceModel {
            source_kind: "openai_account".to_string(),
            source_id: "acc-direct-upstream".to_string(),
            upstream_model: "gpt-5.4-mini".to_string(),
            display_name: Some("gpt-5.4-mini".to_string()),
            status: "available".to_string(),
            discovery_kind: "manual".to_string(),
            last_synced_at: Some(now),
            extra_json: "{}".to_string(),
            created_at: now,
            updated_at: now,
        })
        .expect("upsert source model");

    let candidates = super::prepare_gateway_candidates(
        &storage,
        Some("gpt-5.4-mini"),
        None,
        None,
        crate::gateway::LowQuotaCandidateMode::NormalOnly,
    )
    .expect("prepare candidates");

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].0.id, "acc-direct-upstream");
}

#[test]
fn prepare_gateway_candidates_filters_account_group() {
    let _guard = crate::test_env_guard();
    let storage = Storage::open_in_memory().expect("open");
    storage.init().expect("init");
    insert_active_grouped_account_with_token(&storage, "acc-team-a", "team-a", 0);
    insert_active_grouped_account_with_token(&storage, "acc-team-b", "team-b", 1);

    let candidates = super::prepare_gateway_candidates(
        &storage,
        None,
        None,
        Some("team-a"),
        crate::gateway::LowQuotaCandidateMode::NormalOnly,
    )
    .expect("prepare candidates");

    assert_eq!(
        candidates
            .into_iter()
            .map(|(account, _token)| account.id)
            .collect::<Vec<_>>(),
        vec!["acc-team-a".to_string()]
    );
}

#[test]
fn prepare_gateway_candidates_uses_direct_source_model_even_when_aggregate_mapping_exists() {
    let _guard = crate::test_env_guard();
    let storage = Storage::open_in_memory().expect("open");
    storage.init().expect("init");
    let now = now_ts();
    insert_active_account_with_token(&storage, "acc-aggregate-owned", 0);
    storage
        .upsert_model_source_model(&ModelSourceModel {
            source_kind: "openai_account".to_string(),
            source_id: "acc-aggregate-owned".to_string(),
            upstream_model: "gpt-aggregate-owned".to_string(),
            display_name: Some("gpt-aggregate-owned".to_string()),
            status: "available".to_string(),
            discovery_kind: "manual".to_string(),
            last_synced_at: Some(now),
            extra_json: "{}".to_string(),
            created_at: now,
            updated_at: now,
        })
        .expect("upsert account source model");
    storage
        .upsert_model_source_mapping(&ModelSourceMapping {
            id: "map-aggregate-owned".to_string(),
            platform_model_slug: "gpt-aggregate-owned".to_string(),
            source_kind: "aggregate_api".to_string(),
            source_id: "agg-owned".to_string(),
            upstream_model: "gpt-aggregate-owned".to_string(),
            enabled: true,
            priority: 0,
            weight: 1,
            billing_model_slug: None,
            created_at: now,
            updated_at: now,
        })
        .expect("upsert aggregate mapping");

    let candidates = super::prepare_gateway_candidates(
        &storage,
        Some("gpt-aggregate-owned"),
        None,
        None,
        crate::gateway::LowQuotaCandidateMode::NormalOnly,
    )
    .expect("prepare candidates");

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].0.id, "acc-aggregate-owned");
}

#[test]
fn prepare_gateway_candidates_keeps_explicit_account_mapping_with_aggregate_mapping() {
    let _guard = crate::test_env_guard();
    let storage = Storage::open_in_memory().expect("open");
    storage.init().expect("init");
    let now = now_ts();
    insert_active_account_with_token(&storage, "acc-explicit-route", 0);
    insert_active_account_with_token(&storage, "acc-other-route", 1);
    upsert_account_source_model(&storage, "acc-explicit-route", "gpt-hybrid-route");
    storage
        .upsert_model_source_mapping(&ModelSourceMapping {
            id: "map-explicit-account".to_string(),
            platform_model_slug: "gpt-hybrid-route".to_string(),
            source_kind: "openai_account".to_string(),
            source_id: "acc-explicit-route".to_string(),
            upstream_model: "gpt-hybrid-route".to_string(),
            enabled: true,
            priority: 2,
            weight: 1,
            billing_model_slug: None,
            created_at: now,
            updated_at: now,
        })
        .expect("upsert account mapping");
    storage
        .upsert_model_source_mapping(&ModelSourceMapping {
            id: "map-hybrid-aggregate".to_string(),
            platform_model_slug: "gpt-hybrid-route".to_string(),
            source_kind: "aggregate_api".to_string(),
            source_id: "agg-hybrid".to_string(),
            upstream_model: "gpt-hybrid-route".to_string(),
            enabled: true,
            priority: 1,
            weight: 1,
            billing_model_slug: None,
            created_at: now,
            updated_at: now,
        })
        .expect("upsert aggregate mapping");

    let candidates = super::prepare_gateway_candidates(
        &storage,
        Some("gpt-hybrid-route"),
        None,
        None,
        crate::gateway::LowQuotaCandidateMode::NormalOnly,
    )
    .expect("prepare candidates");

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].0.id, "acc-explicit-route");
}

#[test]
fn prepare_gateway_candidates_with_account_mapping_bypasses_global_candidate_cache() {
    let _guard = crate::test_env_guard();
    let storage = Storage::open_in_memory().expect("open");
    storage.init().expect("init");
    let now = now_ts();
    insert_active_account_with_token(&storage, "acc-cached-other", 0);
    insert_active_account_with_token(&storage, "acc-mapped-only", 1);
    upsert_account_source_model(&storage, "acc-mapped-only", "gpt-scoped-route");
    storage
        .upsert_model_source_mapping(&ModelSourceMapping {
            id: "map-scoped-account".to_string(),
            platform_model_slug: "gpt-scoped-route".to_string(),
            source_kind: "openai_account".to_string(),
            source_id: "acc-mapped-only".to_string(),
            upstream_model: "gpt-scoped-route".to_string(),
            enabled: true,
            priority: 0,
            weight: 1,
            billing_model_slug: None,
            created_at: now,
            updated_at: now,
        })
        .expect("upsert account mapping");

    let all_candidates =
        super::super::super::super::collect_gateway_candidates_with_low_quota_mode(
            &storage,
            crate::gateway::LowQuotaCandidateMode::NormalOnly,
        )
        .expect("warm global cache");
    assert_eq!(all_candidates.len(), 2);

    let candidates = super::prepare_gateway_candidates(
        &storage,
        Some("gpt-scoped-route"),
        None,
        None,
        crate::gateway::LowQuotaCandidateMode::NormalOnly,
    )
    .expect("prepare scoped candidates");

    assert_eq!(
        candidates
            .into_iter()
            .map(|(account, _token)| account.id)
            .collect::<Vec<_>>(),
        vec!["acc-mapped-only".to_string()]
    );
}

#[test]
fn prepare_gateway_candidates_ignores_mapping_to_different_upstream_model() {
    let _guard = crate::test_env_guard();
    let storage = Storage::open_in_memory().expect("open");
    storage.init().expect("init");
    let now = now_ts();
    insert_active_account_with_token(&storage, "acc-review-only", 0);
    upsert_account_source_model(&storage, "acc-review-only", "codex-auto-review");
    storage
        .upsert_model_source_mapping(&ModelSourceMapping {
            id: "map-review-only".to_string(),
            platform_model_slug: "gpt-5.5".to_string(),
            source_kind: "openai_account".to_string(),
            source_id: "acc-review-only".to_string(),
            upstream_model: "codex-auto-review".to_string(),
            enabled: true,
            priority: 0,
            weight: 1,
            billing_model_slug: None,
            created_at: now,
            updated_at: now,
        })
        .expect("upsert account mapping");

    let candidates = super::prepare_gateway_candidates(
        &storage,
        Some("gpt-5.5"),
        None,
        None,
        crate::gateway::LowQuotaCandidateMode::NormalOnly,
    )
    .expect("prepare candidates");

    assert!(candidates.is_empty());
}

/// 函数 `allow_openai_fallback_for_account_accepts_individual_plan_tiers`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-03
///
/// # 参数
/// 无
///
/// # 返回
/// 无
#[test]
fn allow_openai_fallback_for_account_accepts_individual_plan_tiers() {
    let _guard = crate::test_env_guard();
    let storage = Storage::open_in_memory().expect("open");
    storage.init().expect("init");
    let now = now_ts();
    let account = Account {
        id: "acc-pro".to_string(),
        label: "acc-pro".to_string(),
        issuer: "issuer".to_string(),
        chatgpt_account_id: Some("org-pro".to_string()),
        workspace_id: Some("org-pro".to_string()),
        group_name: None,
        sort: 0,
        status: "active".to_string(),
        created_at: now,
        updated_at: now,
    };
    storage.insert_account(&account).expect("insert account");
    let token = Token {
        account_id: "acc-pro".to_string(),
        id_token: "header.payload.sig".to_string(),
        access_token: {
            let header = "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0";
            let payload = "eyJzdWIiOiJhY2MtcHJvIiwiaHR0cHM6Ly9hcGkub3BlbmFpLmNvbS9hdXRoIjp7ImNoYXRncHRfcGxhbl90eXBlIjoicHJvIn19";
            format!("{header}.{payload}.sig")
        },
        refresh_token: "refresh".to_string(),
        api_key_access_token: None,
        last_refresh: now,
    };

    assert!(allow_openai_fallback_for_account_with_snapshot(
        &token, None
    ));
}

/// 函数 `allow_openai_fallback_for_account_rejects_workspace_plans`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-03
///
/// # 参数
/// 无
///
/// # 返回
/// 无
#[test]
fn allow_openai_fallback_for_account_rejects_workspace_plans() {
    let _guard = crate::test_env_guard();
    let storage = Storage::open_in_memory().expect("open");
    storage.init().expect("init");
    let now = now_ts();
    let account = Account {
        id: "acc-team".to_string(),
        label: "acc-team".to_string(),
        issuer: "issuer".to_string(),
        chatgpt_account_id: Some("org-team".to_string()),
        workspace_id: Some("org-team".to_string()),
        group_name: Some("team".to_string()),
        sort: 0,
        status: "active".to_string(),
        created_at: now,
        updated_at: now,
    };
    storage.insert_account(&account).expect("insert account");
    let token = Token {
        account_id: "acc-team".to_string(),
        id_token: "header.payload.sig".to_string(),
        access_token: {
            let header = "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0";
            let payload = "eyJzdWIiOiJhY2MtdGVhbSIsImh0dHBzOi8vYXBpLm9wZW5haS5jb20vYXV0aCI6eyJjaGF0Z3B0X3BsYW5fdHlwZSI6InRlYW0ifX0";
            format!("{header}.{payload}.sig")
        },
        refresh_token: "refresh".to_string(),
        api_key_access_token: None,
        last_refresh: now,
    };

    assert!(!allow_openai_fallback_for_account_with_snapshot(
        &token, None
    ));
}

#[test]
fn candidate_skip_reason_for_proxy_allows_failover_when_head_account_is_inflight_limited() {
    let _guard = crate::gateway::acquire_account_inflight("acc-preferred");
    let actual = candidate_skip_reason_for_proxy("acc-preferred", 0, 2, 1, false);
    assert_eq!(actual, Some(CandidateSkipReason::Inflight));
}

#[test]
fn candidate_skip_reason_for_proxy_can_skip_last_cooldown_candidate() {
    let account_id = "acc-cooldown-last-skip-test";
    crate::gateway::gateway_mark_account_cooldown_for_status(account_id, 403);

    let default_last = candidate_skip_reason_for_proxy(account_id, 0, 1, 0, false);
    let strict_last = candidate_skip_reason_for_proxy(account_id, 0, 1, 0, true);

    assert_eq!(default_last, None);
    assert_eq!(strict_last, Some(CandidateSkipReason::Cooldown));
}
