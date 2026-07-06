use codexmanager_core::rpc::types::{
    AccountGroupEntry, AccountGroupListResult, AccountGroupUpsertParams,
};
use codexmanager_core::storage::{now_ts, AccountGroup, AccountGroupSummary, Storage};

use crate::storage_helpers;

fn normalize_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn normalize_group_name(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("账号组名称不能为空".to_string());
    }
    if trimmed.eq_ignore_ascii_case("__all__") {
        return Err("账号组名称不能使用保留值".to_string());
    }
    Ok(trimmed.to_string())
}

fn normalize_status(value: Option<&str>) -> Result<String, String> {
    match value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("active")
    {
        "active" => Ok("active".to_string()),
        "disabled" => Ok("disabled".to_string()),
        other => Err(format!("unsupported account group status: {other}")),
    }
}

fn group_entry(group: AccountGroupSummary) -> AccountGroupEntry {
    AccountGroupEntry {
        name: group.name,
        description: group.description,
        status: group.status,
        sort: group.sort,
        account_count: group.account_count,
        api_key_count: group.api_key_count,
        created_at: group.created_at,
        updated_at: group.updated_at,
    }
}

fn result_from_storage(storage: &Storage) -> Result<AccountGroupListResult, String> {
    let items = storage
        .list_account_group_summaries()
        .map_err(|err| format!("list account groups failed: {err}"))?
        .into_iter()
        .map(group_entry)
        .collect();
    Ok(AccountGroupListResult { items })
}

pub(crate) fn read_account_groups() -> Result<AccountGroupListResult, String> {
    let storage =
        storage_helpers::open_storage().ok_or_else(|| "storage unavailable".to_string())?;
    result_from_storage(&storage)
}

pub(crate) fn upsert_account_group(
    params: AccountGroupUpsertParams,
) -> Result<AccountGroupEntry, String> {
    let storage =
        storage_helpers::open_storage().ok_or_else(|| "storage unavailable".to_string())?;
    let name = normalize_group_name(params.name.as_str())?;
    let old_name = normalize_text(params.old_name.as_deref());
    let existing = storage
        .list_account_group_summaries()
        .map_err(|err| format!("list account groups failed: {err}"))?
        .into_iter()
        .find(|item| item.name == old_name.as_deref().unwrap_or(name.as_str()));
    let now = now_ts();
    let group = AccountGroup {
        name: name.clone(),
        description: normalize_text(params.description.as_deref()),
        status: normalize_status(params.status.as_deref())?,
        sort: params
            .sort
            .unwrap_or_else(|| existing.as_ref().map(|item| item.sort).unwrap_or(0)),
        created_at: existing.as_ref().map(|item| item.created_at).unwrap_or(now),
        updated_at: now,
    };
    storage
        .upsert_account_group(old_name.as_deref(), &group)
        .map_err(|err| format!("save account group failed: {err}"))?;
    storage
        .list_account_group_summaries()
        .map_err(|err| format!("list account groups failed: {err}"))?
        .into_iter()
        .find(|item| item.name == name)
        .map(group_entry)
        .ok_or_else(|| "账号组保存结果为空".to_string())
}

pub(crate) fn delete_account_group(name: &str) -> Result<AccountGroupListResult, String> {
    let storage =
        storage_helpers::open_storage().ok_or_else(|| "storage unavailable".to_string())?;
    let name = normalize_group_name(name)?;
    let (account_count, api_key_count) = storage
        .delete_account_group(name.as_str())
        .map_err(|err| format!("delete account group failed: {err}"))?;
    if account_count > 0 || api_key_count > 0 {
        return Err(format!(
            "账号组已被使用，不能删除（账号 {account_count} 个，平台密钥 {api_key_count} 个）"
        ));
    }
    result_from_storage(&storage)
}

pub(crate) fn normalize_account_group_filter(
    value: Option<String>,
    storage: &Storage,
) -> Result<Option<String>, String> {
    let Some(value) = normalize_text(value.as_deref()) else {
        return Ok(None);
    };
    if value.eq_ignore_ascii_case("__all__") || value == "全部" {
        return Ok(None);
    }
    if !storage
        .account_group_exists(value.as_str())
        .map_err(|err| format!("check account group failed: {err}"))?
    {
        return Err(format!("账号组不存在: {value}"));
    }
    Ok(Some(value))
}
