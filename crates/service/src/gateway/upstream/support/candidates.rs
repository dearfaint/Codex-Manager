use codexmanager_core::storage::{Account, Storage, Token};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in super::super) enum CandidateSkipReason {
    Cooldown,
    Inflight,
}

fn account_source_ids_for_model(storage: &Storage, model: &str) -> Result<HashSet<String>, String> {
    let mut account_source_ids: HashSet<String> = storage
        .list_enabled_model_source_mapping_source_ids_for_platform_and_kind(model, "openai_account")
        .map_err(|err| format!("list model source mapping source ids failed: {err}"))?
        .into_iter()
        .collect();
    if account_source_ids.is_empty()
        && !storage
            .has_enabled_model_source_mapping_for_platform_and_kind(model, "aggregate_api")
            .map_err(|err| format!("check aggregate model source mappings failed: {err}"))?
    {
        account_source_ids.extend(
            storage
                .list_available_source_model_ids_by_upstream_model("openai_account", model)
                .map_err(|err| format!("list source models by upstream model failed: {err}"))?,
        );
    }
    Ok(account_source_ids)
}

/// 函数 `prepare_gateway_candidates`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - crate: 参数 crate
///
/// # 返回
/// 返回函数执行结果
pub(crate) fn prepare_gateway_candidates(
    storage: &Storage,
    request_model: Option<&str>,
    account_plan_filter: Option<&str>,
    low_quota_mode: super::super::super::LowQuotaCandidateMode,
) -> Result<Vec<(Account, Token)>, String> {
    let normalized_model = request_model
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let account_source_ids = if let Some(model) = normalized_model {
        let _ = crate::apikey_models::bootstrap_account_pool_model_routes(storage, false);
        let source_ids = account_source_ids_for_model(storage, model)?;
        Some(source_ids.into_iter().collect::<Vec<_>>())
    } else {
        None
    };
    // 中文注释：保持账号原始顺序（按账户排序字段）作为候选顺序，失败时再依次切下一个。
    let mut candidates = if let Some(account_source_ids) = account_source_ids.as_deref() {
        super::super::super::collect_gateway_candidates_for_accounts_with_low_quota_mode(
            storage,
            account_source_ids,
            low_quota_mode,
        )?
    } else {
        super::super::super::collect_gateway_candidates_with_low_quota_mode(
            storage,
            low_quota_mode,
        )?
    };
    let normalized_filter = account_plan_filter
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("all"));
    if let Some(plan_filter) = normalized_filter {
        let account_ids = candidates
            .iter()
            .map(|(account, _)| account.id.clone())
            .collect::<Vec<_>>();
        let snapshots = storage
            .latest_usage_snapshots_for_accounts(&account_ids)
            .map_err(|err| format!("list account usage snapshots failed: {err}"))?
            .into_iter()
            .map(|snapshot| (snapshot.account_id.clone(), snapshot))
            .collect::<HashMap<_, _>>();
        candidates.retain(|(account, token)| {
            crate::account_plan::account_matches_plan_filter_with_snapshot(
                token,
                snapshots.get(account.id.as_str()),
                Some(plan_filter),
            )
        });
    }
    Ok(candidates)
}

/// 函数 `free_account_model_override`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - in super: 参数 in super
///
/// # 返回
/// 返回函数执行结果
pub(in super::super) fn free_account_model_override(
    storage: &Storage,
    account: &Account,
    token: &Token,
) -> Option<String> {
    if !crate::account_plan::is_free_or_single_window_account(storage, account.id.as_str(), token) {
        return None;
    }
    let configured = super::super::super::current_free_account_max_model();
    if configured.eq_ignore_ascii_case("auto") {
        None
    } else {
        Some(configured)
    }
}

/// 函数 `allow_openai_fallback_for_account`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-03
///
/// # 参数
/// - storage: 参数 storage
/// - account: 参数 account
/// - token: 参数 token
///
/// # 返回
/// 返回函数执行结果
pub(in super::super) fn allow_openai_fallback_for_account(
    storage: &Storage,
    account: &Account,
    token: &Token,
) -> bool {
    if let Some(plan) = crate::account_plan::resolve_token_account_plan(token) {
        return matches!(plan.normalized.as_str(), "free" | "go" | "plus" | "pro");
    }

    let snapshot = storage
        .latest_usage_snapshot_for_account(account.id.as_str())
        .ok()
        .flatten();
    let token_plan = crate::account_plan::token_plan_from_token(token);
    let Some(plan) =
        crate::account_plan::resolve_account_plan(Some(&token_plan), snapshot.as_ref())
    else {
        return false;
    };
    matches!(plan.normalized.as_str(), "free" | "go" | "plus" | "pro")
}

