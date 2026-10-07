-- Phase 1 only: the personal profile and versioned migration ledger.
CREATE TABLE schema_migrations (
    version INTEGER PRIMARY KEY NOT NULL CHECK (version > 0),
    applied_at TEXT NOT NULL,
    checksum TEXT NOT NULL CHECK (length(checksum) = 64)
);

CREATE TABLE personal_profile (
    profile_id TEXT PRIMARY KEY NOT NULL CHECK (length(profile_id) = 36),
    singleton INTEGER NOT NULL DEFAULT 1 UNIQUE CHECK (singleton = 1),
    user_name TEXT NOT NULL,
    tone TEXT NOT NULL,
    report_style TEXT NOT NULL,
    preferred_kpis_json TEXT NOT NULL
        CHECK (json_valid(preferred_kpis_json) AND json_type(preferred_kpis_json) = 'array'),
    work_hours_json TEXT NOT NULL
        CHECK (json_valid(work_hours_json) AND json_type(work_hours_json) = 'object'),
    settings_json TEXT NOT NULL
        CHECK (json_valid(settings_json) AND json_type(settings_json) = 'object')
);
