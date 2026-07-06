use rusqlite::{Result, Row};

use super::{now_ts, AccountGroup, AccountGroupSummary, Storage};

fn map_account_group_summary(row: &Row<'_>) -> Result<AccountGroupSummary> {
    Ok(AccountGroupSummary {
        name: row.get(0)?,
        description: row.get(1)?,
        status: row.get(2)?,
        sort: row.get(3)?,
        account_count: row.get(4)?,
        api_key_count: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

impl Storage {
    pub(super) fn ensure_account_groups_table(&self) -> Result<()> {
        self.ensure_column("api_keys", "account_group_filter", "TEXT")?;
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS account_groups (
               name TEXT PRIMARY KEY,
               description TEXT,
               status TEXT NOT NULL DEFAULT 'active',
               sort INTEGER NOT NULL DEFAULT 0,
               created_at INTEGER NOT NULL,
               updated_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_account_groups_status_sort
               ON account_groups(status, sort ASC, name ASC);
             CREATE INDEX IF NOT EXISTS idx_api_keys_account_group_filter
               ON api_keys(account_group_filter);
             INSERT OR IGNORE INTO account_groups (
               name, description, status, sort, created_at, updated_at
             )
             SELECT DISTINCT
               TRIM(group_name), NULL, 'active', 0, strftime('%s', 'now'), strftime('%s', 'now')
             FROM accounts
             WHERE group_name IS NOT NULL AND TRIM(group_name) != '';",
        )?;
        Ok(())
    }

    pub fn list_account_group_summaries(&self) -> Result<Vec<AccountGroupSummary>> {
        self.ensure_account_groups_table()?;
        let mut stmt = self.conn.prepare(
            "WITH names(name) AS (
               SELECT name FROM account_groups
               UNION
               SELECT TRIM(group_name) FROM accounts
                 WHERE group_name IS NOT NULL AND TRIM(group_name) != ''
               UNION
               SELECT TRIM(account_group_filter) FROM api_keys
                 WHERE account_group_filter IS NOT NULL AND TRIM(account_group_filter) != ''
             )
             SELECT
               n.name,
               g.description,
               COALESCE(g.status, 'active') AS status,
               COALESCE(g.sort, 0) AS sort,
               (
                 SELECT COUNT(1) FROM accounts a
                 WHERE a.group_name IS NOT NULL AND TRIM(a.group_name) = n.name
               ) AS account_count,
               (
                 SELECT COUNT(1) FROM api_keys k
                 WHERE k.account_group_filter IS NOT NULL AND TRIM(k.account_group_filter) = n.name
               ) AS api_key_count,
               COALESCE(g.created_at, 0) AS created_at,
               COALESCE(g.updated_at, 0) AS updated_at
             FROM names n
             LEFT JOIN account_groups g ON g.name = n.name
             ORDER BY sort ASC, n.name ASC",
        )?;
        stmt.query_map([], map_account_group_summary)?.collect()
    }

    pub fn account_group_exists(&self, name: &str) -> Result<bool> {
        self.ensure_account_groups_table()?;
        let normalized = name.trim();
        self.conn.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM account_groups WHERE name = ?1
               UNION
               SELECT 1 FROM accounts WHERE group_name IS NOT NULL AND TRIM(group_name) = ?1
               UNION
               SELECT 1 FROM api_keys
                WHERE account_group_filter IS NOT NULL AND TRIM(account_group_filter) = ?1
             )",
            [normalized],
            |row| row.get(0),
        )
    }

    pub fn upsert_account_group(&self, old_name: Option<&str>, group: &AccountGroup) -> Result<()> {
        self.ensure_account_groups_table()?;
        let now = now_ts();
        let old_name = old_name.map(str::trim).filter(|value| !value.is_empty());
        let tx = self.conn.unchecked_transaction()?;
        if let Some(old_name) = old_name.filter(|value| *value != group.name) {
            tx.execute(
                "UPDATE accounts SET group_name = ?1, updated_at = ?2
                 WHERE group_name IS NOT NULL AND TRIM(group_name) = ?3",
                (&group.name, now, old_name),
            )?;
            tx.execute(
                "UPDATE api_keys SET account_group_filter = ?1
                 WHERE account_group_filter IS NOT NULL AND TRIM(account_group_filter) = ?2",
                (&group.name, old_name),
            )?;
            tx.execute("DELETE FROM account_groups WHERE name = ?1", [old_name])?;
        }
        tx.execute(
            "INSERT INTO account_groups (
               name, description, status, sort, created_at, updated_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(name) DO UPDATE SET
               description = excluded.description,
               status = excluded.status,
               sort = excluded.sort,
               updated_at = excluded.updated_at",
            (
                &group.name,
                &group.description,
                &group.status,
                group.sort,
                if group.created_at > 0 {
                    group.created_at
                } else {
                    now
                },
                now,
            ),
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn ensure_account_group_name(&self, name: &str) -> Result<()> {
        self.ensure_account_groups_table()?;
        let normalized = name.trim();
        if normalized.is_empty() {
            return Ok(());
        }
        let now = now_ts();
        self.conn.execute(
            "INSERT OR IGNORE INTO account_groups (
               name, description, status, sort, created_at, updated_at
             )
             VALUES (?1, NULL, 'active', 0, ?2, ?2)",
            (normalized, now),
        )?;
        Ok(())
    }

    pub fn delete_account_group(&self, name: &str) -> Result<(i64, i64)> {
        self.ensure_account_groups_table()?;
        let normalized = name.trim();
        let account_count = self.conn.query_row(
            "SELECT COUNT(1) FROM accounts
             WHERE group_name IS NOT NULL AND TRIM(group_name) = ?1",
            [normalized],
            |row| row.get(0),
        )?;
        let api_key_count = self.conn.query_row(
            "SELECT COUNT(1) FROM api_keys
             WHERE account_group_filter IS NOT NULL AND TRIM(account_group_filter) = ?1",
            [normalized],
            |row| row.get(0),
        )?;
        if account_count == 0 && api_key_count == 0 {
            self.conn
                .execute("DELETE FROM account_groups WHERE name = ?1", [normalized])?;
        }
        Ok((account_count, api_key_count))
    }
}
