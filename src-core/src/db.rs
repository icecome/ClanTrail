use crate::error::{AppError, Result};
use crate::models::*;
use chrono::Datelike;
use rusqlite::{params, Connection, OptionalExtension};

/// 本地 SQLite 数据库封装，离线优先的数据源
pub struct TombKeeperDb {
    conn: Connection,
}

impl TombKeeperDb {
    /// 打开（或创建）数据库文件
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        // 自动检测数据库目录是否可写 journal 文件；不可写时退到 MEMORY journal，
        // 避免沙箱/只读目录下 "attempt to write a readonly database"。
        let dir = std::path::Path::new(path)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        let can_write_journal = (|| {
            let test = dir.join(format!(".tk-write-test-{}", std::process::id()));
            std::fs::File::create(&test).ok()?;
            std::fs::remove_file(&test).ok()?;
            Some(())
        })()
        .is_some();
        if !can_write_journal {
            conn.execute_batch("PRAGMA journal_mode = MEMORY;")?;
        }

        let db = Self { conn };
        db.initialize()?;
        Ok(db)
    }

    /// 打开内存数据库（测试用）
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        let db = Self { conn };
        db.initialize()?;
        Ok(db)
    }

    /// 关闭当前文件连接，切到一个临时内存连接。
    /// 用于导入备份前释放对 db 文件的占用（Windows 下必须释放句柄才能替换文件）。
    pub fn close_to_memory(&mut self) -> Result<()> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        self.conn = conn;
        Ok(())
    }

    /// 重新打开指定路径的数据库文件，替换当前连接。
    pub fn reopen(&mut self, path: &str) -> Result<()> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        self.conn = conn;
        Ok(())
    }

    fn initialize(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS families (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                description TEXT,
                origin      TEXT,
                created_at  TEXT NOT NULL,
                updated_at  TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS tomb_groups (
                id          TEXT PRIMARY KEY,
                family_id   TEXT NOT NULL REFERENCES families(id) ON DELETE CASCADE,
                name        TEXT NOT NULL,
                description TEXT,
                created_at  TEXT NOT NULL,
                updated_at  TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS tombs (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                latitude    REAL NOT NULL,
                longitude   REAL NOT NULL,
                address     TEXT,
                description TEXT,
                group_id    TEXT REFERENCES tomb_groups(id) ON DELETE SET NULL,
                family_id   TEXT REFERENCES families(id) ON DELETE SET NULL,
                created_at  TEXT NOT NULL,
                updated_at  TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS persons (
                id              TEXT PRIMARY KEY,
                tomb_id         TEXT NOT NULL REFERENCES tombs(id) ON DELETE CASCADE,
                name            TEXT NOT NULL,
                title           TEXT,
                birth_date      TEXT,
                death_date      TEXT,
                biography       TEXT,
                epitaph         TEXT,
                spouse          TEXT,
                is_joint_burial INTEGER NOT NULL DEFAULT 0,
                children        TEXT,
                order_index     INTEGER NOT NULL DEFAULT 0,
                created_at      TEXT NOT NULL,
                updated_at      TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS photos (
                id          TEXT PRIMARY KEY,
                entity_type TEXT NOT NULL,
                entity_id   TEXT NOT NULL,
                file_path   TEXT NOT NULL,
                caption     TEXT,
                is_cover    INTEGER NOT NULL DEFAULT 0,
                created_at  TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS relations (
                id                TEXT PRIMARY KEY,
                person_id         TEXT NOT NULL REFERENCES persons(id) ON DELETE CASCADE,
                related_person_id TEXT NOT NULL REFERENCES persons(id) ON DELETE CASCADE,
                relation_type     TEXT NOT NULL,
                created_at        TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_tombs_group   ON tombs(group_id);
            CREATE INDEX IF NOT EXISTS idx_tombs_family  ON tombs(family_id);
            CREATE INDEX IF NOT EXISTS idx_persons_tomb  ON persons(tomb_id);
            CREATE INDEX IF NOT EXISTS idx_photos_entity ON photos(entity_type, entity_id);
            CREATE INDEX IF NOT EXISTS idx_relations_person ON relations(person_id);
            CREATE INDEX IF NOT EXISTS idx_relations_related ON relations(related_person_id);
            "#,
        )?;

        // 兼容老库：缺失的列逐个 ALTER TABLE 补上
        self.migrate_add_columns()?;
        Ok(())
    }

    /// 轻量级迁移：仅处理 persons 表新增列
    fn migrate_add_columns(&self) -> Result<()> {
        // pragma table_info 返回 (cid, name, type, notnull, dflt_value, pk)
        let mut stmt = self.conn
            .prepare("PRAGMA table_info(persons)")?;
        let existing: std::collections::HashSet<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<std::result::Result<_, _>>()?;

        let to_add: &[(&str, &str)] = &[
            ("spouse", "TEXT"),
            ("is_joint_burial", "INTEGER NOT NULL DEFAULT 0"),
            ("children", "TEXT"),
        ];
        for (col, decl) in to_add {
            if !existing.contains(*col) {
                let sql = format!("ALTER TABLE persons ADD COLUMN {col} {decl}");
                self.conn.execute_batch(&sql)?;
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Family 家族
    // -----------------------------------------------------------------------
    pub fn create_family(&self, input: NewFamily) -> Result<Family> {
        let now = now_iso();
        let family = Family {
            id: new_uuid(),
            name: input.name,
            description: input.description,
            origin: input.origin,
            created_at: now.clone(),
            updated_at: now,
        };
        self.conn.execute(
            "INSERT INTO families (id, name, description, origin, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                family.id,
                family.name,
                family.description,
                family.origin,
                family.created_at,
                family.updated_at
            ],
        )?;
        Ok(family)
    }

    pub fn get_family(&self, id: &str) -> Result<Family> {
        self.conn
            .query_row(
                "SELECT id, name, description, origin, created_at, updated_at
                 FROM families WHERE id = ?1",
                params![id],
                |row| {
                    Ok(Family {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        description: row.get(2)?,
                        origin: row.get(3)?,
                        created_at: row.get(4)?,
                        updated_at: row.get(5)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("family {id}")))
    }

    pub fn list_families(&self) -> Result<Vec<Family>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, description, origin, created_at, updated_at
             FROM families ORDER BY name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Family {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                origin: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn update_family(
        &self,
        id: &str,
        name: Option<String>,
        description: Option<String>,
        origin: Option<String>,
    ) -> Result<Family> {
        let current = self.get_family(id)?;
        let new_name = name.unwrap_or(current.name);
        let new_desc = description.or(current.description);
        let new_origin = origin.or(current.origin);
        let now = now_iso();
        self.conn.execute(
            "UPDATE families SET name = ?1, description = ?2, origin = ?3, updated_at = ?4 WHERE id = ?5",
            params![new_name, new_desc, new_origin, now, id],
        )?;
        self.get_family(id)
    }

    pub fn delete_family(&self, id: &str) -> Result<()> {
        let n = self.conn.execute("DELETE FROM families WHERE id = ?1", params![id])?;
        if n == 0 {
            return Err(AppError::NotFound(format!("family {id}")));
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // TombGroup 墓组
    // -----------------------------------------------------------------------
    pub fn create_tomb_group(&self, input: NewTombGroup) -> Result<TombGroup> {
        // 校验家族存在
        self.get_family(&input.family_id)?;
        let now = now_iso();
        let group = TombGroup {
            id: new_uuid(),
            family_id: input.family_id,
            name: input.name,
            description: input.description,
            created_at: now.clone(),
            updated_at: now,
        };
        self.conn.execute(
            "INSERT INTO tomb_groups (id, family_id, name, description, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                group.id,
                group.family_id,
                group.name,
                group.description,
                group.created_at,
                group.updated_at
            ],
        )?;
        Ok(group)
    }

    fn row_to_tomb_group(row: &rusqlite::Row<'_>) -> rusqlite::Result<TombGroup> {
        Ok(TombGroup {
            id: row.get(0)?,
            family_id: row.get(1)?,
            name: row.get(2)?,
            description: row.get(3)?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    }

    pub fn get_tomb_group(&self, id: &str) -> Result<TombGroup> {
        self.conn
            .query_row(
                "SELECT id, family_id, name, description, created_at, updated_at
                 FROM tomb_groups WHERE id = ?1",
                params![id],
                Self::row_to_tomb_group,
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("tomb_group {id}")))
    }

    pub fn list_groups_by_family(&self, family_id: &str) -> Result<Vec<TombGroup>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, family_id, name, description, created_at, updated_at
             FROM tomb_groups WHERE family_id = ?1 ORDER BY name",
        )?;
        let rows = stmt.query_map(params![family_id], Self::row_to_tomb_group)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn delete_tomb_group(&self, id: &str) -> Result<()> {
        let n = self.conn.execute("DELETE FROM tomb_groups WHERE id = ?1", params![id])?;
        if n == 0 {
            return Err(AppError::NotFound(format!("tomb_group {id}")));
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Tomb 单墓地
    // -----------------------------------------------------------------------
    pub fn create_tomb(&self, input: NewTomb) -> Result<Tomb> {
        if input.latitude < -90.0 || input.latitude > 90.0 {
            return Err(AppError::InvalidInput("latitude out of range".into()));
        }
        if input.longitude < -180.0 || input.longitude > 180.0 {
            return Err(AppError::InvalidInput("longitude out of range".into()));
        }
        if let Some(fid) = &input.family_id {
            self.get_family(fid)?;
        }
        if let Some(gid) = &input.group_id {
            self.get_tomb_group(gid)?;
        }
        let now = now_iso();
        let tomb = Tomb {
            id: new_uuid(),
            name: input.name,
            latitude: input.latitude,
            longitude: input.longitude,
            address: input.address,
            description: input.description,
            group_id: input.group_id,
            family_id: input.family_id,
            created_at: now.clone(),
            updated_at: now,
        };
        self.conn.execute(
            "INSERT INTO tombs (id, name, latitude, longitude, address, description, group_id, family_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                tomb.id,
                tomb.name,
                tomb.latitude,
                tomb.longitude,
                tomb.address,
                tomb.description,
                tomb.group_id,
                tomb.family_id,
                tomb.created_at,
                tomb.updated_at
            ],
        )?;
        Ok(tomb)
    }

    fn row_to_tomb(row: &rusqlite::Row<'_>) -> rusqlite::Result<Tomb> {
        Ok(Tomb {
            id: row.get(0)?,
            name: row.get(1)?,
            latitude: row.get(2)?,
            longitude: row.get(3)?,
            address: row.get(4)?,
            description: row.get(5)?,
            group_id: row.get(6)?,
            family_id: row.get(7)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
        })
    }

    pub fn get_tomb(&self, id: &str) -> Result<Tomb> {
        self.conn
            .query_row(
                "SELECT id, name, latitude, longitude, address, description, group_id, family_id, created_at, updated_at
                 FROM tombs WHERE id = ?1",
                params![id],
                Self::row_to_tomb,
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("tomb {id}")))
    }

    pub fn list_tombs(&self) -> Result<Vec<Tomb>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, latitude, longitude, address, description, group_id, family_id, created_at, updated_at
             FROM tombs ORDER BY name",
        )?;
        let rows = stmt.query_map([], Self::row_to_tomb)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn list_tombs_by_family(&self, family_id: &str) -> Result<Vec<Tomb>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, latitude, longitude, address, description, group_id, family_id, created_at, updated_at
             FROM tombs WHERE family_id = ?1 ORDER BY name",
        )?;
        let rows = stmt.query_map(params![family_id], Self::row_to_tomb)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn list_tombs_by_group(&self, group_id: &str) -> Result<Vec<Tomb>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, latitude, longitude, address, description, group_id, family_id, created_at, updated_at
             FROM tombs WHERE group_id = ?1 ORDER BY name",
        )?;
        let rows = stmt.query_map(params![group_id], Self::row_to_tomb)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn update_tomb(
        &self,
        id: &str,
        name: Option<String>,
        latitude: Option<f64>,
        longitude: Option<f64>,
        address: Option<String>,
        description: Option<String>,
        group_id: Option<String>,
        family_id: Option<String>,
    ) -> Result<Tomb> {
        let current = self.get_tomb(id)?;
        let new_name = name.unwrap_or(current.name);
        let new_lat = latitude.unwrap_or(current.latitude);
        let new_lng = longitude.unwrap_or(current.longitude);
        let new_addr = address.or(current.address);
        let new_desc = description.or(current.description);
        let new_group = group_id.or(current.group_id);
        let new_family = family_id.or(current.family_id);
        let now = now_iso();
        self.conn.execute(
            "UPDATE tombs SET name = ?1, latitude = ?2, longitude = ?3, address = ?4, description = ?5,
             group_id = ?6, family_id = ?7, updated_at = ?8 WHERE id = ?9",
            params![new_name, new_lat, new_lng, new_addr, new_desc, new_group, new_family, now, id],
        )?;
        self.get_tomb(id)
    }

    pub fn delete_tomb(&self, id: &str) -> Result<()> {
        let n = self.conn.execute("DELETE FROM tombs WHERE id = ?1", params![id])?;
        if n == 0 {
            return Err(AppError::NotFound(format!("tomb {id}")));
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Person 墓主
    // -----------------------------------------------------------------------
    pub fn create_person(&self, input: NewPerson) -> Result<Person> {
        self.get_tomb(&input.tomb_id)?;
        let now = now_iso();
        let person = Person {
            id: new_uuid(),
            tomb_id: input.tomb_id,
            name: input.name,
            title: input.title,
            birth_date: input.birth_date,
            death_date: input.death_date,
            biography: input.biography,
            epitaph: input.epitaph,
            spouse: input.spouse,
            is_joint_burial: input.is_joint_burial,
            children: input.children,
            order_index: input.order_index,
            created_at: now.clone(),
            updated_at: now,
        };
        self.conn.execute(
            "INSERT INTO persons (id, tomb_id, name, title, birth_date, death_date, biography, epitaph, spouse, is_joint_burial, children, order_index, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                person.id,
                person.tomb_id,
                person.name,
                person.title,
                person.birth_date,
                person.death_date,
                person.biography,
                person.epitaph,
                person.spouse,
                person.is_joint_burial as i32,
                person.children,
                person.order_index,
                person.created_at,
                person.updated_at
            ],
        )?;
        Ok(person)
    }

    fn row_to_person(row: &rusqlite::Row<'_>) -> rusqlite::Result<Person> {
        Ok(Person {
            id: row.get(0)?,
            tomb_id: row.get(1)?,
            name: row.get(2)?,
            title: row.get(3)?,
            birth_date: row.get(4)?,
            death_date: row.get(5)?,
            biography: row.get(6)?,
            epitaph: row.get(7)?,
            spouse: row.get(8)?,
            is_joint_burial: {
                let v: i64 = row.get(9)?;
                v != 0
            },
            children: row.get(10)?,
            order_index: row.get(11)?,
            created_at: row.get(12)?,
            updated_at: row.get(13)?,
        })
    }

    pub fn list_persons_by_tomb(&self, tomb_id: &str) -> Result<Vec<Person>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, tomb_id, name, title, birth_date, death_date, biography, epitaph, spouse, is_joint_burial, children, order_index, created_at, updated_at
             FROM persons WHERE tomb_id = ?1 ORDER BY order_index, name",
        )?;
        let rows = stmt.query_map(params![tomb_id], Self::row_to_person)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn get_person(&self, id: &str) -> Result<Person> {
        self.conn
            .query_row(
                "SELECT id, tomb_id, name, title, birth_date, death_date, biography, epitaph, spouse, is_joint_burial, children, order_index, created_at, updated_at
                 FROM persons WHERE id = ?1",
                params![id],
                Self::row_to_person,
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("person {id}")))
    }

    pub fn update_person(
        &self,
        id: &str,
        name: Option<String>,
        title: Option<String>,
        birth_date: Option<String>,
        death_date: Option<String>,
        biography: Option<String>,
        epitaph: Option<String>,
        spouse: Option<String>,
        is_joint_burial: Option<bool>,
        children: Option<String>,
        order_index: Option<i32>,
    ) -> Result<Person> {
        let current = self.get_person(id)?;
        let new_name = name.unwrap_or(current.name);
        let new_title = title.or(current.title);
        let new_birth = birth_date.or(current.birth_date);
        let new_death = death_date.or(current.death_date);
        let new_bio = biography.or(current.biography);
        let new_epitaph = epitaph.or(current.epitaph);
        let new_spouse = spouse.or(current.spouse);
        let new_joint = is_joint_burial.unwrap_or(current.is_joint_burial);
        let new_children = children.or(current.children);
        let new_order = order_index.unwrap_or(current.order_index);
        let now = now_iso();
        self.conn.execute(
            "UPDATE persons SET name = ?1, title = ?2, birth_date = ?3, death_date = ?4, biography = ?5,
             epitaph = ?6, spouse = ?7, is_joint_burial = ?8, children = ?9, order_index = ?10, updated_at = ?11
             WHERE id = ?12",
            params![
                new_name, new_title, new_birth, new_death, new_bio, new_epitaph,
                new_spouse, new_joint as i32, new_children, new_order, now, id
            ],
        )?;
        self.get_person(id)
    }

    pub fn delete_person(&self, id: &str) -> Result<()> {
        let n = self.conn.execute("DELETE FROM persons WHERE id = ?1", params![id])?;
        if n == 0 {
            return Err(AppError::NotFound(format!("person {id}")));
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Photo 照片
    // -----------------------------------------------------------------------
    pub fn add_photo(&self, input: NewPhoto) -> Result<Photo> {
        let photo = Photo {
            id: new_uuid(),
            entity_type: input.entity_type.as_str().to_string(),
            entity_id: input.entity_id,
            file_path: input.file_path,
            caption: input.caption,
            is_cover: input.is_cover,
            created_at: now_iso(),
        };
        self.conn.execute(
            "INSERT INTO photos (id, entity_type, entity_id, file_path, caption, is_cover, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                photo.id,
                photo.entity_type,
                photo.entity_id,
                photo.file_path,
                photo.caption,
                photo.is_cover,
                photo.created_at
            ],
        )?;
        Ok(photo)
    }

    /// 按 ID 取单张照片（删除时用于定位物理文件）
    pub fn get_photo(&self, id: &str) -> Result<Photo> {
        self.conn
            .query_row(
                "SELECT id, entity_type, entity_id, file_path, caption, is_cover, created_at
                 FROM photos WHERE id = ?1",
                params![id],
                |row| {
                    Ok(Photo {
                        id: row.get(0)?,
                        entity_type: row.get(1)?,
                        entity_id: row.get(2)?,
                        file_path: row.get(3)?,
                        caption: row.get(4)?,
                        is_cover: row.get(5)?,
                        created_at: row.get(6)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("photo {id}")))
    }

    pub fn list_photos_by_entity(&self, entity_type: EntityType, entity_id: &str) -> Result<Vec<Photo>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, entity_type, entity_id, file_path, caption, is_cover, created_at
             FROM photos WHERE entity_type = ?1 AND entity_id = ?2 ORDER BY created_at",
        )?;
        let rows = stmt.query_map(params![entity_type.as_str(), entity_id], |row| {
            Ok(Photo {
                id: row.get(0)?,
                entity_type: row.get(1)?,
                entity_id: row.get(2)?,
                file_path: row.get(3)?,
                caption: row.get(4)?,
                is_cover: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn delete_photo(&self, id: &str) -> Result<()> {
        let n = self.conn.execute("DELETE FROM photos WHERE id = ?1", params![id])?;
        if n == 0 {
            return Err(AppError::NotFound(format!("photo {id}")));
        }
        Ok(())
    }

    /// 更新照片封面标记（把同实体的其他封面清除）
    pub fn set_photo_cover(&self, id: &str, is_cover: bool) -> Result<Photo> {
        let photo = self
            .conn
            .query_row(
                "SELECT id, entity_type, entity_id, file_path, caption, is_cover, created_at
                 FROM photos WHERE id = ?1",
                params![id],
                |row| {
                    Ok(Photo {
                        id: row.get(0)?,
                        entity_type: row.get(1)?,
                        entity_id: row.get(2)?,
                        file_path: row.get(3)?,
                        caption: row.get(4)?,
                        is_cover: row.get(5)?,
                        created_at: row.get(6)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("photo {id}")))?;

        if is_cover {
            // 同一实体的其他照片封面标记清零
            self.conn.execute(
                "UPDATE photos SET is_cover = 0 WHERE entity_type = ?1 AND entity_id = ?2 AND id != ?3",
                params![photo.entity_type, photo.entity_id, id],
            )?;
        }
        self.conn
            .execute("UPDATE photos SET is_cover = ?1 WHERE id = ?2", params![is_cover as i32, id])?;
        self.list_photos_by_entity(
            EntityType::from_str(&photo.entity_type).ok_or_else(|| {
                AppError::General(format!("unknown entity_type {}", photo.entity_type))
            })?,
            &photo.entity_id,
        )?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| AppError::NotFound(format!("photo {id}")))
    }

    // -----------------------------------------------------------------------
    // Relation 人物关系
    // -----------------------------------------------------------------------
    /// 创建关系（自动创建反向关系，保证双向一致性）
    pub fn create_relation(&self, input: NewRelation) -> Result<Vec<Relation>> {
        // 校验两个人物都存在
        self.get_person(&input.person_id)?;
        self.get_person(&input.related_person_id)?;
        if input.person_id == input.related_person_id {
            return Err(AppError::InvalidInput("cannot create self-relation".into()));
        }
        // 检查是否已存在相同关系
        let exists: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM relations WHERE person_id = ?1 AND related_person_id = ?2 AND relation_type = ?3",
            params![input.person_id, input.related_person_id, input.relation_type.as_str()],
            |row| row.get(0),
        )?;
        if exists > 0 {
            return Err(AppError::InvalidInput("relation already exists".into()));
        }
        let now = now_iso();
        let rtype = input.relation_type.as_str().to_string();
        let rev_type = input.relation_type.reverse().as_str().to_string();
        let id = new_uuid();
        let rev_id = new_uuid();

        // 插入正向关系
        self.conn.execute(
            "INSERT INTO relations (id, person_id, related_person_id, relation_type, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, input.person_id, input.related_person_id, rtype, now],
        )?;
        // 插入反向关系
        self.conn.execute(
            "INSERT INTO relations (id, person_id, related_person_id, relation_type, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![rev_id, input.related_person_id, input.person_id, rev_type, now],
        )?;

        // 返回该人的所有关系（含新增的）
        self.list_relations_by_person(&input.person_id)
    }

    fn row_to_relation(row: &rusqlite::Row<'_>) -> rusqlite::Result<Relation> {
        Ok(Relation {
            id: row.get(0)?,
            person_id: row.get(1)?,
            related_person_id: row.get(2)?,
            relation_type: row.get(3)?,
            created_at: row.get(4)?,
            related_person_name: row.get(5)?,
            related_person_tomb_id: row.get(6)?,
        })
    }

    /// 列出某人的所有关系（含双向），JOIN 填充关联人物姓名和墓 ID
    pub fn list_relations_by_person(&self, person_id: &str) -> Result<Vec<Relation>> {
        let mut stmt = self.conn.prepare(
            "SELECT r.id, r.person_id, r.related_person_id, r.relation_type, r.created_at,
                    p.name AS related_person_name, p.tomb_id AS related_person_tomb_id
             FROM relations r
             JOIN persons p ON p.id = r.related_person_id
             WHERE r.person_id = ?1
             ORDER BY r.relation_type, p.name",
        )?;
        let rows = stmt.query_map(params![person_id], Self::row_to_relation)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 删除单条关系及其反向关系
    pub fn delete_relation(&self, id: &str) -> Result<()> {
        // 先查出关系信息以便删除反向
        let rel = self.conn.query_row(
            "SELECT id, person_id, related_person_id, relation_type, created_at FROM relations WHERE id = ?1",
            params![id],
            |row| {
                Ok(Relation {
                    id: row.get(0)?,
                    person_id: row.get(1)?,
                    related_person_id: row.get(2)?,
                    relation_type: row.get(3)?,
                    created_at: row.get(4)?,
                    related_person_name: None,
                    related_person_tomb_id: None,
                })
            },
        ).optional()?
        .ok_or_else(|| AppError::NotFound(format!("relation {id}")))?;

        // 删除正向关系
        let n = self.conn.execute("DELETE FROM relations WHERE id = ?1", params![id])?;
        // 删除反向关系
        self.conn.execute(
            "DELETE FROM relations WHERE person_id = ?1 AND related_person_id = ?2 AND relation_type = ?3",
            params![rel.related_person_id, rel.person_id, rel.relation_type],
        )?;
        if n == 0 {
            return Err(AppError::NotFound(format!("relation {id}")));
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // 搜索
    // -----------------------------------------------------------------------
    /// 按关键字搜索墓地（名称/地址/描述模糊匹配）
    pub fn search_tombs(&self, keyword: &str) -> Result<Vec<Tomb>> {
        let pattern = format!("%{}%", keyword);
        let mut stmt = self.conn.prepare(
            "SELECT id, name, latitude, longitude, address, description, group_id, family_id, created_at, updated_at
             FROM tombs
             WHERE name LIKE ?1 OR address LIKE ?1 OR description LIKE ?1
             ORDER BY name",
        )?;
        let rows = stmt.query_map(params![pattern], Self::row_to_tomb)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 按关键字搜索人物（姓名/生平模糊匹配），返回对应墓地
    pub fn search_persons(&self, keyword: &str) -> Result<Vec<Tomb>> {
        let pattern = format!("%{}%", keyword);
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT t.id, t.name, t.latitude, t.longitude, t.address, t.description, t.group_id, t.family_id, t.created_at, t.updated_at
             FROM persons p JOIN tombs t ON p.tomb_id = t.id
             WHERE p.name LIKE ?1 OR p.biography LIKE ?1
             ORDER BY t.name",
        )?;
        let rows = stmt.query_map(params![pattern], Self::row_to_tomb)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 按关键字搜索家族（名称/祖籍模糊匹配）
    pub fn search_families(&self, keyword: &str) -> Result<Vec<Family>> {
        let pattern = format!("%{}%", keyword);
        let mut stmt = self.conn.prepare(
            "SELECT id, name, description, origin, created_at, updated_at
             FROM families WHERE name LIKE ?1 OR origin LIKE ?1 ORDER BY name",
        )?;
        let rows = stmt.query_map(params![pattern], |row| {
            Ok(Family {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                origin: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    // -----------------------------------------------------------------------
    // 祭祀/忌日提醒
    // -----------------------------------------------------------------------
    /// 列出未来 `days` 天内的提醒（忌日 + 传统节日）。
    /// 忌日按人物 death_date 的农历日期计算未来三年的公历日期。
    pub fn list_reminders(&self, days: u32) -> Result<Vec<Reminder>> {
        use lunar_lite::{lunar_to_solar, solar_to_lunar, LunarDate, SolarDate};

        let today = chrono::Local::now().date_naive();
        let end_date = today + chrono::Duration::days(days as i64);
        let mut reminders: Vec<Reminder> = Vec::new();

        // 1. 人物忌日
        let mut stmt = self.conn.prepare(
            "SELECT id, tomb_id, name, title, death_date FROM persons WHERE death_date IS NOT NULL AND death_date != ''",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;

        for row in rows {
            let (id, tomb_id, name, title, death_date) = row?;
            let solar = parse_naive_date(&death_date);
            let solar = match solar {
                Some(d) => d,
                None => continue,
            };
            let lunar = match solar_to_lunar(SolarDate {
                year: solar.year(),
                month: solar.month() as u8,
                day: solar.day() as u8,
            }) {
                Ok(l) => l,
                Err(_) => continue,
            };

            let base_year = today.year();
            for year_offset in 0..=2 {
                let target_lunar_year = base_year + year_offset;
                let target_solar = match lunar_to_solar(LunarDate {
                    year: target_lunar_year,
                    month: lunar.month,
                    day: lunar.day,
                    is_leap_month: lunar.is_leap_month,
                }) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let Some(target_date) = chrono::NaiveDate::from_ymd_opt(
                    target_solar.year,
                    target_solar.month as u32,
                    target_solar.day as u32,
                ) else {
                    continue;
                };
                if target_date < today || target_date > end_date {
                    continue;
                }
                let days_until = (target_date - today).num_days();
                reminders.push(Reminder {
                    id: new_uuid(),
                    reminder_type: ReminderType::DeathAnniversary,
                    title: format!("{}{} 忌日", title.as_deref().unwrap_or(""), name),
                    date: target_date.to_string(),
                    lunar_date: format!("农历{}月{}", lunar.month, lunar_day_name(lunar.day)),
                    person_id: Some(id.clone()),
                    tomb_id: Some(tomb_id.clone()),
                    days_until,
                });
            }
        }

        // 2. 传统节日（清明、重阳）
        add_festival_reminders(&mut reminders, today, end_date);

        reminders.sort_by_key(|r| (r.date.clone(), r.days_until));
        Ok(reminders)
    }
}

fn parse_naive_date(s: &str) -> Option<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

fn lunar_day_name(day: u8) -> String {
    match day {
        1 => "初一".into(),
        2 => "初二".into(),
        3 => "初三".into(),
        4 => "初四".into(),
        5 => "初五".into(),
        6 => "初六".into(),
        7 => "初七".into(),
        8 => "初八".into(),
        9 => "初九".into(),
        10 => "初十".into(),
        11 => "十一".into(),
        12 => "十二".into(),
        13 => "十三".into(),
        14 => "十四".into(),
        15 => "十五".into(),
        16 => "十六".into(),
        17 => "十七".into(),
        18 => "十八".into(),
        19 => "十九".into(),
        20 => "二十".into(),
        21 => "廿一".into(),
        22 => "廿二".into(),
        23 => "廿三".into(),
        24 => "廿四".into(),
        25 => "廿五".into(),
        26 => "廿六".into(),
        27 => "廿七".into(),
        28 => "廿八".into(),
        29 => "廿九".into(),
        30 => "三十".into(),
        d => format!("初{d}"),
    }
}

fn add_festival_reminders(reminders: &mut Vec<Reminder>, today: chrono::NaiveDate, end_date: chrono::NaiveDate) {
    use lunar_lite::{lunar_to_solar, LunarDate};

    let base_year = today.year();
    for year_offset in 0..=2 {
        let year = base_year + year_offset;

        // 清明：通常落在 4 月 4 日或 5 日。这里按 4 月 5 日简化处理（误差一天可接受）。
        if let Some(date) = chrono::NaiveDate::from_ymd_opt(year, 4, 5) {
            if date >= today && date <= end_date {
                reminders.push(Reminder {
                    id: new_uuid(),
                    reminder_type: ReminderType::Festival,
                    title: "清明节".into(),
                    date: date.to_string(),
                    lunar_date: "公历 4 月 5 日".into(),
                    person_id: None,
                    tomb_id: None,
                    days_until: (date - today).num_days(),
                });
            }
        }

        // 重阳：农历九月初九
        if let Ok(solar) = lunar_to_solar(LunarDate {
            year,
            month: 9,
            day: 9,
            is_leap_month: false,
        }) {
            if let Some(date) = chrono::NaiveDate::from_ymd_opt(solar.year, solar.month as u32, solar.day as u32) {
                if date >= today && date <= end_date {
                    reminders.push(Reminder {
                        id: new_uuid(),
                        reminder_type: ReminderType::Festival,
                        title: "重阳节".into(),
                        date: date.to_string(),
                        lunar_date: "农历九月初九".into(),
                        person_id: None,
                        tomb_id: None,
                        days_until: (date - today).num_days(),
                    });
                }
            }
        }

        // 春节：农历正月初一
        if let Ok(solar) = lunar_to_solar(LunarDate {
            year,
            month: 1,
            day: 1,
            is_leap_month: false,
        }) {
            if let Some(date) =
                chrono::NaiveDate::from_ymd_opt(solar.year, solar.month as u32, solar.day as u32)
            {
                if date >= today && date <= end_date {
                    reminders.push(Reminder {
                        id: new_uuid(),
                        reminder_type: ReminderType::Festival,
                        title: "春节".into(),
                        date: date.to_string(),
                        lunar_date: "农历正月初一".into(),
                        person_id: None,
                        tomb_id: None,
                        days_until: (date - today).num_days(),
                    });
                }
            }
        }

        // 中元节：农历七月十五
        if let Ok(solar) = lunar_to_solar(LunarDate {
            year,
            month: 7,
            day: 15,
            is_leap_month: false,
        }) {
            if let Some(date) =
                chrono::NaiveDate::from_ymd_opt(solar.year, solar.month as u32, solar.day as u32)
            {
                if date >= today && date <= end_date {
                    reminders.push(Reminder {
                        id: new_uuid(),
                        reminder_type: ReminderType::Festival,
                        title: "中元节".into(),
                        date: date.to_string(),
                        lunar_date: "农历七月十五".into(),
                        person_id: None,
                        tomb_id: None,
                        days_until: (date - today).num_days(),
                    });
                }
            }
        }

        // 寒衣节：农历十月初一
        if let Ok(solar) = lunar_to_solar(LunarDate {
            year,
            month: 10,
            day: 1,
            is_leap_month: false,
        }) {
            if let Some(date) =
                chrono::NaiveDate::from_ymd_opt(solar.year, solar.month as u32, solar.day as u32)
            {
                if date >= today && date <= end_date {
                    reminders.push(Reminder {
                        id: new_uuid(),
                        reminder_type: ReminderType::Festival,
                        title: "寒衣节".into(),
                        date: date.to_string(),
                        lunar_date: "农历十月初一".into(),
                        person_id: None,
                        tomb_id: None,
                        days_until: (date - today).num_days(),
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> TombKeeperDb {
        TombKeeperDb::in_memory().expect("in-memory db")
    }

    #[test]
    fn family_crud() {
        let db = setup();
        let f = db
            .create_family(NewFamily {
                name: "王氏家族".into(),
                description: Some("宗族档案".into()),
                origin: Some("山西洪洞".into()),
            })
            .unwrap();
        assert_eq!(db.get_family(&f.id).unwrap().name, "王氏家族");
        assert_eq!(db.list_families().unwrap().len(), 1);

        let updated = db
            .update_family(&f.id, Some("王氏族谱".into()), None, None)
            .unwrap();
        assert_eq!(updated.name, "王氏族谱");
        assert_eq!(updated.origin.as_deref(), Some("山西洪洞"));

        db.delete_family(&f.id).unwrap();
        assert!(db.get_family(&f.id).is_err());
    }

    #[test]
    fn tomb_group_belongs_to_family() {
        let db = setup();
        let f = db
            .create_family(NewFamily { name: "李氏".into(), description: None, origin: None })
            .unwrap();
        let g = db
            .create_tomb_group(NewTombGroup {
                family_id: f.id.clone(),
                name: "祖坟区".into(),
                description: Some("山腰老坟".into()),
            })
            .unwrap();
        assert_eq!(db.list_groups_by_family(&f.id).unwrap().len(), 1);
        assert_eq!(db.list_groups_by_family(&g.id).unwrap().len(), 0);

        // 删除家族应级联删除墓组
        db.delete_family(&f.id).unwrap();
        assert!(db.list_groups_by_family(&f.id).unwrap().is_empty());
    }

    #[test]
    fn tomb_crud_with_relations() {
        let db = setup();
        let f = db
            .create_family(NewFamily { name: "赵氏".into(), description: None, origin: None })
            .unwrap();
        let g = db
            .create_tomb_group(NewTombGroup {
                family_id: f.id.clone(),
                name: "东区".into(),
                description: None,
            })
            .unwrap();

        let t = db
            .create_tomb(NewTomb {
                name: "赵公明墓".into(),
                latitude: 34.21,
                longitude: 108.94,
                address: Some("西安东郊".into()),
                description: Some("明代墓".into()),
                group_id: Some(g.id.clone()),
                family_id: Some(f.id.clone()),
            })
            .unwrap();
        assert_eq!(db.list_tombs_by_family(&f.id).unwrap().len(), 1);
        assert_eq!(db.list_tombs_by_group(&g.id).unwrap().len(), 1);

        // 非法经纬度
        let bad = db.create_tomb(NewTomb {
            name: "bad".into(),
            latitude: 100.0,
            longitude: 0.0,
            address: None,
            description: None,
            group_id: None,
            family_id: None,
        });
        assert!(bad.is_err());

        // 关联不存在家族应报错
        let bad_family = db.create_tomb(NewTomb {
            name: "bad2".into(),
            latitude: 0.0,
            longitude: 0.0,
            address: None,
            description: None,
            group_id: None,
            family_id: Some("not-exist".into()),
        });
        assert!(bad_family.is_err());
        drop(t);
    }

    #[test]
    fn person_and_photos() {
        let db = setup();
        let t = db
            .create_tomb(NewTomb {
                name: "合葬墓".into(),
                latitude: 30.5,
                longitude: 114.3,
                address: None,
                description: None,
                group_id: None,
                family_id: None,
            })
            .unwrap();

        let p1 = db
            .create_person(NewPerson {
                tomb_id: t.id.clone(),
                name: "张爷爷".into(),
                title: Some("祖父".into()),
                birth_date: Some("1900-01-01".into()),
                death_date: Some("1978-05-20".into()),
                biography: Some("一生务农".into()),
                epitaph: None,
                spouse: Some("张奶奶".into()),
                is_joint_burial: true,
                children: Some("张大、张二、张三".into()),
                order_index: 1,
            })
            .unwrap();
        assert_eq!(p1.spouse.as_deref(), Some("张奶奶"));
        assert!(p1.is_joint_burial);
        assert_eq!(p1.children.as_deref(), Some("张大、张二、张三"));

        db.create_person(NewPerson {
            tomb_id: t.id.clone(),
            name: "张奶奶".into(),
            title: Some("祖母".into()),
            birth_date: None,
            death_date: None,
            biography: None,
            epitaph: None,
            spouse: None,
            is_joint_burial: true,
            children: None,
            order_index: 2,
        })
        .unwrap();

        let persons = db.list_persons_by_tomb(&t.id).unwrap();
        assert_eq!(persons.len(), 2);
        assert_eq!(persons[0].order_index, 1);
        assert_eq!(persons[1].order_index, 2);

        // 测试 update_person
        let updated = db
            .update_person(
                &p1.id,
                None,
                None,
                None,
                None,
                None,
                None,
                Some("张氏".into()),
                Some(false),
                None,
                Some(5),
            )
            .unwrap();
        assert_eq!(updated.spouse.as_deref(), Some("张氏"));
        assert!(!updated.is_joint_burial);
        assert_eq!(updated.order_index, 5);

        db.add_photo(NewPhoto {
            entity_type: EntityType::Tomb,
            entity_id: t.id.clone(),
            file_path: "/photos/tomb1.jpg".into(),
            caption: Some("墓碑".into()),
            is_cover: true,
        })
        .unwrap();
        let photos = db.list_photos_by_entity(EntityType::Tomb, &t.id).unwrap();
        assert_eq!(photos.len(), 1);
        assert!(photos[0].is_cover);
    }

    #[test]
    fn search_works() {
        let db = setup();
        let f = db
            .create_family(NewFamily {
                name: "陈家".into(),
                description: None,
                origin: Some("福建".into()),
            })
            .unwrap();
        let t = db
            .create_tomb(NewTomb {
                name: "陈公墓".into(),
                latitude: 24.5,
                longitude: 118.1,
                address: Some("厦门集美".into()),
                description: Some("清代墓".into()),
                group_id: None,
                family_id: Some(f.id.clone()),
            })
            .unwrap();
        db.create_person(NewPerson {
            tomb_id: t.id.clone(),
            name: "陈永福".into(),
            title: None,
            birth_date: None,
            death_date: None,
            biography: Some("晚清举人".into()),
            epitaph: None,
            spouse: None,
            is_joint_burial: false,
            children: None,
            order_index: 1,
        })
        .unwrap();

        assert_eq!(db.search_tombs("陈公").unwrap().len(), 1);
        assert_eq!(db.search_tombs("集美").unwrap().len(), 1);
        assert_eq!(db.search_persons("永福").unwrap().len(), 1);
        assert_eq!(db.search_persons("举人").unwrap().len(), 1);
        assert_eq!(db.search_families("福建").unwrap().len(), 1);
        assert_eq!(db.search_tombs("不存在").unwrap().len(), 0);
    }
}