/// 函数 `candidate_skip_reason_for_proxy`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - in super: 参数 in super
///
/// # 返回
/// 返回函数执行结果
pub(in super::super) fn candidate_skip_reason_for_proxy(
    account_id: &str,
    idx: usize,
    candidate_count: usize,
    account_max_inflight: usize,
    skip_last_cooldown: bool,
) -> Option<CandidateSkipReason> {
    let has_more_candidates = idx + 1 < candidate_count;
    if super::super::super::is_account_in_cooldown(account_id)
        && (has_more_candidates || skip_last_cooldown)
    {
        super::super::super::record_gateway_candidate_skip(
            super::super::super::GatewayCandidateSkipReason::Cooldown,
        );
        return Some(CandidateSkipReason::Cooldown);
    }

    if account_max_inflight > 0
        && super::super::super::account_inflight_count(account_id) >= account_max_inflight
        && has_more_candidates
    {
        // 中文注释：并发上限是软约束，最后一个候选仍要尝试，避免把可恢复抖动直接放大成全局不可用。
        super::super::super::record_gateway_candidate_skip(
            super::super::super::GatewayCandidateSkipReason::Inflight,
        );
        return Some(CandidateSkipReason::Inflight);
    }

    None
}
#[cfg(test)]
mod tests {
    use super::{
        allow_openai_fallback_for_account, candidate_skip_reason_for_proxy,
        free_account_model_override, CandidateSkipReason,
    };
    use codexmanager_core::storage::{
        now_ts, Account, ModelSourceMapping, ModelSourceModel, Storage, Token, UsageSnapshotRecord,
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

    /// 函数 `free_account_model_override_uses_configured_model_for_free_account`
    ///
    /// 作者: gaohongshun
    ///
    /// 时间: 2026-04-02
    ///
    /// # 参数
    /// 无
    ///
    /// # 返回
    /// 无
    #[test]
    fn free_account_model_override_uses_configured_model_for_free_account() {
        let _guard = crate::test_env_guard();
        let storage = Storage::open_in_memory().expect("open");
        storage.init().expect("init");
        let now = now_ts();
        storage
            .insert_account(&Account {
                id: "acc-free".to_string(),
                label: "acc-free".to_string(),
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
        let token = Token {
            account_id: "acc-free".to_string(),
            id_token: "header.payload.sig".to_string(),
            access_token: "header.payload.sig".to_string(),
            refresh_token: "refresh".to_string(),
            api_key_access_token: None,
            last_refresh: now,
        };
        storage.insert_token(&token).expect("insert token");
        storage
            .insert_usage_snapshot(&UsageSnapshotRecord {
                account_id: "acc-free".to_string(),
                used_percent: Some(10.0),
                window_minutes: Some(300),
                resets_at: None,
                secondary_used_percent: Some(20.0),
                secondary_window_minutes: Some(10_080),
                secondary_resets_at: None,
                credits_json: Some(r#"{"planType":"free"}"#.to_string()),
                captured_at: now,
            })
            .expect("insert usage");

        let original = crate::gateway::current_free_account_max_model();
        crate::gateway::set_free_account_max_model("gpt-5.2").expect("set free model");

        let account = Account {
            id: "acc-free".to_string(),
            label: "acc-free".to_string(),
            issuer: "issuer".to_string(),
            chatgpt_account_id: None,
            workspace_id: None,
            group_name: None,
            sort: 0,
            status: "active".to_string(),
            created_at: now,
            updated_at: now,
        };
        let actual = free_account_model_override(&storage, &account, &token);

        let _ = crate::gateway::set_free_account_max_model(&original);

        assert_eq!(actual.as_deref(), Some("gpt-5.2"));
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
            crate::gateway::LowQuotaCandidateMode::NormalOnly,
        )
        .expect("prepare candidates");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].0.id, "acc-direct-upstream");
    }

    #[test]
    fn prepare_gateway_candidates_skips_source_model_fallback_when_aggregate_mapping_exists() {
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
            crate::gateway::LowQuotaCandidateMode::NormalOnly,
        )
        .expect("prepare candidates");

        assert!(candidates.is_empty());
    }

