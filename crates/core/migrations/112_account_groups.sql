CREATE TABLE IF NOT EXISTS account_groups (
  name TEXT PRIMARY KEY,
  description TEXT,
  status TEXT NOT NULL DEFAULT 'active',
  sort INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_account_groups_status_sort
  ON account_groups(status, sort ASC, name ASC);

INSERT OR IGNORE INTO account_groups (
  name, description, status, sort, created_at, updated_at
)
SELECT DISTINCT
  TRIM(group_name), NULL, 'active', 0, strftime('%s', 'now'), strftime('%s', 'now')
FROM accounts
WHERE group_name IS NOT NULL AND TRIM(group_name) != '';

ALTER TABLE api_keys ADD COLUMN account_group_filter TEXT;

CREATE INDEX IF NOT EXISTS idx_api_keys_account_group_filter
  ON api_keys(account_group_filter);
