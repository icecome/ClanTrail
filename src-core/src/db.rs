use crate::error::{AppError, Result};
use crate::models::*;
use chrono::Datelike;
use rusqlite::{params, Connection, OptionalExtension};

/// 内嵌 SQL 迁移（refinery），从 src-core/migrations 编译期嵌入
mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("migrations");
}

/// 本地 SQLite 数据库封装，离线优先的数据源
pub struct ClanTrailDb {
    conn: Connection,
}

impl ClanTrailDb {
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
        } else {
            conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
        }

        let mut db = Self { conn };
        db.initialize()?;
        Ok(db)
    }

    /// 打开内存数据库（测试用）
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        let mut db = Self { conn };
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
    /// 导入备份后调用：自动跑迁移，兼容旧版本备份（refinery 幂等）。
    pub fn reopen(&mut self, path: &str) -> Result<()> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
        self.conn = conn;
        self.initialize()?;
        Ok(())
    }

    /// 创建一致的数据库快照文件（使用 VACUUM INTO，SQLite 3.27+）。
    /// 导出备份时使用此方法，避免直接拷贝运行中库带来的不一致问题。
    pub fn create_backup_snapshot(&self, dest_path: &str) -> Result<()> {
        // VACUUM INTO 不支持参数化，但 dest_path 来源于受控路径（临时目录），安全。
        let sql = format!("VACUUM INTO '{}'", dest_path.replace('\'', "''"));
        self.conn.execute_batch(&sql)?;
        Ok(())
    }

    fn initialize(&mut self) -> Result<()> {
        // 兼容老库：members 表早期版本缺少 spouse/is_joint_burial/children 列，
        // 先确保这些列存在，使 refinery V1（CREATE IF NOT EXISTS）对已有表安全。
        self.migrate_add_columns()?;

        // 运行 refinery 迁移（V1 初始 schema、V2 同步列、V3 Edge.side 等）
        embedded::migrations::runner().run(&mut self.conn)?;
        Ok(())
    }

    /// 轻量级迁移：仅处理 members 表新增列（老库兼容）
    fn migrate_add_columns(&self) -> Result<()> {
        // 新库由 refinery V1 建全表；仅当 members 表已存在（老库）时才需补列
        let table_exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='members')",
            [],
            |row| row.get(0),
        )?;
        if !table_exists {
            return Ok(());
        }
        // pragma table_info 返回 (cid, name, type, notnull, dflt_value, pk)
        let mut stmt = self.conn
            .prepare("PRAGMA table_info(members)")?;
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
                let sql = format!("ALTER TABLE members ADD COLUMN {col} {decl}");
                self.conn.execute_batch(&sql)?;
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Clan 家族
    // -----------------------------------------------------------------------
    pub fn create_clan(&self, input: NewClan) -> Result<Clan> {
        let now = now_iso();
        let clan = Clan {
            id: new_uuid(),
            name: input.name,
            description: input.description,
            origin: input.origin,
            created_at: now.clone(),
            updated_at: now,
            version: 1,
            deleted: false,
        };
        self.conn.execute(
            "INSERT INTO clans (id, name, description, origin, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                clan.id,
                clan.name,
                clan.description,
                clan.origin,
                clan.created_at,
                clan.updated_at
            ],
        )?;
        Ok(clan)
    }

    pub fn get_clan(&self, id: &str) -> Result<Clan> {
        self.conn
            .query_row(
                "SELECT id, name, description, origin, created_at, updated_at, version, deleted
                 FROM clans WHERE id = ?1",
                params![id],
                |row| {
                    Ok(Clan {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        description: row.get(2)?,
                        origin: row.get(3)?,
                        created_at: row.get(4)?,
                        updated_at: row.get(5)?,
                        version: row.get(6)?,
                        deleted: row.get(7)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("clan {id}")))
    }

    pub fn list_clans(&self) -> Result<Vec<Clan>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, description, origin, created_at, updated_at, version, deleted
             FROM clans ORDER BY name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Clan {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                origin: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                version: row.get(6)?,
                deleted: row.get(7)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn update_clan(
        &self,
        id: &str,
        name: Option<String>,
        description: Option<String>,
        origin: Option<String>,
    ) -> Result<Clan> {
        let current = self.get_clan(id)?;
        let new_name = name.unwrap_or(current.name);
        let new_desc = description.or(current.description);
        let new_origin = origin.or(current.origin);
        let now = now_iso();
        self.conn.execute(
            "UPDATE clans SET name = ?1, description = ?2, origin = ?3, updated_at = ?4, version = version + 1 WHERE id = ?5",
            params![new_name, new_desc, new_origin, now, id],
        )?;
        self.get_clan(id)
    }

    /// 删除家族并级联清理其子级（墓组、墓地、墓主、关系、照片）。
    /// 墓地归属的墓组仅解除关联，不删除墓地本身。
    pub fn delete_clan(&self, id: &str) -> Result<()> {
        // 不使用事务（self.conn 不可变借用），依次执行 DELETE
        // 墓组下的墓地解除墓组归属（墓地仍归家族所有）
        self.conn.execute(
            "UPDATE graves SET burial_group_id = NULL WHERE burial_group_id IN (SELECT id FROM burial_groups WHERE clan_id = ?1)",
            params![id],
        )?;
        // 墓地照片
        self.conn.execute(
            "DELETE FROM images WHERE entity_type = 'grave' AND entity_id IN (SELECT id FROM graves WHERE clan_id = ?1)",
            params![id],
        )?;
        // 墓主照片（含在世成员照片）
        self.conn.execute(
            "DELETE FROM images WHERE entity_type = 'member' AND entity_id IN (SELECT id FROM members WHERE grave_id IN (SELECT id FROM graves WHERE clan_id = ?1) OR clan_id = ?1)",
            params![id],
        )?;
        // 墓主关系（含指向被删墓主的反向关系）
        self.conn.execute(
            "DELETE FROM edges WHERE member_id IN (SELECT id FROM members WHERE grave_id IN (SELECT id FROM graves WHERE clan_id = ?1) OR clan_id = ?1) OR related_member_id IN (SELECT id FROM members WHERE grave_id IN (SELECT id FROM graves WHERE clan_id = ?1) OR clan_id = ?1)",
            params![id],
        )?;
        // 墓主 + 在世成员
        self.conn.execute(
            "DELETE FROM members WHERE grave_id IN (SELECT id FROM graves WHERE clan_id = ?1) OR clan_id = ?1",
            params![id],
        )?;
        // 墓地
        self.conn.execute("DELETE FROM graves WHERE clan_id = ?1", params![id])?;
        // 墓组
        self.conn.execute("DELETE FROM burial_groups WHERE clan_id = ?1", params![id])?;
        // 家族照片
        self.conn.execute("DELETE FROM images WHERE entity_type = 'clan' AND entity_id = ?1", params![id])?;
        // 家族
        let n = self.conn.execute("DELETE FROM clans WHERE id = ?1", params![id])?;
        if n == 0 {
            return Err(AppError::NotFound(format!("clan {id}")));
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // BurialGroup 墓组
    // -----------------------------------------------------------------------
    pub fn create_burial_group(&self, input: NewBurialGroup) -> Result<BurialGroup> {
        // 校验家族存在
        self.get_clan(&input.clan_id)?;
        let now = now_iso();
        let group = BurialGroup {
            id: new_uuid(),
            clan_id: input.clan_id,
            name: input.name,
            description: input.description,
            created_at: now.clone(),
            updated_at: now,
        };
        self.conn.execute(
            "INSERT INTO burial_groups (id, clan_id, name, description, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                group.id,
                group.clan_id,
                group.name,
                group.description,
                group.created_at,
                group.updated_at
            ],
        )?;
        Ok(group)
    }

    fn row_to_burial_group(row: &rusqlite::Row<'_>) -> rusqlite::Result<BurialGroup> {
        Ok(BurialGroup {
            id: row.get(0)?,
            clan_id: row.get(1)?,
            name: row.get(2)?,
            description: row.get(3)?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    }

    pub fn get_burial_group(&self, id: &str) -> Result<BurialGroup> {
        self.conn
            .query_row(
                "SELECT id, clan_id, name, description, created_at, updated_at
                 FROM burial_groups WHERE id = ?1",
                params![id],
                Self::row_to_burial_group,
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("burial_group {id}")))
    }

    pub fn list_groups_by_clan(&self, clan_id: &str) -> Result<Vec<BurialGroup>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, clan_id, name, description, created_at, updated_at
             FROM burial_groups WHERE clan_id = ?1 ORDER BY name",
        )?;
        let rows = stmt.query_map(params![clan_id], Self::row_to_burial_group)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 删除墓组：墓地下属归家族所有，仅解除其墓组归属，不删除墓地本身。
    pub fn delete_burial_group(&self, id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE graves SET burial_group_id = NULL WHERE burial_group_id = ?1",
            params![id],
        )?;
        let n = self.conn.execute("DELETE FROM burial_groups WHERE id = ?1", params![id])?;
        if n == 0 {
            return Err(AppError::NotFound(format!("burial_group {id}")));
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Grave 单墓地
    // -----------------------------------------------------------------------
    pub fn create_grave(&self, input: NewGrave) -> Result<Grave> {
        if input.latitude < -90.0 || input.latitude > 90.0 {
            return Err(AppError::InvalidInput("latitude out of range".into()));
        }
        if input.longitude < -180.0 || input.longitude > 180.0 {
            return Err(AppError::InvalidInput("longitude out of range".into()));
        }
        if let Some(fid) = &input.clan_id {
            self.get_clan(fid)?;
        }
        if let Some(gid) = &input.burial_group_id {
            self.get_burial_group(gid)?;
        }
        let now = now_iso();
        let grave = Grave {
            id: new_uuid(),
            name: input.name,
            latitude: input.latitude,
            longitude: input.longitude,
            address: input.address,
            description: input.description,
            burial_group_id: input.burial_group_id,
            clan_id: input.clan_id,
            created_at: now.clone(),
            updated_at: now,
            version: 1,
            deleted: false,
        };
        self.conn.execute(
            "INSERT INTO graves (id, name, latitude, longitude, address, description, burial_group_id, clan_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                grave.id,
                grave.name,
                grave.latitude,
                grave.longitude,
                grave.address,
                grave.description,
                grave.burial_group_id,
                grave.clan_id,
                grave.created_at,
                grave.updated_at
            ],
        )?;
        Ok(grave)
    }

    fn row_to_grave(row: &rusqlite::Row<'_>) -> rusqlite::Result<Grave> {
        Ok(Grave {
            id: row.get(0)?,
            name: row.get(1)?,
            latitude: row.get(2)?,
            longitude: row.get(3)?,
            address: row.get(4)?,
            description: row.get(5)?,
            burial_group_id: row.get(6)?,
            clan_id: row.get(7)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
            version: row.get(10)?,
            deleted: row.get(11)?,
        })
    }

    pub fn get_grave(&self, id: &str) -> Result<Grave> {
        self.conn
            .query_row(
                "SELECT id, name, latitude, longitude, address, description, burial_group_id, clan_id, created_at, updated_at, version, deleted
                 FROM graves WHERE id = ?1",
                params![id],
                Self::row_to_grave,
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("grave {id}")))
    }

    pub fn list_graves(&self) -> Result<Vec<Grave>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, latitude, longitude, address, description, burial_group_id, clan_id, created_at, updated_at, version, deleted
             FROM graves ORDER BY name",
        )?;
        let rows = stmt.query_map([], Self::row_to_grave)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn list_graves_by_clan(&self, clan_id: &str) -> Result<Vec<Grave>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, latitude, longitude, address, description, burial_group_id, clan_id, created_at, updated_at, version, deleted
             FROM graves WHERE clan_id = ?1 ORDER BY name",
        )?;
        let rows = stmt.query_map(params![clan_id], Self::row_to_grave)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn list_graves_by_group(&self, burial_group_id: &str) -> Result<Vec<Grave>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, latitude, longitude, address, description, burial_group_id, clan_id, created_at, updated_at, version, deleted
             FROM graves WHERE burial_group_id = ?1 ORDER BY name",
        )?;
        let rows = stmt.query_map(params![burial_group_id], Self::row_to_grave)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn update_grave(
        &self,
        id: &str,
        name: Option<String>,
        latitude: Option<f64>,
        longitude: Option<f64>,
        address: Option<String>,
        description: Option<String>,
        burial_group_id: Option<String>,
        clan_id: Option<String>,
    ) -> Result<Grave> {
        let current = self.get_grave(id)?;
        let new_name = name.unwrap_or(current.name);
        let new_lat = latitude.unwrap_or(current.latitude);
        let new_lng = longitude.unwrap_or(current.longitude);
        let new_addr = address.or(current.address);
        let new_desc = description.or(current.description);
        let new_group = burial_group_id.or(current.burial_group_id);
        let new_clan = clan_id.or(current.clan_id);
        let now = now_iso();
        self.conn.execute(
            "UPDATE graves SET name = ?1, latitude = ?2, longitude = ?3, address = ?4, description = ?5,
             burial_group_id = ?6, clan_id = ?7, updated_at = ?8, version = version + 1 WHERE id = ?9",
            params![new_name, new_lat, new_lng, new_addr, new_desc, new_group, new_clan, now, id],
        )?;
        self.get_grave(id)
    }

    /// 删除墓地并级联清理其子级（墓主、关系、照片）。
    pub fn delete_grave(&self, id: &str) -> Result<()> {
        // 墓主照片
        self.conn.execute(
            "DELETE FROM images WHERE entity_type = 'member' AND entity_id IN (SELECT id FROM members WHERE grave_id = ?1)",
            params![id],
        )?;
        // 墓主关系（含指向被删墓主的反向关系）
        self.conn.execute(
            "DELETE FROM edges WHERE member_id IN (SELECT id FROM members WHERE grave_id = ?1) OR related_member_id IN (SELECT id FROM members WHERE grave_id = ?1)",
            params![id],
        )?;
        // 墓主
        self.conn.execute("DELETE FROM members WHERE grave_id = ?1", params![id])?;
        // 墓地照片
        self.conn.execute(
            "DELETE FROM images WHERE entity_type = 'grave' AND entity_id = ?1",
            params![id],
        )?;
        // 墓地
        let n = self.conn.execute("DELETE FROM graves WHERE id = ?1", params![id])?;
        if n == 0 {
            return Err(AppError::NotFound(format!("grave {id}")));
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Member 墓主
    // -----------------------------------------------------------------------
    pub fn create_member(&self, input: NewMember) -> Result<Member> {
        // 校验归属：有墓则校验墓存在；无墓（在世）则校验族存在
        if let Some(gid) = &input.grave_id {
            self.get_grave(gid)?;
        }
        if let Some(cid) = &input.clan_id {
            self.get_clan(cid)?;
        }
        let now = now_iso();
        let member = Member {
            id: new_uuid(),
            grave_id: input.grave_id,
            clan_id: input.clan_id,
            name: input.name,
            title: input.title,
            birth_date: input.birth_date,
            death_date: input.death_date,
            biography: input.biography,
            epitaph: input.epitaph,
            spouse: input.spouse,
            is_joint_burial: input.is_joint_burial,
            children: input.children,
            is_alive: input.is_alive,
            order_index: input.order_index,
            created_at: now.clone(),
            updated_at: now,
            version: 1,
            deleted: false,
        };
        self.conn.execute(
            "INSERT INTO members (id, grave_id, clan_id, name, title, birth_date, death_date, biography, epitaph, spouse, is_joint_burial, children, is_alive, order_index, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                member.id,
                member.grave_id,
                member.clan_id,
                member.name,
                member.title,
                member.birth_date,
                member.death_date,
                member.biography,
                member.epitaph,
                member.spouse,
                member.is_joint_burial as i32,
                member.children,
                member.is_alive as i32,
                member.order_index,
                member.created_at,
                member.updated_at
            ],
        )?;
        Ok(member)
    }

    fn row_to_member(row: &rusqlite::Row<'_>) -> rusqlite::Result<Member> {
        Ok(Member {
            id: row.get(0)?,
            grave_id: row.get(1)?,
            clan_id: row.get(2)?,
            name: row.get(3)?,
            title: row.get(4)?,
            birth_date: row.get(5)?,
            death_date: row.get(6)?,
            biography: row.get(7)?,
            epitaph: row.get(8)?,
            spouse: row.get(9)?,
            is_joint_burial: {
                let v: i64 = row.get(10)?;
                v != 0
            },
            children: row.get(11)?,
            is_alive: {
                let v: i64 = row.get(12)?;
                v != 0
            },
            order_index: row.get(13)?,
            created_at: row.get(14)?,
            updated_at: row.get(15)?,
            version: row.get(16)?,
            deleted: row.get(17)?,
        })
    }

    pub fn list_members_by_grave(&self, grave_id: &str) -> Result<Vec<Member>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, grave_id, clan_id, name, title, birth_date, death_date, biography, epitaph, spouse, is_joint_burial, children, is_alive, order_index, created_at, updated_at, version, deleted
             FROM members WHERE grave_id = ?1 ORDER BY order_index, name",
        )?;
        let rows = stmt.query_map(params![grave_id], Self::row_to_member)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn get_member(&self, id: &str) -> Result<Member> {
        self.conn
            .query_row(
                "SELECT id, grave_id, clan_id, name, title, birth_date, death_date, biography, epitaph, spouse, is_joint_burial, children, is_alive, order_index, created_at, updated_at, version, deleted
                 FROM members WHERE id = ?1",
                params![id],
                Self::row_to_member,
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("member {id}")))
    }

    pub fn update_member(
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
        clan_id: Option<String>,
        is_alive: Option<bool>,
    ) -> Result<Member> {
        let current = self.get_member(id)?;
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
        let new_clan = clan_id.or(current.clan_id);
        let new_alive = is_alive.unwrap_or(current.is_alive);
        let now = now_iso();
        self.conn.execute(
            "UPDATE members SET name = ?1, title = ?2, birth_date = ?3, death_date = ?4, biography = ?5,
             epitaph = ?6, spouse = ?7, is_joint_burial = ?8, children = ?9, order_index = ?10,
             clan_id = ?11, is_alive = ?12, updated_at = ?13, version = version + 1
             WHERE id = ?14",
            params![
                new_name, new_title, new_birth, new_death, new_bio, new_epitaph,
                new_spouse, new_joint as i32, new_children, new_order,
                new_clan, new_alive as i32, now, id
            ],
        )?;
        self.get_member(id)
    }

    /// 删除墓主并清理其照片与关系（含指向该墓主的反向关系）。
    pub fn delete_member(&self, id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM images WHERE entity_type = 'member' AND entity_id = ?1",
            params![id],
        )?;
        self.conn.execute(
            "DELETE FROM edges WHERE member_id = ?1 OR related_member_id = ?1",
            params![id],
        )?;
        let n = self.conn.execute("DELETE FROM members WHERE id = ?1", params![id])?;
        if n == 0 {
            return Err(AppError::NotFound(format!("member {id}")));
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Image 照片
    // -----------------------------------------------------------------------
    pub fn add_image(&self, input: NewImage) -> Result<Image> {
        let image = Image {
            id: new_uuid(),
            entity_type: input.entity_type.as_str().to_string(),
            entity_id: input.entity_id,
            file_path: input.file_path,
            caption: input.caption,
            is_cover: input.is_cover,
            created_at: now_iso(),
            version: 1,
            deleted: false,
        };
        self.conn.execute(
            "INSERT INTO images (id, entity_type, entity_id, file_path, caption, is_cover, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                image.id,
                image.entity_type,
                image.entity_id,
                image.file_path,
                image.caption,
                image.is_cover,
                image.created_at
            ],
        )?;
        Ok(image)
    }

    /// 按 ID 取单张照片（删除时用于定位物理文件）
    pub fn get_image(&self, id: &str) -> Result<Image> {
        self.conn
            .query_row(
                "SELECT id, entity_type, entity_id, file_path, caption, is_cover, created_at, version, deleted
                 FROM images WHERE id = ?1",
                params![id],
                |row| {
                    Ok(Image {
                        id: row.get(0)?,
                        entity_type: row.get(1)?,
                        entity_id: row.get(2)?,
                        file_path: row.get(3)?,
                        caption: row.get(4)?,
                        is_cover: row.get(5)?,
                        created_at: row.get(6)?,
                        version: row.get(7)?,
                        deleted: row.get(8)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("image {id}")))
    }

    pub fn list_images_by_entity(&self, entity_type: EntityType, entity_id: &str) -> Result<Vec<Image>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, entity_type, entity_id, file_path, caption, is_cover, created_at, version, deleted
             FROM images WHERE entity_type = ?1 AND entity_id = ?2 ORDER BY created_at",
        )?;
        let rows = stmt.query_map(params![entity_type.as_str(), entity_id], |row| {
            Ok(Image {
                id: row.get(0)?,
                entity_type: row.get(1)?,
                entity_id: row.get(2)?,
                file_path: row.get(3)?,
                caption: row.get(4)?,
                is_cover: row.get(5)?,
                created_at: row.get(6)?,
                version: row.get(7)?,
                deleted: row.get(8)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn delete_image(&self, id: &str) -> Result<()> {
        let n = self.conn.execute("DELETE FROM images WHERE id = ?1", params![id])?;
        if n == 0 {
            return Err(AppError::NotFound(format!("image {id}")));
        }
        Ok(())
    }

    /// 更新照片封面标记（原子操作：单条 SQL 完成同实体封面切换）
    pub fn set_image_cover(&self, id: &str, is_cover: bool) -> Result<Image> {
        let image = self
            .conn
            .query_row(
                "SELECT id, entity_type, entity_id, file_path, caption, is_cover, created_at, version, deleted
                 FROM images WHERE id = ?1",
                params![id],
                |row| {
                    Ok(Image {
                        id: row.get(0)?,
                        entity_type: row.get(1)?,
                        entity_id: row.get(2)?,
                        file_path: row.get(3)?,
                        caption: row.get(4)?,
                        is_cover: row.get(5)?,
                        created_at: row.get(6)?,
                        version: row.get(7)?,
                        deleted: row.get(8)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("image {id}")))?;

        // 原子操作：同实体其他照片 is_cover = 0，目标照片按需设置
        if is_cover {
            self.conn.execute(
                "UPDATE images SET is_cover = (id = ?1), version = version + 1 WHERE entity_type = ?2 AND entity_id = ?3",
                params![id, image.entity_type, image.entity_id],
            )?;
        } else {
            self.conn.execute(
                "UPDATE images SET is_cover = 0, version = version + 1 WHERE id = ?1",
                params![id],
            )?;
        }

        self.list_images_by_entity(
            EntityType::from_str(&image.entity_type).ok_or_else(|| {
                AppError::General(format!("unknown entity_type {}", image.entity_type))
            })?,
            &image.entity_id,
        )?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| AppError::NotFound(format!("image {id}")))
    }

    // -----------------------------------------------------------------------
    // Edge 人物关系
    // -----------------------------------------------------------------------
    /// 创建关系。spouse 双向对称；son/daughter 只存单方向（父/母 → 子/女），反向由查询推导。
    pub fn create_edge(&mut self, input: NewEdge) -> Result<Vec<Edge>> {
        // 校验两个人物都存在
        self.get_member(&input.member_id)?;
        self.get_member(&input.related_member_id)?;
        if input.member_id == input.related_member_id {
            return Err(AppError::InvalidInput("cannot create self-relation".into()));
        }
        let now = now_iso();
        let rtype = input.relation_type.as_str().to_string();
        let id = new_uuid();

        // 检查是否已存在相同关系
        let exists: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM edges WHERE member_id = ?1 AND related_member_id = ?2 AND relation_type = ?3",
            params![input.member_id, input.related_member_id, input.relation_type.as_str()],
            |row| row.get(0),
        )?;
        if exists > 0 {
            return Err(AppError::InvalidInput("relation already exists".into()));
        }

        let tx = self.conn.transaction()?;
        // 插入正向关系
        tx.execute(
            "INSERT INTO edges (id, member_id, related_member_id, relation_type, created_at, side) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, input.member_id, input.related_member_id, rtype, now, input.side],
        )?;

        // spouse 双向对称，写入反向边
        if matches!(input.relation_type, EdgeType::Spouse) {
            let rev_id = new_uuid();
            tx.execute(
                "INSERT INTO edges (id, member_id, related_member_id, relation_type, created_at, side) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![rev_id, input.related_member_id, input.member_id, rtype, now, input.side],
            )?;
        }
        tx.commit()?;

        self.list_edges_by_member(&input.member_id)
    }

    fn row_to_edge(row: &rusqlite::Row<'_>) -> rusqlite::Result<Edge> {
        Ok(Edge {
            id: row.get(0)?,
            member_id: row.get(1)?,
            related_member_id: row.get(2)?,
            relation_type: row.get(3)?,
            created_at: row.get(4)?,
            side: row.get(5)?,
            version: row.get(6)?,
            deleted: row.get(7)?,
            related_member_name: row.get(8)?,
            related_member_grave_id: row.get(9)?,
        })
    }

    /// 列出某人的所有关系（双向）。JOIN 填充关联人物姓名和墓 ID。
    /// spouse 边成对存储；son/daughter 只存父→子方向，这里同时查反向（该人是子女）。
    pub fn list_edges_by_member(&self, member_id: &str) -> Result<Vec<Edge>> {
        let mut stmt = self.conn.prepare(
            "SELECT r.id, r.member_id, r.related_member_id, r.relation_type, r.created_at,
                    r.side, r.version, r.deleted,
                    p.name AS related_member_name, p.grave_id AS related_member_grave_id
             FROM edges r
             JOIN members p ON p.id = r.related_member_id
             WHERE r.member_id = ?1
                OR (r.related_member_id = ?1 AND r.relation_type IN ('son', 'daughter'))
             ORDER BY r.relation_type, p.name",
        )?;
        let rows = stmt.query_map(params![member_id], Self::row_to_edge)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 列出某族的全部在世成员（is_alive=1 且 clan_id 匹配）
    pub fn list_members_by_clan(&self, clan_id: &str, alive: Option<bool>) -> Result<Vec<Member>> {
        let mut sql = String::from(
            "SELECT id, grave_id, clan_id, name, title, birth_date, death_date, biography, epitaph, spouse, is_joint_burial, children, is_alive, order_index, created_at, updated_at, version, deleted
             FROM members WHERE clan_id = ?1",
        );
        if let Some(a) = alive {
            sql.push_str(if a { " AND is_alive = 1" } else { " AND is_alive = 0" });
        }
        sql.push_str(" ORDER BY is_alive, order_index, name");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![clan_id], Self::row_to_member)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 查询某族的关系图数据：全部人物（在世+已故），及人物间关系边（spouse 去重为一条）。
    /// 返回 (members, edges)。edges 用 member_id(源) 语义：spouse 取正向，son/daughter 取父→子。
    pub fn list_graph_by_clan(&self, clan_id: &str) -> Result<(Vec<Member>, Vec<Edge>)> {
        let members = self.list_members_by_clan(clan_id, None)?;
        // 收集该族人物 ID 集合，用于过滤边（只保留两端都属该族的边）
        let ids: std::collections::HashSet<String> =
            members.iter().map(|m| m.id.clone()).collect();
        let mut stmt = self.conn.prepare(
            "SELECT r.id, r.member_id, r.related_member_id, r.relation_type, r.created_at,
                    r.side, r.version, r.deleted,
                    p.name AS related_member_name, p.grave_id AS related_member_grave_id
             FROM edges r
             JOIN members p ON p.id = r.related_member_id
             WHERE (r.member_id IN (SELECT id FROM members WHERE clan_id = ?1)
                    OR r.related_member_id IN (SELECT id FROM members WHERE clan_id = ?1))
             ORDER BY r.relation_type",
        )?;
        let rows = stmt.query_map(params![clan_id], Self::row_to_edge)?;
        let mut edges: Vec<Edge> = rows
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| AppError::from(e))?;

        // spouse 成对存储，去重为一条（保留 member_id 为源的方向）
        let mut seen = std::collections::HashSet::new();
        edges.retain(|e| {
            let key = if e.relation_type == "spouse" {
                let (a, b) = if e.member_id < e.related_member_id {
                    (e.member_id.as_str(), e.related_member_id.as_str())
                } else {
                    (e.related_member_id.as_str(), e.member_id.as_str())
                };
                format!("{a}|{b}")
            } else {
                e.id.clone()
            };
            seen.insert(key)
        });

        // 只保留两端都在该族内的边
        edges.retain(|e| ids.contains(&e.member_id) && ids.contains(&e.related_member_id));

        Ok((members, edges))
    }

    /// 查询以某人物为中心、BFS 指定层数的子图（Ego 图）。
    /// 返回 (members, edges)。
    pub fn list_egograph(&self, member_id: &str, depth: usize) -> Result<(Vec<Member>, Vec<Edge>)> {
        let mut member_ids = std::collections::HashSet::new();
        member_ids.insert(member_id.to_string());
        let mut frontier: Vec<String> = vec![member_id.to_string()];

        // 逐层 BFS 收集层内人物
        for _ in 0..depth {
            let mut next: Vec<String> = Vec::new();
            for mid in &frontier {
                let mut stmt = self.conn.prepare(
                    "SELECT member_id, related_member_id FROM edges
                     WHERE member_id = ?1 OR related_member_id = ?1",
                )?;
                let rows = stmt.query_map(params![mid], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?;
                for row in rows {
                    let (a, b) = row?;
                    for other in [a, b] {
                        if member_ids.insert(other.clone()) {
                            next.push(other);
                        }
                    }
                }
            }
            frontier = next;
        }

        // 取人物
        let mut members = Vec::new();
        for mid in &member_ids {
            if let Ok(m) = self.get_member(mid) {
                members.push(m);
            }
        }

        // 取人物间边
        let placeholders = vec!["?"].repeat(member_ids.len()).join(",");
        let mut params_vec: Vec<String> = member_ids.iter().cloned().collect();
        params_vec.push(member_id.to_string());
        let mut stmt = self.conn.prepare(
            &format!(
                "SELECT r.id, r.member_id, r.related_member_id, r.relation_type, r.created_at,
                        r.side, r.version, r.deleted,
                        p.name AS related_member_name, p.grave_id AS related_member_grave_id
                 FROM edges r
                 JOIN members p ON p.id = r.related_member_id
                 WHERE r.member_id IN ({placeholders})
                    OR (r.related_member_id IN ({placeholders}) AND r.relation_type IN ('son','daughter'))"
            ),
        )?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params_vec), Self::row_to_edge)?;
        let edges: Vec<Edge> = rows
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| AppError::from(e))?;

        Ok((members, edges))
    }

    /// 删除单条关系及其反向关系
    pub fn delete_edge(&mut self, id: &str) -> Result<()> {
        // 先查出关系信息以便删除反向
        let rel = self.conn.query_row(
            "SELECT id, member_id, related_member_id, relation_type, created_at, side, version, deleted FROM edges WHERE id = ?1",
            params![id],
            |row| {
                Ok(Edge {
                    id: row.get(0)?,
                    member_id: row.get(1)?,
                    related_member_id: row.get(2)?,
                    relation_type: row.get(3)?,
                    created_at: row.get(4)?,
                    side: row.get(5)?,
                    version: row.get(6)?,
                    deleted: row.get(7)?,
                    related_member_name: None,
                    related_member_grave_id: None,
                })
            },
        ).optional()?
        .ok_or_else(|| AppError::NotFound(format!("relation {id}")))?;

        let tx = self.conn.transaction()?;
        // 删除正向关系
        let n = tx.execute("DELETE FROM edges WHERE id = ?1", params![id])?;
        // 删除反向关系
        tx.execute(
            "DELETE FROM edges WHERE member_id = ?1 AND related_member_id = ?2 AND relation_type = ?3",
            params![rel.related_member_id, rel.member_id, rel.relation_type],
        )?;
        tx.commit()?;
        if n == 0 {
            return Err(AppError::NotFound(format!("relation {id}")));
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // 搜索
    // -----------------------------------------------------------------------
    /// 按关键字搜索墓地（名称/地址/描述模糊匹配）
    pub fn search_graves(&self, keyword: &str) -> Result<Vec<Grave>> {
        let pattern = format!("%{}%", keyword);
        let mut stmt = self.conn.prepare(
            "SELECT id, name, latitude, longitude, address, description, burial_group_id, clan_id, created_at, updated_at, version, deleted
             FROM graves
             WHERE name LIKE ?1 OR address LIKE ?1 OR description LIKE ?1
             ORDER BY name",
        )?;
        let rows = stmt.query_map(params![pattern], Self::row_to_grave)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 按关键字搜索人物（姓名/生平模糊匹配），返回对应墓地
    pub fn search_members(&self, keyword: &str) -> Result<Vec<Grave>> {
        let pattern = format!("%{}%", keyword);
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT t.id, t.name, t.latitude, t.longitude, t.address, t.description, t.burial_group_id, t.clan_id, t.created_at, t.updated_at, t.version, t.deleted
             FROM members p JOIN graves t ON p.grave_id = t.id
             WHERE p.name LIKE ?1 OR p.biography LIKE ?1
             ORDER BY t.name",
        )?;
        let rows = stmt.query_map(params![pattern], Self::row_to_grave)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 按关键字搜索家族（名称/祖籍模糊匹配）
    pub fn search_clans(&self, keyword: &str) -> Result<Vec<Clan>> {
        let pattern = format!("%{}%", keyword);
        let mut stmt = self.conn.prepare(
            "SELECT id, name, description, origin, created_at, updated_at, version, deleted
             FROM clans WHERE name LIKE ?1 OR origin LIKE ?1 ORDER BY name",
        )?;
        let rows = stmt.query_map(params![pattern], |row| {
            Ok(Clan {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                origin: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                version: row.get(6)?,
                deleted: row.get(7)?,
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
            "SELECT id, grave_id, name, title, death_date FROM members WHERE death_date IS NOT NULL AND death_date != ''",
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
            let (id, grave_id, name, title, death_date) = row?;
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
                    member_id: Some(id.clone()),
                    grave_id: Some(grave_id.clone()),
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
                    member_id: None,
                    grave_id: None,
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
                        member_id: None,
                        grave_id: None,
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
                        member_id: None,
                        grave_id: None,
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
                        member_id: None,
                        grave_id: None,
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
                        member_id: None,
                        grave_id: None,
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

    fn setup() -> ClanTrailDb {
        ClanTrailDb::in_memory().expect("in-memory db")
    }

    #[test]
    fn clan_crud() {
        let db = setup();
        let f = db
            .create_clan(NewClan {
                name: "王氏家族".into(),
                description: Some("宗族档案".into()),
                origin: Some("山西洪洞".into()),
            })
            .unwrap();
        assert_eq!(db.get_clan(&f.id).unwrap().name, "王氏家族");
        assert_eq!(db.list_clans().unwrap().len(), 1);

        let updated = db
            .update_clan(&f.id, Some("王氏族谱".into()), None, None)
            .unwrap();
        assert_eq!(updated.name, "王氏族谱");
        assert_eq!(updated.origin.as_deref(), Some("山西洪洞"));

        db.delete_clan(&f.id).unwrap();
        assert!(db.get_clan(&f.id).is_err());
    }

    #[test]
    fn burial_group_belongs_to_clan() {
        let db = setup();
        let f = db
            .create_clan(NewClan { name: "李氏".into(), description: None, origin: None })
            .unwrap();
        let g = db
            .create_burial_group(NewBurialGroup {
                clan_id: f.id.clone(),
                name: "祖坟区".into(),
                description: Some("山腰老坟".into()),
            })
            .unwrap();
        assert_eq!(db.list_groups_by_clan(&f.id).unwrap().len(), 1);
        assert_eq!(db.list_groups_by_clan(&g.id).unwrap().len(), 0);

        // 删除家族应级联删除墓组
        db.delete_clan(&f.id).unwrap();
        assert!(db.list_groups_by_clan(&f.id).unwrap().is_empty());
    }

    #[test]
    fn clan_delete_cascades_graves_members_and_images() {
        let db = setup();
        let f = db
            .create_clan(NewClan {
                name: "陈氏".into(),
                description: None,
                origin: None,
            })
            .unwrap();
        let t = db
            .create_grave(NewGrave {
                name: "陈氏祖墓".into(),
                latitude: 23.1,
                longitude: 113.2,
                address: None,
                description: None,
                burial_group_id: None,
                clan_id: Some(f.id.clone()),
            })
            .unwrap();
        let p = db
            .create_member(NewMember {
                grave_id: Some(t.id.clone()),
                clan_id: None,
                is_alive: false,
                name: "陈公".into(),
                title: None,
                birth_date: None,
                death_date: None,
                biography: None,
                epitaph: None,
                spouse: None,
                is_joint_burial: false,
                children: None,
                order_index: 1,
            })
            .unwrap();
        db.add_image(NewImage {
            entity_type: EntityType::Member,
            entity_id: p.id.clone(),
            file_path: "photos/chen.jpg".into(),
            caption: None,
            is_cover: false,
        })
        .unwrap();

        // 删除家族前，子级均存在
        assert_eq!(db.list_graves_by_clan(&f.id).unwrap().len(), 1);
        assert_eq!(db.list_members_by_grave(&t.id).unwrap().len(), 1);
        assert_eq!(db.list_images_by_entity(EntityType::Member, &p.id).unwrap().len(), 1);

        // 删除家族应级联清理墓地、墓主与照片
        db.delete_clan(&f.id).unwrap();
        assert!(db.get_clan(&f.id).is_err());
        assert!(db.get_grave(&t.id).is_err());
        assert!(db.get_member(&p.id).is_err());
        assert!(db.list_images_by_entity(EntityType::Member, &p.id).unwrap().is_empty());
    }

    #[test]
    fn grave_crud_with_edges() {
        let db = setup();
        let f = db
            .create_clan(NewClan { name: "赵氏".into(), description: None, origin: None })
            .unwrap();
        let g = db
            .create_burial_group(NewBurialGroup {
                clan_id: f.id.clone(),
                name: "东区".into(),
                description: None,
            })
            .unwrap();

        let t = db
            .create_grave(NewGrave {
                name: "赵公明墓".into(),
                latitude: 34.21,
                longitude: 108.94,
                address: Some("西安东郊".into()),
                description: Some("明代墓".into()),
                burial_group_id: Some(g.id.clone()),
                clan_id: Some(f.id.clone()),
            })
            .unwrap();
        assert_eq!(db.list_graves_by_clan(&f.id).unwrap().len(), 1);
        assert_eq!(db.list_graves_by_group(&g.id).unwrap().len(), 1);

        // 非法经纬度
        let bad = db.create_grave(NewGrave {
            name: "bad".into(),
            latitude: 100.0,
            longitude: 0.0,
            address: None,
            description: None,
            burial_group_id: None,
            clan_id: None,
        });
        assert!(bad.is_err());

        // 关联不存在家族应报错
        let bad_clan = db.create_grave(NewGrave {
            name: "bad2".into(),
            latitude: 0.0,
            longitude: 0.0,
            address: None,
            description: None,
            burial_group_id: None,
            clan_id: Some("not-exist".into()),
        });
        assert!(bad_clan.is_err());
        drop(t);
    }

    #[test]
    fn member_and_images() {
        let db = setup();
        let t = db
            .create_grave(NewGrave {
                name: "合葬墓".into(),
                latitude: 30.5,
                longitude: 114.3,
                address: None,
                description: None,
                burial_group_id: None,
                clan_id: None,
            })
            .unwrap();

        let p1 = db
            .create_member(NewMember {
                grave_id: Some(t.id.clone()),
                clan_id: None,
                is_alive: false,
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

        db.create_member(NewMember {
            grave_id: Some(t.id.clone()),
                clan_id: None,
                is_alive: false,
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

        let members = db.list_members_by_grave(&t.id).unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members[0].order_index, 1);
        assert_eq!(members[1].order_index, 2);

        // 测试 update_member
        let updated = db
            .update_member(
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
                None,
                None,
            )
            .unwrap();
        assert_eq!(updated.spouse.as_deref(), Some("张氏"));
        assert!(!updated.is_joint_burial);
        assert_eq!(updated.order_index, 5);

        db.add_image(NewImage {
            entity_type: EntityType::Grave,
            entity_id: t.id.clone(),
            file_path: "/images/grave1.jpg".into(),
            caption: Some("墓碑".into()),
            is_cover: true,
        })
        .unwrap();
        let images = db.list_images_by_entity(EntityType::Grave, &t.id).unwrap();
        assert_eq!(images.len(), 1);
        assert!(images[0].is_cover);
    }

    #[test]
    fn search_works() {
        let db = setup();
        let f = db
            .create_clan(NewClan {
                name: "陈家".into(),
                description: None,
                origin: Some("福建".into()),
            })
            .unwrap();
        let t = db
            .create_grave(NewGrave {
                name: "陈公墓".into(),
                latitude: 24.5,
                longitude: 118.1,
                address: Some("厦门集美".into()),
                description: Some("清代墓".into()),
                burial_group_id: None,
                clan_id: Some(f.id.clone()),
            })
            .unwrap();
        db.create_member(NewMember {
            grave_id: Some(t.id.clone()),
                clan_id: None,
                is_alive: false,
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

        assert_eq!(db.search_graves("陈公").unwrap().len(), 1);
        assert_eq!(db.search_graves("集美").unwrap().len(), 1);
        assert_eq!(db.search_members("永福").unwrap().len(), 1);
        assert_eq!(db.search_members("举人").unwrap().len(), 1);
        assert_eq!(db.search_clans("福建").unwrap().len(), 1);
        assert_eq!(db.search_graves("不存在").unwrap().len(), 0);
    }

    #[test]
    fn lunar_solar_roundtrip() {
        use lunar_lite::{lunar_to_solar, solar_to_lunar, LunarDate, SolarDate};
        // 已知锚点：2023-01-22 是农历癸卯年正月初一
        let l = solar_to_lunar(SolarDate {
            year: 2023,
            month: 1,
            day: 22,
        })
        .expect("solar_to_lunar 失败");
        assert_eq!((l.month, l.day, l.is_leap_month), (1, 1, false));
        // 回推：农历 2023 正月初一 应为公历 2023-01-22
        let s = lunar_to_solar(LunarDate {
            year: 2023,
            month: 1,
            day: 1,
            is_leap_month: false,
        })
        .expect("lunar_to_solar 失败");
        assert_eq!((s.year, s.month, s.day), (2023, 1, 22));

        // 任意日期往返一致性（最易出错的换算核心）
        let orig = SolarDate {
            year: 1995,
            month: 8,
            day: 15,
        };
        let lunar = solar_to_lunar(orig).expect("solar_to_lunar 失败");
        let back = lunar_to_solar(LunarDate {
            year: lunar.year,
            month: lunar.month,
            day: lunar.day,
            is_leap_month: lunar.is_leap_month,
        })
        .expect("lunar_to_solar 失败");
        assert_eq!(
            (back.year, back.month, back.day),
            (orig.year, orig.month, orig.day)
        );
    }

    #[test]
    fn list_reminders_includes_death_anniversary() {
        let db = setup();
        let t = db
            .create_grave(NewGrave {
                name: "忌日测试墓".into(),
                latitude: 30.0,
                longitude: 114.0,
                address: None,
                description: None,
                burial_group_id: None,
                clan_id: None,
            })
            .unwrap();
        let p = db
            .create_member(NewMember {
                grave_id: Some(t.id.clone()),
                clan_id: None,
                is_alive: false,
                name: "忌日测试人".into(),
                title: Some("考".into()),
                birth_date: None,
                death_date: Some("1980-06-15".into()),
                biography: None,
                epitaph: None,
                spouse: None,
                is_joint_burial: false,
                children: None,
                order_index: 1,
            })
            .unwrap();

        // 窗口放大到 3 年，必然覆盖至少一个周年忌日
        let reminders = db.list_reminders(365 * 3).unwrap();
        let hit = reminders
            .iter()
            .find(|r| r.member_id.as_deref() == Some(p.id.as_str()));
        assert!(hit.is_some(), "应为该逝者生成忌日提醒");
        let hit = hit.unwrap();
        assert!(hit.title.contains("忌日测试人"), "标题应包含逝者名");
        assert!(
            hit.lunar_date.contains("农历"),
            "lunar_date 应为农历表述，实际: {}",
            hit.lunar_date
        );
        assert!(hit.days_until >= 0);
    }
}