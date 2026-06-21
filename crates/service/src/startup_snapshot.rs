use codexmanager_core::rpc::types::{StartupSnapshotResult, UsageAggregateSummaryResult};

use crate::{
    account_list, apikey_list, apikey_models, gateway, requestlog_list, requestlog_today_summary,
    storage_helpers, usage_aggregate, RpcActor,
};

const STARTUP_REQUEST_LOG_DEFAULT_LIMIT: i64 = 24;
const STARTUP_REQUEST_LOG_MAX_LIMIT: i64 = 24;

fn normalize_startup_request_log_limit(limit: Option<i64>) -> i64 {
    match limit {
        Some(value) if value <= 0 => 0,
        Some(value) => value.min(STARTUP_REQUEST_LOG_MAX_LIMIT),
        None => STARTUP_REQUEST_LOG_DEFAULT_LIMIT,
    }
}

/// 函数 `read_startup_snapshot`
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
pub(crate) fn read_startup_snapshot(
    request_log_limit: Option<i64>,
    day_start_ts: Option<i64>,
    day_end_ts: Option<i64>,
    include_api_models: bool,
) -> Result<StartupSnapshotResult, String> {
    let request_log_limit = normalize_startup_request_log_limit(request_log_limit);
    let storage =
        storage_helpers::open_storage().ok_or_else(|| "open storage failed".to_string())?;
    let accounts = storage
        .list_account_summary_rows()
        .map_err(|err| format!("list accounts failed: {err}"))?;
    let db_path = std::env::var("CODEXMANAGER_DB_PATH").unwrap_or_else(|_| "<unset>".to_string());
    log::info!(
        "startup/snapshot read: db_path={} account_count={}",
        db_path,
        accounts.len()
    );
    let account_ids = accounts
        .iter()
        .map(|account| account.id.clone())
        .collect::<Vec<_>>();
    let account_context =
        account_list::build_account_summary_context_from_rows(&storage, accounts)?;
    let usage_aggregate_summary =
        usage_aggregate::compute_usage_aggregate_summary_for_account_ids_list(
            &account_ids,
            &account_context.usage_snapshots,
        );
    let usage_snapshots = account_context
        .usage_snapshots
        .into_iter()
        .map(crate::usage_read::usage_snapshot_result_from_record)
        .collect();
    let api_keys = apikey_list::read_api_keys_with_storage(&storage)?;
    let api_models = if include_api_models {
        apikey_models::read_model_options_from_storage(&storage)?
    } else {
        Default::default()
    };
    let manual_preferred_account_id = gateway::manual_preferred_account();
    let request_log_today_summary =
        requestlog_today_summary::read_requestlog_today_summary_with_storage(
            &storage,
            day_start_ts,
            day_end_ts,
        )?;
    let request_logs =
        requestlog_list::read_request_logs_with_storage(&storage, None, Some(request_log_limit))?;

    Ok(StartupSnapshotResult {
        accounts: account_context.items,
        usage_snapshots,
        usage_aggregate_summary,
        api_keys,
        api_models,
        manual_preferred_account_id,
        request_log_today_summary,
        request_logs,
    })
}

pub(crate) fn read_startup_snapshot_for_actor(
    actor: &RpcActor,
    request_log_limit: Option<i64>,
    day_start_ts: Option<i64>,
    day_end_ts: Option<i64>,
    include_api_models: bool,
) -> Result<StartupSnapshotResult, String> {
    if actor.is_admin() {
        return read_startup_snapshot(
            request_log_limit,
            day_start_ts,
            day_end_ts,
            include_api_models,
        );
    }
    let request_log_limit = normalize_startup_request_log_limit(request_log_limit);
    let user_id = actor
        .user_id
        .as_deref()
        .ok_or_else(|| "permission_denied: startup requires user session".to_string())?;
    let storage =
        storage_helpers::open_storage().ok_or_else(|| "open storage failed".to_string())?;
    let key_ids = storage
        .list_api_key_ids_for_user(user_id)
        .map_err(|err| format!("list api key ids for user failed: {err}"))?;
    let api_keys = apikey_list::read_api_keys_for_ids_with_storage(&storage, &key_ids)?;
    let api_models = if include_api_models {
        apikey_models::read_model_options_from_storage(&storage)?
    } else {
        Default::default()
    };
    let request_log_today_summary =
        requestlog_today_summary::read_requestlog_today_summary_for_key_ids_with_storage(
            &storage,
            day_start_ts,
            day_end_ts,
            &key_ids,
        )?;
    let request_logs = requestlog_list::read_request_logs_for_key_ids_with_storage(
        &storage,
        None,
        Some(request_log_limit),
        &key_ids,
    )?;

    Ok(StartupSnapshotResult {
        accounts: Vec::new(),
        usage_snapshots: Vec::new(),
        usage_aggregate_summary: UsageAggregateSummaryResult::default(),
        api_keys,
        api_models,
        manual_preferred_account_id: None,
        request_log_today_summary,
        request_logs,
    })
}

#[cfg(test)]
mod tests {
    use super::normalize_startup_request_log_limit;

    #[test]
    fn startup_request_log_limit_defaults_to_light_sample() {
        assert_eq!(normalize_startup_request_log_limit(None), 24);
    }

    #[test]
    fn startup_request_log_limit_clamps_large_values() {
        assert_eq!(normalize_startup_request_log_limit(Some(120)), 24);
        assert_eq!(normalize_startup_request_log_limit(Some(500)), 24);
    }

    #[test]
    fn startup_request_log_limit_keeps_smaller_values_and_allows_zero() {
        assert_eq!(normalize_startup_request_log_limit(Some(8)), 8);
        assert_eq!(normalize_startup_request_log_limit(Some(0)), 0);
        assert_eq!(normalize_startup_request_log_limit(Some(-1)), 0);
    }
}