    #[test]
    fn prepare_gateway_candidates_keeps_explicit_account_mapping_with_aggregate_mapping() {
        let _guard = crate::test_env_guard();
        let storage = Storage::open_in_memory().expect("open");
        storage.init().expect("init");
        let now = now_ts();
        insert_active_account_with_token(&storage, "acc-explicit-route", 0);
        insert_active_account_with_token(&storage, "acc-other-route", 1);
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

    /// 函数 `free_account_model_override_accepts_single_window_weekly_account`
    ///
    /// 作者: gaohongshun
    ///
    /// 时间: 2026-04-02
    ///
    /// # 参数
    /// 无
    ///
    /// # 返回
    /// 无
    #[test]
    fn free_account_model_override_accepts_single_window_weekly_account() {
        let _guard = crate::test_env_guard();
        let storage = Storage::open_in_memory().expect("open");
        storage.init().expect("init");
        let now = now_ts();
        storage
            .insert_account(&Account {
                id: "acc-weekly".to_string(),
                label: "acc-weekly".to_string(),
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
        let token = Token {
            account_id: "acc-weekly".to_string(),
            id_token: "header.payload.sig".to_string(),
            access_token: "header.payload.sig".to_string(),
            refresh_token: "refresh".to_string(),
            api_key_access_token: None,
            last_refresh: now,
        };
        storage.insert_token(&token).expect("insert token");
        storage
            .insert_usage_snapshot(&UsageSnapshotRecord {
                account_id: "acc-weekly".to_string(),
                used_percent: Some(10.0),
                window_minutes: Some(10_080),
                resets_at: None,
                secondary_used_percent: None,
                secondary_window_minutes: None,
                secondary_resets_at: None,
                credits_json: None,
                captured_at: now,
            })
            .expect("insert usage");

        let original = crate::gateway::current_free_account_max_model();
        crate::gateway::set_free_account_max_model("gpt-5.2").expect("set free model");

        let account = Account {
            id: "acc-weekly".to_string(),
            label: "acc-weekly".to_string(),
            issuer: "issuer".to_string(),
            chatgpt_account_id: None,
            workspace_id: None,
            group_name: None,
            sort: 0,
            status: "active".to_string(),
            created_at: now,
            updated_at: now,
        };
        let actual = free_account_model_override(&storage, &account, &token);

        let _ = crate::gateway::set_free_account_max_model(&original);

        assert_eq!(actual.as_deref(), Some("gpt-5.2"));
    }

    /// 函数 `free_account_model_override_skips_rewrite_when_configured_auto`
    ///
    /// 作者: gaohongshun
    ///
    /// 时间: 2026-04-02
    ///
    /// # 参数
    /// 无
    ///
    /// # 返回
    /// 无
    #[test]
    fn free_account_model_override_skips_rewrite_when_configured_auto() {
        let _guard = crate::test_env_guard();
        let storage = Storage::open_in_memory().expect("open");
        storage.init().expect("init");
        let now = now_ts();
        storage
            .insert_account(&Account {
                id: "acc-auto".to_string(),
                label: "acc-auto".to_string(),
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
        let token = Token {
            account_id: "acc-auto".to_string(),
            id_token: "header.payload.sig".to_string(),
            access_token: "header.payload.sig".to_string(),
            refresh_token: "refresh".to_string(),
            api_key_access_token: None,
            last_refresh: now,
        };
        storage.insert_token(&token).expect("insert token");
        storage
            .insert_usage_snapshot(&UsageSnapshotRecord {
                account_id: "acc-auto".to_string(),
                used_percent: Some(10.0),
                window_minutes: Some(300),
                resets_at: None,
                secondary_used_percent: Some(20.0),
                secondary_window_minutes: Some(10_080),
                secondary_resets_at: None,
                credits_json: Some(r#"{"planType":"free"}"#.to_string()),
                captured_at: now,
            })
            .expect("insert usage");

        let original = crate::gateway::current_free_account_max_model();
        crate::gateway::set_free_account_max_model("auto").expect("set free model");

        let account = Account {
            id: "acc-auto".to_string(),
            label: "acc-auto".to_string(),
            issuer: "issuer".to_string(),
            chatgpt_account_id: None,
            workspace_id: None,
            group_name: None,
            sort: 0,
            status: "active".to_string(),
            created_at: now,
            updated_at: now,
        };
        let actual = free_account_model_override(&storage, &account, &token);

        let _ = crate::gateway::set_free_account_max_model(&original);

        assert_eq!(actual, None);
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

        assert!(allow_openai_fallback_for_account(
            &storage, &account, &token
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

        assert!(!allow_openai_fallback_for_account(
            &storage, &account, &token
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
}
