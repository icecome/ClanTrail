-- V4: 人物模型升级——Member 从「墓主」升级为「人物」
-- 1) grave_id 改为可空（在世亲属无墓）
-- 2) 新增 clan_id（在世者直接归族）
-- 3) 新增 is_alive（在世标记）
-- SQLite 不支持 ALTER COLUMN DROP NOT NULL，需重建表。

CREATE TABLE members_v4 (
    id              TEXT PRIMARY KEY,
    grave_id        TEXT REFERENCES graves(id) ON DELETE SET NULL,
    clan_id         TEXT REFERENCES clans(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    title           TEXT,
    birth_date      TEXT,
    death_date      TEXT,
    biography       TEXT,
    epitaph         TEXT,
    spouse          TEXT,
    is_joint_burial INTEGER NOT NULL DEFAULT 0,
    children        TEXT,
    is_alive        INTEGER NOT NULL DEFAULT 0,
    order_index     INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    version         INTEGER NOT NULL DEFAULT 1,
    deleted         INTEGER NOT NULL DEFAULT 0
);

-- 迁移旧数据：clan_id 从所属墓反查；旧墓主默认 is_alive=0
INSERT INTO members_v4 (id, grave_id, clan_id, name, title, birth_date, death_date,
     biography, epitaph, spouse, is_joint_burial, children, is_alive, order_index,
     created_at, updated_at, version, deleted)
SELECT m.id, m.grave_id, g.clan_id, m.name, m.title, m.birth_date, m.death_date,
     m.biography, m.epitaph, m.spouse, m.is_joint_burial, m.children, 0, m.order_index,
     m.created_at, m.updated_at, COALESCE(m.version, 1), COALESCE(m.deleted, 0)
FROM members m
LEFT JOIN graves g ON g.id = m.grave_id;

DROP TABLE members;
ALTER TABLE members_v4 RENAME TO members;

CREATE INDEX IF NOT EXISTS idx_members_tomb  ON members(grave_id);
CREATE INDEX IF NOT EXISTS idx_members_clan  ON members(clan_id);
CREATE INDEX IF NOT EXISTS idx_members_alive ON members(is_alive);