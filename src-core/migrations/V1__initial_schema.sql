CREATE TABLE IF NOT EXISTS clans (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT,
    origin      TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS burial_groups (
    id          TEXT PRIMARY KEY,
    clan_id   TEXT NOT NULL REFERENCES clans(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    description TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS graves (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    latitude    REAL NOT NULL,
    longitude   REAL NOT NULL,
    address     TEXT,
    description TEXT,
    burial_group_id    TEXT REFERENCES burial_groups(id) ON DELETE SET NULL,
    clan_id   TEXT REFERENCES clans(id) ON DELETE SET NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS members (
    id              TEXT PRIMARY KEY,
    grave_id         TEXT NOT NULL REFERENCES graves(id) ON DELETE CASCADE,
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

CREATE TABLE IF NOT EXISTS images (
    id          TEXT PRIMARY KEY,
    entity_type TEXT NOT NULL,
    entity_id   TEXT NOT NULL,
    file_path   TEXT NOT NULL,
    caption     TEXT,
    is_cover    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS edges (
    id                TEXT PRIMARY KEY,
    member_id         TEXT NOT NULL REFERENCES members(id) ON DELETE CASCADE,
    related_member_id TEXT NOT NULL REFERENCES members(id) ON DELETE CASCADE,
    relation_type     TEXT NOT NULL,
    created_at        TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_graves_group   ON graves(burial_group_id);
CREATE INDEX IF NOT EXISTS idx_graves_family  ON graves(clan_id);
CREATE INDEX IF NOT EXISTS idx_members_tomb  ON members(grave_id);
CREATE INDEX IF NOT EXISTS idx_images_entity ON images(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_edges_person ON edges(member_id);
CREATE INDEX IF NOT EXISTS idx_edges_related ON edges(related_member_id);