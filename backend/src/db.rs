use sqlx::SqlitePool;
use uuid::Uuid;

pub struct UserRow {
    pub uuid: String,
    pub name: String,
    pub email: String,
}

pub async fn get_user_by_uuid(pool: &SqlitePool, uuid: Uuid) -> sqlx::Result<Option<UserRow>> {
    let uuid = uuid.to_string();
    let row = sqlx::query!("SELECT uuid, name, email FROM users WHERE uuid = ?", uuid)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| UserRow { uuid: r.uuid, name: r.name, email: r.email }))
}

pub struct SlotRow {
    pub id: i64,
    pub label: String,
    pub max_bookings: i64,
    pub current_bookings: i64,
}

pub async fn get_slots(pool: &SqlitePool) -> sqlx::Result<Vec<SlotRow>> {
    let rows = sqlx::query!(
        r#"
        SELECT s.id, s.label, s.max_bookings,
               COUNT(b.user_uuid) AS current_bookings
        FROM slots s
        LEFT JOIN bookings b ON b.slot_id = s.id
        GROUP BY s.id
        ORDER BY s.id
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| SlotRow {
            id: r.id,
            label: r.label,
            max_bookings: r.max_bookings,
            current_bookings: r.current_bookings,
        })
        .collect())
}

pub async fn get_current_slot_id(pool: &SqlitePool, user_uuid: &str) -> sqlx::Result<Option<i64>> {
    sqlx::query_scalar!("SELECT slot_id FROM bookings WHERE user_uuid = ?", user_uuid)
        .fetch_optional(pool)
        .await
}

pub enum BookResult {
    Ok { slot_id: i64, label: String },
    SlotFull,
}

pub async fn book_slot(
    pool: &SqlitePool,
    user_uuid: &str,
    name: &str,
    email: &str,
    slot_id: i64,
) -> sqlx::Result<BookResult> {
    let mut tx = pool.begin().await?;

    sqlx::query!(
        "UPDATE users SET name = ?, email = ? WHERE uuid = ?",
        name,
        email,
        user_uuid
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!("DELETE FROM bookings WHERE user_uuid = ?", user_uuid)
        .execute(&mut *tx)
        .await?;

    let slot = sqlx::query!(
        r#"
        SELECT s.label, s.max_bookings, COUNT(b.user_uuid) AS current_bookings
        FROM slots s
        LEFT JOIN bookings b ON b.slot_id = s.id
        WHERE s.id = ?
        GROUP BY s.id
        "#,
        slot_id
    )
    .fetch_optional(&mut *tx)
    .await?;

    let Some(slot) = slot else {
        tx.rollback().await?;
        return Ok(BookResult::SlotFull);
    };

    if slot.current_bookings >= slot.max_bookings {
        tx.rollback().await?;
        return Ok(BookResult::SlotFull);
    }

    sqlx::query!(
        "INSERT INTO bookings (user_uuid, slot_id) VALUES (?, ?)",
        user_uuid,
        slot_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(BookResult::Ok { slot_id, label: slot.label })
}

pub struct UserRecord {
    pub token: String,
    pub name: String,
    pub email: String,
}

pub async fn is_valid_admin(pool: &SqlitePool, admin_uuid: &str) -> sqlx::Result<bool> {
    let count: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM admins WHERE uuid = ?",
        admin_uuid
    )
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

pub async fn list_users(pool: &SqlitePool) -> sqlx::Result<Vec<UserRecord>> {
    let rows = sqlx::query!("SELECT uuid, name, email FROM users ORDER BY uuid")
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| UserRecord { token: r.uuid, name: r.name, email: r.email })
        .collect())
}

pub async fn create_user(pool: &SqlitePool) -> sqlx::Result<UserRecord> {
    let uuid = Uuid::new_v4().to_string();
    sqlx::query!(
        "INSERT INTO users (uuid, name, email) VALUES (?, '', '')",
        uuid
    )
    .execute(pool)
    .await?;
    Ok(UserRecord { token: uuid, name: String::new(), email: String::new() })
}

pub async fn delete_user(pool: &SqlitePool, user_uuid: &str) -> sqlx::Result<bool> {
    let result = sqlx::query!("DELETE FROM users WHERE uuid = ?", user_uuid)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn list_slots_admin(pool: &SqlitePool) -> sqlx::Result<Vec<SlotRow>> {
    get_slots(pool).await
}

pub async fn create_slot(pool: &SqlitePool, label: &str, max_bookings: i32) -> sqlx::Result<SlotRow> {
    let result = sqlx::query!(
        "INSERT INTO slots (label, max_bookings) VALUES (?, ?)",
        label,
        max_bookings
    )
    .execute(pool)
    .await?;
    let id = result.last_insert_rowid();
    Ok(SlotRow { id, label: label.to_string(), max_bookings: max_bookings as i64, current_bookings: 0 })
}

pub async fn delete_slot(pool: &SqlitePool, slot_id: i64) -> sqlx::Result<bool> {
    let result = sqlx::query!("DELETE FROM slots WHERE id = ?", slot_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn ensure_admin(pool: &SqlitePool) -> sqlx::Result<Option<String>> {
    let count: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM admins")
        .fetch_one(pool)
        .await?;

    if count == 0 {
        let uuid = Uuid::new_v4().to_string();
        sqlx::query!("INSERT INTO admins (uuid, name) VALUES (?, 'admin')", uuid)
            .execute(pool)
            .await?;
        return Ok(Some(uuid));
    }

    Ok(None)
}
